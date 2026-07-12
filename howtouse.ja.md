# WMScript v2 プロジェクトガイド

プロジェクト設定はルートの`wms.toml`だけを使用します。`package`、`game`、
`target`、`capabilities`、必要に応じて`[[asset]]`を記述してください。

```powershell
cargo run -p wms -- check samples/novel
cargo run -p wms -- test samples/novel
cargo run -p wms -- run samples/novel --target headless --inputs harbor
cargo run -p wms -- package samples/novel
```

未知key、重複asset ID/name、絶対path、`..`によるroot外参照はエラーです。
