# WMScript v2 Project Guide

Every project has one `wms.toml` and one entry source.

```toml
[package]
name = "my-game"
version = "0.1.0"
entry = "src/main.wms"

[game]
tick_hz = 60
seed = 1
save_compat_version = 1

[target]
default = "headless"

[capabilities]
allow = []
```

Unknown keys, duplicate asset IDs/names, absolute paths, and parent traversal are errors.
Use `wms check`, `wms test`, `wms run`, and `wms package` for every project.
