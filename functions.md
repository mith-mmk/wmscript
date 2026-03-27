# WMLScript Language and Functions

This document summarizes the currently implemented surface language and the callable
runtime-facing functions exposed by the default runtime.

For the normative specifications, see:
- [SPEC/language.md](SPEC/language.md)
- [SPEC/vm.md](SPEC/vm.md)
- [SPEC/op.md](SPEC/op.md)
- [SPEC/hostapi.md](SPEC/hostapi.md)

## 1. Language Overview

WMLScript currently supports a small module-based script format:

- `import "path/to/module.wml";`
- `import "path/to/module.wml" as alias;`
- `export func name(params) { ... }`
- `export let name = literal;`

The compiler front end currently lowers a limited subset of function bodies:

- expression statements terminated by `;`
- `return;`
- `return <expr>;`
- `if expr { ... }`
- `if expr { ... } else { ... }`
- `recv();` to wait for the next message from the frontend or another worker

### 1.1 Module Example

```wml
import "shared/ui.wml" as ui;

export let title = "My Game";

export func main() {
    return 1 + 2 * 3;
}
```

## 2. Function Bodies

The current expression grammar is intentionally small:

- expression statements:
  - `expr;`
- conditionals:
  - `if expr { ... }`
  - `if expr { ... } else { ... }`
- literals:
  - `nil`
  - `true`
  - `false`
  - integer literals
  - floating-point literals
  - string literals
- unary:
  - `-expr`
- binary:
  - `expr + expr`
  - `expr - expr`
  - `expr * expr`
  - `expr / expr`
- comparison:
  - `expr == expr`
  - `expr != expr`
- grouping:
  - `(expr)`
- call expressions:
  - `ext.namespace.name(expr, ...)`
  - `recv()`
  - `try_recv()`
  - `yield()`
  - `sleep()`

The compiler performs constant folding and type tagging for this subset.

### 2.1 Current Limitations

- No `match` / `while` / `for` yet.
- No user-defined structs or classes yet.
- `export let` currently accepts literal values only.

## 3. Runtime-Facing Functions

The runtime installs a small set of extension namespaces under `ext.*`.
These are the callable entry points currently exposed by the default runtime.

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
| `ext.scene.reset` | `reset()` | `bool` | Restores the default scene layout. |

### 3.6 `ext.message`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.message.show` | `show(text: string)` or `show(speaker: string, text: string)` | `bool` | Shows a message window line and optional speaker name. |
| `ext.message.append` | `append(line: string)` | `bool` | Appends one line to the current message text and backlog. |
| `ext.message.choices` | `choices(label1, label2, ...)` | `bool` | Populates the current choice list shown in the message window. |
| `ext.message.prompt` | `prompt(text: string)` | `bool` | Sets the input prompt shown above the player input field. |
| `ext.message.hide` | `hide()` | `bool` | Hides the message window. |
| `ext.message.speed` | `speed(value)` | `bool` | Sets the text reveal speed used by the frontend message window. |
| `ext.message.auto` | `auto(enabled)` | `bool` | Enables or disables auto progression mode in the message window. |
| `ext.message.skip` | `skip(enabled)` | `bool` | Enables or disables skip mode in the message window. |
| `ext.message.clear` | `clear()` | `bool` | Clears the message window text, prompt, and choices. |

### 3.7 `ext.image`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.image.load` | `load(resource_id: int)` | `handle \| request_id` | Loads an image resource and returns a handle when ready. |
| `ext.image.info` | `info(handle)` | `table` | Returns resource id, type, size, and state metadata. |
| `ext.image.status` | `status(handle)` | `int` | Returns a numeric resource state code. |
| `ext.image.release` | `release(handle)` | `bool` | Releases the image handle. |
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

- `wmltoolchain` compiles the current source subset into an archive.
- `wmlfrontend` can run the same project in `native`, `wasm`, or `egui` mode.
- The `egui` frontend defaults to a Japanese-friendly Noto Sans preset.
- A simple read-flag convention works well with `state`: set keys like
  `read:chapter_1:0001` with `state.set(...)` and check them with
  `state.has(...)` when you decide whether to skip already-read content.
- For choice-driven branching, the frontend stores the selected choice id in
  `ui.last_choice`; a common pattern is to call `recv();` and then branch on
  `state.get("ui.last_choice")`.

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
