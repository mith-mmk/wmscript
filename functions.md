# WMScript Language and Functions

This document summarizes the currently implemented surface language and the callable
runtime-facing functions exposed by the default runtime.

For the normative specifications, see:
- [SPEC/language.md](SPEC/language.md)
- [SPEC/vm.md](SPEC/vm.md)
- [SPEC/op.md](SPEC/op.md)
- [SPEC/hostapi.md](SPEC/hostapi.md)

## 1. Language Overview

WMScript currently supports a small module-based script format:

- `import "path/to/module.wms";`
- `import "path/to/module.wms" as alias;`
- `export func name(params) { ... }`
- `export let name = literal;`

The compiler front end currently lowers a limited subset of function bodies:

- expression statements terminated by `;`
- local bindings with `let name = expr;`
- `return;`
- `return <expr>;`
- `if expr { ... }`
- `if expr { ... } else { ... }`
- `if expr { ... } else if expr { ... } else { ... }`
- `recv();` to wait for the next message from the frontend or another worker

### 1.1 Module Example

```wms
import "shared/ui.wms" as ui;

export let title = "My Game";

export func main() {
    return 1 + 2 * 3;
}
```

## 2. Function Bodies

The current expression grammar is intentionally small:

- expression statements:
  - `expr;`
- local bindings:
  - `let name = expr;`
- conditionals:
  - `if expr { ... }`
  - `if expr { ... } else { ... }`
  - `if expr { ... } else if expr { ... } else { ... }`
- literals:
  - `nil`
  - `true`
  - `false`
  - integer literals
  - floating-point literals
  - string literals
- unary:
  - `-expr`
  - `!expr`
- binary:
  - `expr + expr`
  - `expr - expr`
  - `expr * expr`
  - `expr / expr`
  - `expr && expr`
  - `expr || expr`
- comparison:
  - `expr == expr`
  - `expr != expr`
  - `expr < expr`
  - `expr <= expr`
  - `expr > expr`
  - `expr >= expr`
- grouping:
  - `(expr)`
- call expressions:
  - `ext.namespace.name(expr, ...)`
  - `recv()`
  - `try_recv()`
  - `yield()`
  - `sleep()`
- local variable references:
  - bare identifiers bound earlier in the same function body by `let`

The compiler performs constant folding and type tagging for this subset.
When extension metadata provides a return type hint, the compiler uses it for
type tagging of `ext.*` calls.

### 2.1 Current Limitations

- No `match` / `while` / `for` yet.
- No user-defined structs or classes yet.
- `export let` currently accepts literal values only.

## 3. Runtime-Facing Functions

The runtime installs a small set of extension namespaces under `ext.*`.
These are the callable entry points currently exposed by the default runtime.

### 3.0 Capability Gate

The compiler rejects `ext.*` calls when the selected platform profile does not
provide the required capability bit. In practice:

- `CAP_FILE_SYSTEM` is required by `ext.fs.*`
- `CAP_NETWORK` is required by `ext.net.*`
- `CAP_ASYNC_IO` is required by `ext.llm.*` and `ext.audio.*`
- `CAP_GUI` is required by `ext.scene.*`, `ext.message.*`, and `ext.image.*`
- `state.*` and `ext.vm.*` do not require a platform capability

Current default profiles:

| Profile | File system | Async I/O | GUI | Network | Web compat |
| --- | --- | --- | --- | --- | --- |
| `native` | yes | yes | yes | yes | no |
| `egui` | yes | yes | yes | yes | no |
| `wasm` | no | yes | no | no | yes |

If a script references an extension that is unavailable on the chosen profile,
compilation fails before bytecode is emitted.

### 3.1 `ext.fs`

Requires: `CAP_FILE_SYSTEM`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.fs.read` | `read(path: string)` | `string` | Reads a text file from the host file system. |
| `ext.fs.write` | `write(path: string, contents: string)` | `nil` | Writes a text file to the host file system. |
| `ext.fs.exists` | `exists(path: string)` | `bool` | Checks whether a path exists. |

### 3.2 `ext.debug`

Requires: no capability

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.debug.log` | `log(value)` | `nil` | Appends a rendered value to the runtime debug log. |
| `ext.debug.inspect` | `inspect(value)` | `string` | Returns a rendered textual representation. |

### 3.3 `ext.net`

