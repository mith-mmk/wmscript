# wmtoolchain legacy library

`wmtoolchain` no longer exposes a CLI. Its library remains only to decode and
execute WARC v1 packages through `wms legacy run`.

New projects use the `wms` crate, `wms.toml`, the v2 compiler, and WARC v2:

```powershell
cargo run -p wms -- check samples/novel
cargo run -p wms -- package samples/novel
```
