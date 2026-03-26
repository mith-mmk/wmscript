#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use wmlvm::HostError;

pub trait AudioBackend {
    fn play(
        &mut self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
        looped: bool,
        position_ms: u64,
        volume: f32,
    ) -> Result<(), HostError>;

    fn pause(&mut self, handle: u64) -> Result<(), HostError>;

    fn stop(&mut self, handle: u64) -> Result<(), HostError>;

    fn seek(&mut self, handle: u64, position_ms: u64) -> Result<(), HostError>;

    fn volume(&mut self, handle: u64, volume: f32) -> Result<(), HostError>;

    fn release(&mut self, handle: u64) -> Result<(), HostError>;

    fn clear(&mut self) -> Result<(), HostError>;
}

pub fn create_disabled_audio_backend() -> Box<dyn AudioBackend> {
    Box::new(DisabledAudioBackend)
}

pub fn create_default_audio_backend() -> Box<dyn AudioBackend> {
    #[cfg(target_os = "windows")]
    {
        Box::new(PowerShellAudioBackend::default())
    }

    #[cfg(not(target_os = "windows"))]
    {
        create_disabled_audio_backend()
    }
}

/// Shared audio backend handle that keeps playback alive until the last clone drops.
pub struct SharedAudioBackend {
    inner: RefCell<Box<dyn AudioBackend>>,
}

impl SharedAudioBackend {
    pub fn new(backend: Box<dyn AudioBackend>) -> Self {
        Self {
            inner: RefCell::new(backend),
        }
    }

    pub fn replace(&self, backend: Box<dyn AudioBackend>) {
        *self.inner.borrow_mut() = backend;
    }

    pub fn play(
        &self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
        looped: bool,
        position_ms: u64,
        volume: f32,
    ) -> Result<(), HostError> {
        self.inner
            .borrow_mut()
            .play(handle, resource_id, bytes, looped, position_ms, volume)
    }

    pub fn pause(&self, handle: u64) -> Result<(), HostError> {
        self.inner.borrow_mut().pause(handle)
    }

    pub fn stop(&self, handle: u64) -> Result<(), HostError> {
        self.inner.borrow_mut().stop(handle)
    }

    pub fn seek(&self, handle: u64, position_ms: u64) -> Result<(), HostError> {
        self.inner.borrow_mut().seek(handle, position_ms)
    }

    pub fn volume(&self, handle: u64, volume: f32) -> Result<(), HostError> {
        self.inner.borrow_mut().volume(handle, volume)
    }

    pub fn release(&self, handle: u64) -> Result<(), HostError> {
        self.inner.borrow_mut().release(handle)
    }

    pub fn clear(&self) -> Result<(), HostError> {
        self.inner.borrow_mut().clear()
    }
}

impl Drop for SharedAudioBackend {
    fn drop(&mut self) {
        let _ = self.inner.borrow_mut().clear();
    }
}

impl fmt::Debug for SharedAudioBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SharedAudioBackend")
    }
}

struct DisabledAudioBackend;

impl AudioBackend for DisabledAudioBackend {
    fn play(
        &mut self,
        _handle: u64,
        _resource_id: u32,
        _bytes: &[u8],
        _looped: bool,
        _position_ms: u64,
        _volume: f32,
    ) -> Result<(), HostError> {
        Ok(())
    }

    fn pause(&mut self, _handle: u64) -> Result<(), HostError> {
        Ok(())
    }

    fn stop(&mut self, _handle: u64) -> Result<(), HostError> {
        Ok(())
    }

    fn seek(&mut self, _handle: u64, _position_ms: u64) -> Result<(), HostError> {
        Ok(())
    }

    fn volume(&mut self, _handle: u64, _volume: f32) -> Result<(), HostError> {
        Ok(())
    }

    fn release(&mut self, _handle: u64) -> Result<(), HostError> {
        Ok(())
    }