Requires: `CAP_NETWORK`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.net.get` | `get(url: string)` | `string` | Performs a GET request through the configured network backend. |
| `ext.net.post` | `post(url: string, body: string)` | `string` | Performs a POST request through the configured network backend. |

### 3.4 `ext.llm`

Requires: `CAP_ASYNC_IO`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.llm.generate` | `generate(prompt: string)` | `string` | Sends a prompt to the configured LLM backend. |

### 3.5 `ext.scene`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.scene.layout` | `layout(choice_x, choice_y, choice_w, choice_h, message_x, message_y, message_w, message_h)` | `bool` | Sets the frontend scene layout used for choice and message panels. |
| `ext.scene.reset` | `reset()` | `bool` | Restores the default scene layout and clears the active message window and recorded image draws. |

### 3.6 `ext.message`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.message.show` | `show(text: string)` or `show(speaker: string, text: string)` | `bool` | Shows a message window line and optional speaker name. |
| `ext.message.append` | `append(line: string)` | `bool` | Appends one line to the current message text and backlog. |
| `ext.message.choices` | `choices()` or `choices(label1, label2, ...)` | `bool` | Populates the current choice list shown in the message window. Calling it with no args clears the current choices. |
| `ext.message.choices_named` | `choices_named()` or `choices_named(id1, label1, id2, label2, ...)` | `bool` | Populates the choice list using engine-defined stable choice ids. Calling it with no args clears the current choices. |
| `ext.message.prompt` | `prompt()` or `prompt(text: string)` | `bool` | Sets or clears the input prompt shown above the player input field. |
| `ext.message.hide` | `hide()` | `bool` | Hides the message window. |
| `ext.message.speed` | `speed(value)` | `bool` | Sets the text reveal speed used by the frontend message window. |
| `ext.message.auto` | `auto(enabled)` | `bool` | Enables or disables auto progression mode in the message window. |
| `ext.message.skip` | `skip(enabled)` | `bool` | Enables or disables skip mode in the message window. |
| `ext.message.log_clear` | `log_clear()` | `bool` | Clears only the text log/backlog while leaving the current page state untouched. |
| `ext.message.clear` | `clear()` | `bool` | Clears the message window text, prompt, and choices. |
| `ext.message.box_style` | `box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | Sets the message window panel fill and stroke colors. |
| `ext.message.text_color` | `text_color(r, g, b, a)` | `bool` | Sets the body text color. |
| `ext.message.speaker_color` | `speaker_color(r, g, b, a)` | `bool` | Sets the speaker-name color. |
| `ext.message.accent_color` | `accent_color(r, g, b, a)` | `bool` | Sets the accent color used for headings, hints, and emphasis. |
| `ext.message.font_size` | `font_size(body, speaker)` | `bool` | Sets the body and speaker font sizes used by the frontend message window. |
| `ext.message.reset_style` | `reset_style()` | `bool` | Restores the default message-window style preset. |
| `ext.message.frame` | `frame()` or `frame(resource_id)` | `bool` | Sets or clears the image resource used as the message-window frame. |
| `ext.message.content_inset` | `content_inset(left, top, right, bottom)` | `bool` | Sets the inner text region inset from the outer frame image. |
| `ext.message.input_box_style` | `input_box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | Sets the player-input panel fill and stroke colors. |
| `ext.message.input_text_color` | `input_text_color(r, g, b, a)` | `bool` | Sets the typed-text color used by the player input field. |
| `ext.message.input_hint_color` | `input_hint_color(r, g, b, a)` | `bool` | Sets the placeholder hint color used by the player input field. |
| `ext.message.input_prompt_color` | `input_prompt_color(r, g, b, a)` | `bool` | Sets the prompt label color shown above the player input field. |
| `ext.message.choice_box_style` | `choice_box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | Sets the choice-panel fill and stroke colors. |
| `ext.message.choice_text_color` | `choice_text_color(r, g, b, a)` | `bool` | Sets the choice label color. |
| `ext.message.choice_accent_color` | `choice_accent_color(r, g, b, a)` | `bool` | Sets the choice-panel heading and cursor accent color. |
| `ext.message.choice_selected_style` | `choice_selected_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | Sets the selected-choice row fill and stroke colors. |
### 3.7 `ext.image`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.image.load` | `load(resource_id: int)` | `handle \| request_id` | Loads an image resource and returns a handle when ready. |
| `ext.image.info` | `info(handle)` | `table` | Returns resource id, type, size, and state metadata. |
| `ext.image.status` | `status(handle)` | `int` | Returns a numeric resource state code. |
| `ext.image.release` | `release(handle)` | `bool` | Releases the image handle and removes draw calls or icon-sheet state tied to that handle. |
| `ext.image.draw` | `draw(handle, x, y)` | `bool` | Records an image draw call for the frontend renderer. |
| `ext.image.draw_part` | `draw_part(handle, sx, sy, sw, sh, dx, dy)` | `bool` | Records a sub-rectangle image draw call. |
| `ext.image.draw_ext` | `draw_ext(handle, sx, sy, sw, sh, dx, dy, dw, dh, rot, alpha)` | `bool` | Records an extended image draw call with scaling and rotation. |
| `ext.image.set_icon_sheet` | `set_icon_sheet(handle, cell_w, cell_h)` | `bool` | Stores sprite-sheet metadata for later icon draws. |
| `ext.image.draw_icon` | `draw_icon(handle, index, x, y)` | `bool` | Records a sprite draw call from the configured icon sheet. |

