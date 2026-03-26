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

- `return;`
- `return <expr>;`

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
- grouping:
  - `(expr)`

The compiler performs constant folding and type tagging for this subset.

### 2.1 Current Limitations

- No `if` / `match` / `while` / `for` yet.
- No function call syntax in the script surface yet.
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

### 3.5 `ext.image`

Requires: `CAP_GUI`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.image.load` | `load(resource_id: int)` | `handle \| request_id` | Loads an image resource and returns a handle when ready. |
| `ext.image.info` | `info(handle)` | `table` | Returns resource id, type, size, and state metadata. |
| `ext.image.status` | `status(handle)` | `int` | Returns a numeric resource state code. |
| `ext.image.release` | `release(handle)` | `bool` | Releases the image handle. |

### 3.6 `ext.audio`

Requires: `CAP_ASYNC_IO`

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.audio.load` | `load(resource_id: int)` | `handle \| request_id` | Loads an audio resource and returns a handle when ready. |
| `ext.audio.play` | `play(handle, loop=false)` | `bool` | Starts or resumes playback. |
| `ext.audio.pause` | `pause(handle)` | `bool` | Pauses playback. |
| `ext.audio.stop` | `stop(handle)` | `bool` | Stops playback and rewinds to the beginning. |
| `ext.audio.seek` | `seek(handle, position_ms)` | `bool` | Moves the playback cursor. |
| `ext.audio.volume` | `volume(handle, value)` | `bool` | Updates playback volume. |
| `ext.audio.status` | `status(handle)` | `int` | Returns the current playback state code. |
| `ext.audio.release` | `release(handle)` | `bool` | Releases the audio handle. |

### 3.7 `ext.vm`

Requires: no capability

| Function | Signature | Returns | Notes |
| --- | --- | --- | --- |
| `ext.vm.save` | `save(slot: int)` | `bool` | Stores a runtime checkpoint in memory. |
| `ext.vm.load` | `load(slot: int)` | `bool` | Restores a previously stored checkpoint. |

## 4. VM-Level Execution Primitives

These are VM opcodes rather than surface-language functions, but they are part of the
current execution model and are useful to know when reading the runtime code.

- `send(worker_id, payload)` - queues a message to another worker
- `recv()` - waits for a message or yields waiting state
- `try_recv()` - reads a message if one is available
- `yield` - voluntarily yields the worker
- `sleep` - moves the worker into sleeping state

## 5. Practical Notes

- `wmltoolchain` compiles the current source subset into an archive.
- `wmlfrontend` can run the same project in `native`, `wasm`, or `egui` mode.
- The `egui` frontend defaults to a Japanese-friendly Noto Sans preset.

## 6. Examples

See:
- `samples/helloworld`
- `samples/inputlink`
- `samples/workercomm`
- `samples/assetload`
- `samples/easynovel`