    fn clear(&mut self) -> Result<(), HostError> {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[derive(Default)]
struct PowerShellAudioBackend {
    sessions: BTreeMap<u64, AudioSession>,
}

#[cfg(target_os = "windows")]
struct AudioSession {
    child: Child,
    stdin: ChildStdin,
    resource_id: u32,
    path: PathBuf,
}

#[cfg(target_os = "windows")]
impl AudioBackend for PowerShellAudioBackend {
    fn play(
        &mut self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
        looped: bool,
        position_ms: u64,
        volume: f32,
    ) -> Result<(), HostError> {
        self.cleanup_finished(handle);
        let session = self.ensure_session(handle, resource_id, bytes)?;
        session.send_line(&format!("loop:{}", u8::from(looped)))?;
        session.send_line(&format!("volume:{:.6}", volume.clamp(0.0, 1.0)))?;
        session.send_line(&format!("seek:{position_ms}"))?;
        session.send_line("play")?;
        Ok(())
    }

    fn pause(&mut self, handle: u64) -> Result<(), HostError> {
        if let Some(session) = self.sessions.get_mut(&handle) {
            session.send_line("pause")?;
        }
        self.cleanup_finished(handle);
        Ok(())
    }

    fn stop(&mut self, handle: u64) -> Result<(), HostError> {
        if let Some(mut session) = self.sessions.remove(&handle) {
            let _ = session.send_line("stop");
            let _ = session.send_line("exit");
            let _ = terminate_process_tree(session.child.id());
            let _ = session.child.wait();
            let _ = fs::remove_file(&session.path);
        }
        Ok(())
    }

    fn seek(&mut self, handle: u64, position_ms: u64) -> Result<(), HostError> {
        if let Some(session) = self.sessions.get_mut(&handle) {
            session.send_line(&format!("seek:{position_ms}"))?;
        }
        self.cleanup_finished(handle);
        Ok(())
    }

    fn volume(&mut self, handle: u64, volume: f32) -> Result<(), HostError> {
        if let Some(session) = self.sessions.get_mut(&handle) {
            session.send_line(&format!("volume:{:.6}", volume.clamp(0.0, 1.0)))?;
        }
        self.cleanup_finished(handle);
        Ok(())
    }

    fn release(&mut self, handle: u64) -> Result<(), HostError> {
        self.stop(handle)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), HostError> {
        let handles = self.sessions.keys().copied().collect::<Vec<_>>();
        for handle in handles {
            self.release(handle)?;
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for PowerShellAudioBackend {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

#[cfg(target_os = "windows")]
impl PowerShellAudioBackend {
    fn ensure_session(
        &mut self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
    ) -> Result<&mut AudioSession, HostError> {
        let needs_restart = match self.sessions.get_mut(&handle) {
            Some(session) => {
                if session.resource_id != resource_id {
                    true
                } else {
                    match session.child.try_wait() {
                        Ok(Some(_)) => true,
                        Ok(None) => false,
                        Err(error) => return Err(HostError::Failed(error.to_string())),
                    }
                }
            }
            None => true,
        };

        if needs_restart {
            if let Some(mut session) = self.sessions.remove(&handle) {
                let _ = terminate_process_tree(session.child.id());
                let _ = session.child.wait();
                let _ = fs::remove_file(&session.path);
            }
            let session = self.spawn_session(handle, resource_id, bytes)?;
            self.sessions.insert(handle, session);
        }

        self.sessions
            .get_mut(&handle)
            .ok_or_else(|| HostError::Failed("audio session missing after spawn".to_owned()))
    }

    fn spawn_session(
        &self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
    ) -> Result<AudioSession, HostError> {
        let path = self.write_temp_audio_file(handle, resource_id, bytes)?;
        let uri = path_to_file_uri(&path);
        let encoded = encode_powershell_command(&audio_script());
        let mut child = spawn_shell(&encoded, &uri)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HostError::Failed("audio process has no stdin".to_owned()))?;
        Ok(AudioSession {
            child,
            stdin,
            resource_id,
            path,
        })
    }

    fn write_temp_audio_file(
        &self,
        handle: u64,
        resource_id: u32,
        bytes: &[u8],
    ) -> Result<PathBuf, HostError> {
        let ext = audio_file_extension(bytes);
        let path = std::env::temp_dir().join(format!("wml-audio-{handle}-{resource_id}.{ext}"));
        fs::write(&path, bytes).map_err(|error| HostError::Failed(error.to_string()))?;
        Ok(path)
    }

    fn cleanup_finished(&mut self, handle: u64) {
        let finished = self
            .sessions
            .get_mut(&handle)
            .and_then(|session| match session.child.try_wait() {
                Ok(Some(_)) => Some(true),
                Ok(None) => Some(false),
                Err(_) => Some(true),
            })
            .unwrap_or(false);
        if finished {
            if let Some(session) = self.sessions.remove(&handle) {
                let _ = fs::remove_file(&session.path);
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl AudioSession {
    fn send_line(&mut self, line: &str) -> Result<(), HostError> {
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|error| HostError::Failed(error.to_string()))
    }
}

#[cfg(target_os = "windows")]
fn spawn_shell(encoded_command: &str, audio_uri: &str) -> Result<Child, HostError> {
    fn spawn_with(
        binary: &str,
        encoded_command: &str,
        audio_uri: &str,
    ) -> Result<Child, std::io::Error> {
        let mut command = Command::new(binary);
        command
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-EncodedCommand")
            .arg(encoded_command)
            .env("WML_AUDIO_URI", audio_uri)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn()
    }

    match spawn_with("powershell.exe", encoded_command, audio_uri) {
        Ok(child) => Ok(child),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            spawn_with("pwsh.exe", encoded_command, audio_uri)
                .map_err(|error| HostError::Failed(error.to_string()))
        }
        Err(error) => Err(HostError::Failed(error.to_string())),
    }
}

#[cfg(target_os = "windows")]
fn audio_file_extension(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        "wav"
    } else if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
        "mp3"
    } else {
        "aud"
    }
}

#[cfg(target_os = "windows")]
fn audio_script() -> String {
    String::from(
        r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore
$uri = $env:WML_AUDIO_URI
$script:player = New-Object System.Windows.Media.MediaPlayer
$script:looped = $false
$script:playing = $false
function Stop-Playback {
    $script:playing = $false
    try {
        $script:player.Stop()
        $script:player.Position = [TimeSpan]::Zero
    } catch {
    }
}
function Close-Playback {
    Stop-Playback
    try {
        $script:player.Close()
    } catch {
    }
}
try {
    $script:player.Open([Uri]::new($uri))
    $script:player.Volume = 1.0
    while ($true) {
        if ([Console]::In.Peek() -ge 0) {
            $line = [Console]::In.ReadLine()
            if ($null -eq $line) { break }
            switch -Regex ($line) {
                '^exit$' {
                    break
                }
                '^play$' {
                    $script:playing = $true
                    $script:player.Play()
                }
                '^pause$' {
                    Stop-Playback
                }
                '^stop$' {
                    Stop-Playback
                }
                '^seek:(\d+)$' {
                    $script:player.Position = [TimeSpan]::FromMilliseconds([double]$Matches[1])
                }
                '^volume:([0-9.]+)$' {
                    $script:player.Volume = [double]::Parse(
                        $Matches[1],
                        [System.Globalization.CultureInfo]::InvariantCulture
                    )
                }
                '^loop:(\d+)$' {
                    $script:looped = [int]$Matches[1] -ne 0
                }
            }
        } elseif ($script:looped -and $script:playing -and $script:player.NaturalDuration.HasTimeSpan) {
            $duration_ms = $script:player.NaturalDuration.TimeSpan.TotalMilliseconds
            if ($duration_ms -gt 0 -and $script:player.Position.TotalMilliseconds -ge ($duration_ms - 40)) {
                $script:player.Position = [TimeSpan]::Zero
                $script:player.Play()
            }
            Start-Sleep -Milliseconds 20
        } else {
            Start-Sleep -Milliseconds 20
        }
    }
} finally {
    Close-Playback
}
"#,
    )
}

fn encode_powershell_command(script: &str) -> String {
    let utf16 = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<u8>>();
    base64_encode(&utf16)
}

#[cfg(target_os = "windows")]
fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{}", normalized)
    } else {
        format!("file:///{}", normalized)
    }
}

#[cfg(target_os = "windows")]
fn terminate_process_tree(pid: u32) -> Result<(), HostError> {
    match Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(HostError::Failed(format!(
            "failed to terminate audio process tree {pid}: {status}"
        ))),
        Err(error) => Err(HostError::Failed(error.to_string())),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        let triple = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        encoded.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        encoded.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        encoded.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        encoded.push(TABLE[(triple & 0x3F) as usize] as char);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let mut triple = (remainder[0] as u32) << 16;
        if remainder.len() == 2 {
            triple |= (remainder[1] as u32) << 8;
        }
        encoded.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        encoded.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if remainder.len() == 2 {
            encoded.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            encoded.push('=');
        }
        encoded.push('=');
    }

    encoded
}