### 3.8 `ext.audio`

Requires: `CAP_ASYNC_IO`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.audio.load` | `load(resource_id: int)` | `handle \| request_id` | Loads an audio resource and returns a handle when ready. |
| `ext.audio.play` | `play(handle, loop=false)` | `bool` | Starts or resumes playback. |
| `ext.audio.playback` | `playback(handle, loop=false)` | `bool` | Alias for `play` used by the higher-level script surface. |
| `ext.audio.pause` | `pause(handle)` | `bool` | Pauses playback. |
| `ext.audio.stop` | `stop(handle)` | `bool` | Stops playback and rewinds to the beginning. |
| `ext.audio.seek` | `seek(handle, position_ms)` | `bool` | Moves the playback cursor. |
| `ext.audio.volume` | `volume(handle, value)` | `bool` | Updates playback volume. |
| `ext.audio.status` | `status(handle)` | `int` | Returns the current playback state code. |
| `ext.audio.release` | `release(handle)` | `bool` | Releases the audio handle. |

### 3.9 `ext.vm`

Requires: no capability

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.vm.save` | `save(slot: int)` | `bool` | Stores a runtime checkpoint in memory. |
| `ext.vm.load` | `load(slot: int)` | `bool` | Restores a previously stored checkpoint. |

### 3.10 `state`

Requires: no capability

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `state.save` | `save(slot: int)` | `bool` | Stores the current persistent key/value state into a slot. |
| `state.load` | `load(slot: int)` | `bool` | Restores the persistent key/value state from a slot. |
| `state.has` | `has(key: string)` | `bool` | Checks whether a key exists in the current state. |
| `state.get` | `get(key: string)` | `value` | Returns the current value for a key or `nil`. |
| `state.set` | `set(key: string, value)` | `bool` | Writes a value into the current state. |
| `state.erase` | `erase(key: string)` | `bool` | Removes a key from the current state. |

## 4. VM-Level Execution Primitives

These are VM opcodes rather than surface-language functions, but they are part of the
current execution model and are useful to know when reading the runtime code.

- `send(worker_id, payload)` - queues a message to another worker
- `recv()` - waits for a message or yields waiting state
- `try_recv()` - reads a message if one is available
- `yield()` - voluntarily yields the worker
- `sleep()` - moves the worker into sleeping state

## 5. Practical Notes

- `wmtoolchain` compiles the current source subset into an archive.
- `wmfrontend` can run the same project in `native`, `wasm`, or `egui` mode.
- The `egui` frontend defaults to a Japanese-friendly Noto Sans preset.
- A simple read-flag convention works well with `state`: set keys like
  `read:chapter_1:0001` with `state.set(...)` and check them with
  `state.has(...)` when you decide whether to skip already-read content.
- For engine-driven message windows, a practical pattern is to call
  `ext.message.choices_named(...)`, wait with `recv()`, then read
  `state.get("ui.last_choice")` / `state.get("ui.last_input")`.
- Message window colors and font sizes can also be authored from script with
  `ext.message.box_style(...)`, `text_color(...)`, `speaker_color(...)`,
  `accent_color(...)`, `font_size(...)`, and `reset_style()`.
- The frontend still mirrors the latest reply into `ui.last_choice`,
  `ui.last_input`, and `ui.last_reply` for compatibility with older samples.

## 6. Examples

See:
- `samples/helloworld`
- `samples/inputlink`
- `samples/workercomm`
- `samples/engineworker`
- `samples/assetload`
- `samples/imageaudio`
- `samples/uiimage`
- `samples/easynovel`








