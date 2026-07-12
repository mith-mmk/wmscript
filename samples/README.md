# WMScript v2 Samples

All samples use the same `wms.toml` project contract and unified CLI.

```powershell
cargo run -p wms -- check samples/novel
cargo run -p wms -- test samples/novel
cargo run -p wms -- run samples/novel --target headless --inputs harbor
cargo run -p wms -- package samples/novel
```

Repeat the same commands for `rpg`, `rts`, and `simulation`. Generated files are
written below each project's `.test-wms/` directory, which is covered by the
repository `.gitignore` through the `.test*` rule.
