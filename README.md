# WMScript v2

WMScript is a deterministic, gradually typed game language for visual novels,
RPGs, RTS games, and simulations. Source code compiles to the unchanged WMP1 /
bytecode v1 VM.

## Quick start

```powershell
cargo run -p wms -- new .test-my-game
cargo run -p wms -- check .test-my-game
cargo run -p wms -- test .test-my-game
cargo run -p wms -- run .test-my-game --target headless
cargo run -p wms -- package .test-my-game
```

Existing WARC v1 files are isolated behind:

```powershell
cargo run -p wms -- legacy run path/to/game.warc
```

## Workspace boundary

- Fixed VM layer: `wmvm`, `wmbytecode`, `wmverifier`
- v2 compiler: `wmcompiler::v2`
- deterministic world/runtime: `wmruntime::game`
- unified CLI and project pipeline: `wms`
- native report adapter: `wmfrontend::v2_adapter`
- examples: `samples/novel`, `samples/rpg`, `samples/rts`, `samples/simulation`

See [language specification](SPEC/language.md), [project guide](howtouse.md),
and [sample catalog](samples/README.md).
