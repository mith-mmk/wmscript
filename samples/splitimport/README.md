# Split Import Sample

This sample validates split-script compile flow with nested imports.

- `main.wms` imports `chapter/part1.wms`
- `chapter/part1.wms` imports `chapter/part2.wms`

Run examples:

```bash
# compile/package (C1 target)
cargo run -p wmtoolchain -- samples/splitimport/main.wms --platform native

# run script directly
cargo run -p wmfrontend -- samples/splitimport/main.wms --platform native
```
