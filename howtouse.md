# How To Use

This document mirrors the contents of `samples/` and the runtime examples used in this workspace.

## Samples

- `helloworld/`
  - Minimal script sample that returns a folded arithmetic expression.
- `inputlink/`
  - Host-driven input sample that returns a string from the runtime bridge.
- `workercomm/`
  - Two-worker message passing sample.
- `assetload/`
  - Runtime archive and resource loading sample.
- `easynovel/`
  - Small story-driven sample with chapters and narration.

## Hello World Sample

This sample is intentionally tiny. It exercises the compiler front end,
constant folding, and the VM runtime with a single exported entry point.

Source:

```wml
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

```wml
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

```wml
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

```wml
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
The runtime example chooses which chapter to run via a command-line argument.

Source:

```wml
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Aki arrives at the last train platform.";
}

export func chapter_1() {
    return "A lantern lights the stairs down to the station.";
}

export func chapter_2() {
    return "Aki chooses the quiet route home.";
}

export func main() {
    return "Prologue";
}
```

Notes:

- The current compiler can emit these functions directly because each body is a
  simple `return` expression.
- The sample is intentionally written to be easy to extend with branching later.

Run examples:

- `cargo run -p wmlruntime --example easynovel`
- `cargo run -p wmlruntime --example easynovel -- chapter_1`
- `cargo run -p wmlruntime --example easynovel -- chapter_2`

## Compile And Run

The runtime examples are the canonical way to exercise the current workspace.

```bash
cargo run -p wmlruntime --example hello_runtime
cargo run -p wmlruntime --example input_link
cargo run -p wmlruntime --example worker_comm
cargo run -p wmlruntime --example asset_load
cargo run -p wmlruntime --example easynovel
```

### Host Function Example

`CALL_HOST` uses a host function registered on the runtime side.

```rust
runtime.register_host_function(wmlhost::HostFunction::new(1, 1, 1, 0), |args| {
    Ok(args.first().cloned().unwrap_or(wmlvm::Value::Nil))
});
```

### Archive And Resource Example

- `Runtime::load_archive` loads a bundle into the runtime.
- `ResourceManager` exposes resource state and handles.
- Signed archives can be verified with `wmlarchive::Archive::verify_signature`.

## Constraints

- The compiler currently handles a limited subset of WMLScript.
- The examples in `samples/` are intentionally small and map to the runtime examples.
- When adding a new sample, keep its README and the corresponding runtime example in sync.

