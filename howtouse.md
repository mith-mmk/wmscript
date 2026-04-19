# How To Use

このページは導線だけを持つ軽量版です。実行コマンドの正本は `samples/README.md` です。

## Start Here

1. Workspace overview: [README.md](README.md)
2. Samples run catalog: [samples/README.md](samples/README.md)
3. Toolchain CLI: [crates/wmtoolchain/README.md](crates/wmtoolchain/README.md)
4. Language/API surface: [functions.md](functions.md)

## Quick Commands

```bash
# headless script run for AI/CI verification
cargo run -p wmfrontend --bin wmautoui -- samples/inputlink/main.wms --input AI-INPUT --expect AI-INPUT

# GUI run (when display environment is available)
cargo run -p wmfrontend --bin wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto

# build package
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc

# run package
cargo run -p wmfrontend --bin wmfrontend -- releases/helloworld-cycle.warc --platform native
```


