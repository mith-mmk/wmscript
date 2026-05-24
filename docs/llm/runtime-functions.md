# Runtime Functions For LLMs

This page summarizes the runtime-facing calls that are documented for the
current draft surface. For the longer table, use [../../functions.md](../../functions.md).
For exact registration details, inspect the current Rust extension registry.

## Capability Model

The compiler may reject `ext.*` calls if the selected platform profile does not
provide the required capability.

| Profile | File system | Async I/O | GUI | Network | Web compat |
| --- | --- | --- | --- | --- | --- |
| `native` | yes | yes | yes | yes | no |
| `egui` | yes | yes | yes | yes | no |
| `wasm` | no | yes | no | no | yes |

Documented capability expectations:

- `ext.fs.*`: file system.
- `ext.net.*`: network.
- `ext.llm.*`: async I/O.
- `ext.audio.*`: async I/O.
- `ext.scene.*`, `ext.message.*`, `ext.image.*`: GUI.
- `state.*`, `ext.vm.*`, `ext.automation.*`, `ext.rts.*`: no platform
  capability in the documented default profile.

## Message UI: `ext.message`

Use this namespace for writer-facing message window flows.

Common calls:

- `ext.message.show(text)` or `ext.message.show(speaker, text)`
- `ext.message.append(line)`
- `ext.message.choices(label1, label2, ...)`
- `ext.message.choices_named(id1, label1, id2, label2, ...)`
- `ext.message.prompt()` or `ext.message.prompt(text)`
- `ext.message.clear()`
- `ext.message.log_clear()`
- `ext.message.hide()`
- `ext.message.speed(value)`
- `ext.message.auto(enabled)`
- `ext.message.skip(enabled)`
- Styling calls such as `box_style`, `text_color`, `speaker_color`,
  `accent_color`, `font_size`, `reset_style`, `frame`, `content_inset`,
  `input_*`, and `choice_*`.

Typical pattern:

```wms
ext.message.show("Guide", "Choose a route.");
ext.message.choices_named("north", "Go North", "south", "Go South");
recv();
let route = state.get("ui.last_choice");
ext.message.choices_named();
```

## Persistent State: `state`

Use `state.*` for script-visible key/value state.

- `state.save(slot)`
- `state.load(slot)`
- `state.has(key)`
- `state.get(key)`
- `state.set(key, value)`
- `state.erase(key)`

Frontend replies are normalized into state keys:

- `ui.last_choice`
- `ui.last_input`
- `ui.last_reply`

## Scene: `ext.scene`

Use scene calls for coarse frontend presentation:

- `ext.scene.layout(...)`
- `ext.scene.reset()`
- `ext.scene.opening(title)`
- `ext.scene.ending(title)`

Check [../../functions.md](../../functions.md) and current samples before using
less common scene calls.

## Image: `ext.image`

Use image calls with resource ids or handles supplied by the packaged project:

- `ext.image.load(resource_id)`
- `ext.image.info(handle)`
- `ext.image.status(handle)`
- `ext.image.release(handle)`
- `ext.image.draw(handle, x, y)`
- `ext.image.draw_part(handle, sx, sy, sw, sh, dx, dy)`
- `ext.image.draw_ext(handle, sx, sy, sw, sh, dx, dy, dw, dh, rot, alpha)`
- `ext.image.set_icon_sheet(handle, cell_w, cell_h)`
- `ext.image.draw_icon(handle, index, x, y)`

Resource ids are usually assigned by `wmtoolchain` options such as
`--image name@100=path`.

## Audio: `ext.audio`

Use audio calls with packaged audio resources:

- `ext.audio.load(resource_id)`
- `ext.audio.play(handle, loop=false)`
- `ext.audio.playback(handle, loop=false)`
- `ext.audio.pause(handle)`
- `ext.audio.stop(handle)`
- `ext.audio.seek(handle, position_ms)`
- `ext.audio.volume(handle, value)`
- `ext.audio.status(handle)`
- `ext.audio.release(handle)`

Codec support depends on the frontend/backend.

## File System, Network, LLM, Debug

File system:

- `ext.fs.read(path)`
- `ext.fs.write(path, contents)`
- `ext.fs.exists(path)`

Network:

- `ext.net.get(url)`
- `ext.net.post(url, body)`

LLM:

- `ext.llm.generate(prompt)`

Debug:

- `ext.debug.log(value)`
- `ext.debug.inspect(value)`

Do not use file system or network calls in web-compatible examples unless the
target profile supports them.

## VM Checkpoints: `ext.vm`

Use `ext.vm.save(slot)` and `ext.vm.load(slot)` for runtime checkpoints.

For long-term game progress, prefer `state.save(slot)` and `state.load(slot)`
unless a spec or sample requires VM-level checkpointing.

## Gameplay Helpers

The current documented helper namespaces include:

- `ext.automation.*` for resource counters and deterministic production jobs.
- `ext.rts.*` for simple unit state, movement, and damage.

Use [../../functions.md](../../functions.md) as the detailed reference for these
helpers. Treat branch-local or newly added helper namespaces as unstable until
they are documented in the root reference or `SPEC/`.
