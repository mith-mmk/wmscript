# WMScript

Writer-First のゲームスクリプト実行基盤です。

first target は「サンプル script + assets から toolchain で package し、frontend/runtime で再現実行できること」です。

## Quick Start

### 1) サンプルを直接実行（egui）

```bash
cargo run -p wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto
```

### 2) toolchain で .warc を作る

```bash
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc
```

### 3) 生成した .warc を実行する

```bash
cargo run -p wmfrontend -- releases/helloworld-cycle.warc --platform native
```

## Main Entry Documents

- Samples run catalog: [samples/README.md](samples/README.md)
- Toolchain CLI guide: [crates/wmtoolchain/README.md](crates/wmtoolchain/README.md)
- Language/API surface: [functions.md](functions.md)
- Japanese language/API surface: [function.ja.md](function.ja.md)
- Specs: [SPEC](SPEC)
- Project status: [todo.md](todo.md)

## Workspace Layout

- `crates/`: runtime, frontend, compiler, archive, toolchain
- `samples/`: script samples and sample-specific docs
- `scripts/`: release/build scripts
- `SPEC/`: normative specs
