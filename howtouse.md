# How To Use

This document mirrors the contents of `samples/` and the runtime/toolchain
examples used in this workspace.

## Samples

- `helloworld/`
  - Minimal script sample that returns a folded arithmetic expression.
- `inputlink/`
  - Host-driven input sample that returns a string from the runtime bridge.
- `workercomm/`
  - Two-worker message passing sample.
- `engineworker/`
  - Engine-driven message window sample with page advance, named choices, and follow-up input.
- `messagewindow/`
  - Focused message window sample that pages text with `recv()` and clears prompt/choice state from the script.
- `assetload/`
  - Runtime archive and resource loading sample.
- `imageaudio/`
  - Combined image draw and audio playback demo.
- `uiimage/`
  - Scene layout demo that mirrors `samples/uiimage.png`.
- `easynovel/`
  - Small story-driven sample with chapters and narration.

## Hello World Sample

This sample is intentionally tiny. It exercises the compiler front end,
constant folding, and the VM runtime with a single exported entry point.

Source:

```wms
export func main() {
    return 1 + 2 * 3;
}
```

Expected result:

- The optimizer folds the expression to `7`.
- The runtime returns `7` from `main`.

## Input Link Sample

This sample shows a script that reads a value from a host callback and returns it.

Source:

```wms
export func main() {
    return input();
}
```

Runtime behavior:

- The host provides a single string input.
- The script returns that string unchanged.

## Worker Communication Sample

This sample demonstrates one worker sending a string to another worker.

Source:

```wms
worker sender {
    send 2, "hello worker";
}

worker receiver {
    return recv();
}
```

Runtime behavior:

- Worker 1 sends a payload to worker 2.
- Worker 2 receives the payload and returns it.

## Asset Load Sample

This sample shows a bundled asset being packaged into an archive and then loaded
through the runtime resource manager.

Source:

```wms
export func main() {
    return load_asset(100);
}
```

Runtime behavior:

- The archive contains a single asset resource.
- The runtime loads the archive and resolves the resource by id.
- The resource bytes become available through the resource manager.

## Easy Novel Sample

This sample is a tiny story-driven script. It keeps the structure of a visual-novel
style project, but stays within the compiler's current expression and function model.
The runtime example chooses which chapter to run via a command-line argument, and the
frontend message window renders the returned chapter text directly.

Source:

```wms
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Narrator: The last train platform is almost empty.\nNarrator: Aki stops under the lantern light and listens to the rails.\nAki: The city feels farther away than usual.";
}

export func chapter_1() {
    return "Narrator: A lantern lights the stairs down to the station.\nAki: The next train is still ten minutes away.\nNarrator: A quiet voice answers from the ticket gate.";
}

export func chapter_2() {
    return "Narrator: Aki chooses the quiet route home.\nAki: I'll take the river path tonight.\nNarrator: The station lights fade behind the empty road.";
}

export func main() {
    return "Narrator: Select a chapter from the runtime example.\nNarrator: prologue, chapter_1, or chapter_2.\nNarrator: The returned text will appear in the message window.";
}
```

Notes:

- The current compiler can emit these functions directly because each body is a
  simple `return` expression.
- The sample is intentionally written to be easy to extend with branching later.
- The frontend reads the final returned string and shows it in the message window.

Run examples:

- `cargo run -p wmruntime --example easynovel`
- `cargo run -p wmruntime --example easynovel -- chapter_1`
- `cargo run -p wmruntime --example easynovel -- chapter_2`

## Compile And Run

The runtime examples are the canonical way to exercise the current workspace.

```bash
cargo run -p wmruntime --example hello_runtime
cargo run -p wmruntime --example input_link
cargo run -p wmruntime --example worker_comm
cargo run -p wmruntime --example asset_load
cargo run -p wmruntime --example easynovel
cargo run -p wmfrontend -- samples/easynovel/main.wms --platform egui --font noto
cargo run -p wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto
cargo run -p wmfrontend -- --demo uiimage --platform egui --font noto
cargo run -p wmfrontend -- --demo image-audio --platform egui --font noto
cargo run -p wmfrontend -- --demo engineworker --platform egui --font noto
cargo run -p wmfrontend -- --demo messagewindow --platform egui --font noto
```

The egui frontend defaults to Noto Sans for Japanese-friendly rendering. Use
`--font default`, `--font noto`, or `--font mono` to switch presets.

The compiler also rejects `ext.*` calls that require capabilities missing from
the selected platform profile. For example, `ext.fs.*` and `ext.net.*` are not
available under `wasm`, while `state.*` and `ext.vm.*` remain portable.

`wmfrontend` also supports a built-in demo mode:

- `--demo uiimage` runs the embedded scene layout showcase without reading a script file.
- `--demo image-audio` runs the embedded image/audio showcase without reading a script file.
- `--demo engineworker` runs the embedded worker split showcase without reading a script file.
- `--demo messagewindow` runs the embedded engine-driven message window showcase without reading a script file.
- `--package NAME` overrides the demo package name.
- `--platform native|wasm|egui` selects the runtime/backend profile.
- `--image NAME=PATH` and `--asset NAME=PATH` attach extra resources in file-backed mode.

## Toolchain

`wmtoolchain` compiles a WML script into a packaged archive and can optionally
bundle assets into the output.

Command line:

```bash
wmtoolchain <script.wms> [--package NAME] [--out FILE] [--step-limit N] [--platform native|wasm|egui] [--release] [--asset NAME=PATH] [--image NAME=PATH]
```

Behavior:

- `script.wms` is the input source file.
- `--package NAME` overrides the package name. If omitted, the package name is
  derived from the script file stem.
- `--out FILE` writes the generated archive to a custom path. If omitted, the
  output defaults to `<script>.warc`.
- `--step-limit N` sets the VM step limit used by the toolchain config.
- `--platform native|wasm|egui` selects the platform profile.
- `--release` enables release mode in the toolchain config.
- `--asset NAME=PATH` adds a packaged asset. Pass the flag multiple times to
  include more than one asset.
- `--image NAME=PATH` adds a packaged image asset. Use this when the frontend
  should receive image bytes for message windows, portraits, or scene art.

Examples:

```bash
cargo run -p wmtoolchain -- samples/helloworld/main.wms
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out samples/helloworld/main.warc
cargo run -p wmtoolchain -- samples/easynovel/main.wms --package easynovel --platform native --step-limit 256
cargo run -p wmtoolchain -- samples/easynovel/main.wms --package easynovel --out build/easynovel.warc --asset ui/title=assets/title.bin
cargo run -p wmtoolchain -- samples/easynovel/main.wms --package easynovel --out build/easynovel.warc --image ui/background=assets/background.png
```

## Host Function Example

`CALL_HOST` uses a host function registered on the runtime side.

```rust
runtime.register_host_function(wmhost::HostFunction::new(1, 1, 1, 0), |args| {
    Ok(args.first().cloned().unwrap_or(wmvm::Value::Nil))
});
```

## Archive And Resource Example

- `Runtime::load_archive` loads a bundle into the runtime.
- `ResourceManager` exposes resource state and handles.
- Signed archives can be verified with `wmarchive::Archive::verify_signature`.

## Constraints

- The compiler currently handles a limited subset of WMScript.
- The examples in `samples/` are intentionally small and map to the runtime examples.
- When adding a new sample, keep its README and the corresponding runtime example in sync.
