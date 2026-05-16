use super::super::*;

impl Runtime {
    pub fn install_audio_extension(&mut self) -> Result<AudioExtension, RuntimeError> {
        let load_host_id = 150;
        let play_host_id = 151;
        let pause_host_id = 152;
        let stop_host_id = 153;
        let seek_host_id = 154;
        let volume_host_id = 155;
        let release_host_id = 156;
        let status_host_id = 157;
        let playback_host_id = 158;
        let resources = self.resources.clone();
        let audio_states = self.audio_states.clone();

        let _ = self.register_host_function(
            HostFunction::new(load_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let resource_id = expect_integer_arg(args, 0, "resource_id")? as u32;
                match resources
                    .borrow_mut()
                    .load_resource(resource_id)
                    .map_err(resource_error_to_host_error)?
                {
                    LoadResult::Ready(handle) => {
                        audio_states.borrow_mut().insert(
                            handle.raw(),
                            AudioPlaybackState {
                                resource_id,
                                playing: false,
                                looped: false,
                                position_ms: 0,
                                volume: 1.0,
                            },
                        );
                        Ok(Value::Handle(handle.into()))
                    }
                    LoadResult::Pending(request_id) => Ok(Value::Integer(request_id as i64)),
                }
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(play_host_id, 1, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let looped = args.get(1).map(|value| value.truthy()).unwrap_or(false);
                let (resource_id, bytes) = audio_bytes_for_handle(&resources, handle)?;
                let backend = audio_backend.clone();
                let mut states = audio_states.borrow_mut();
                let state = states.entry(handle).or_insert_with(|| AudioPlaybackState {
                    resource_id,
                    playing: false,
                    looped: false,
                    position_ms: 0,
                    volume: 1.0,
                });
                backend.play(
                    handle,
                    resource_id,
                    &bytes,
                    looped,
                    state.position_ms,
                    state.volume,
                )?;
                state.resource_id = resource_id;
                state.playing = true;
                state.looped = looped;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(playback_host_id, 1, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let looped = args.get(1).map(|value| value.truthy()).unwrap_or(false);
                let (resource_id, bytes) = audio_bytes_for_handle(&resources, handle)?;
                let backend = audio_backend.clone();
                let mut states = audio_states.borrow_mut();
                let state = states.entry(handle).or_insert_with(|| AudioPlaybackState {
                    resource_id,
                    playing: false,
                    looped: false,
                    position_ms: 0,
                    volume: 1.0,
                });
                backend.play(
                    handle,
                    resource_id,
                    &bytes,
                    looped,
                    state.position_ms,
                    state.volume,
                )?;
                state.resource_id = resource_id;
                state.playing = true;
                state.looped = looped;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(pause_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                audio_backend.pause(handle)?;
                if let Some(state) = audio_states.borrow_mut().get_mut(&handle) {
                    state.playing = false;
                }
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(stop_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                audio_backend.stop(handle)?;
                if let Some(state) = audio_states.borrow_mut().get_mut(&handle) {
                    state.playing = false;
                    state.position_ms = 0;
                }
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(seek_host_id, 2, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let position_ms = expect_number_arg(args, 1, "position_ms")?;
                audio_backend.seek(handle, position_ms.max(0.0) as u64)?;
                let mut states = audio_states.borrow_mut();
                let state = states
                    .entry(handle)
                    .or_insert_with(AudioPlaybackState::default);
                state.position_ms = position_ms.max(0.0) as u64;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(volume_host_id, 2, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let volume = expect_number_arg(args, 1, "volume")?;
                audio_backend.volume(handle, volume.clamp(0.0, 1.0) as f32)?;
                let mut states = audio_states.borrow_mut();
                let state = states
                    .entry(handle)
                    .or_insert_with(AudioPlaybackState::default);
                state.volume = volume.clamp(0.0, 1.0) as f32;
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(release_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                audio_backend.release(handle.raw())?;
                audio_states.borrow_mut().remove(&handle.raw());
                resources
                    .borrow_mut()
                    .release(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let _ = self.register_host_function(
            HostFunction::new(status_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let status = audio_states
                    .borrow()
                    .get(&handle)
                    .map(|state| if state.playing { 2 } else { 1 })
                    .unwrap_or(0);
                Ok(Value::Integer(status))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.audio",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_ASYNC_IO),
                ExtensionFunctionSpec::new("play", play_host_id, 1, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("playback", playback_host_id, 1, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("pause", pause_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("stop", stop_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("seek", seek_host_id, 2, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("volume", volume_host_id, 2, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Integer),
            ],
        )?;

        Ok(AudioExtension {
            load_ext_id: ids[0],
            play_ext_id: ids[1],
            playback_ext_id: ids[2],
            pause_ext_id: ids[3],
            stop_ext_id: ids[4],
            seek_ext_id: ids[5],
            volume_ext_id: ids[6],
            release_ext_id: ids[7],
            status_ext_id: ids[8],
            load_host_id,
            play_host_id,
            playback_host_id,
            pause_host_id,
            stop_host_id,
            seek_host_id,
            volume_host_id,
            release_host_id,
            status_host_id,
        })
    }
}
