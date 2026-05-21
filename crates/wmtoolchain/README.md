# wmtoolchain

WMScript を package archive (`.warc`) に変換する CLI です。

## Usage

```bash
cargo run -p wmtoolchain -- <script.wms> \
  [--package NAME] \
  [--out FILE] \
  [--step-limit N] \
  [--platform native|wasm|egui] \
  [--release] \
  [--asset NAME[@ID]=PATH] \
  [--image NAME[@ID]=PATH] \
  [--audio NAME[@ID]=PATH]
```

## Common Examples

```bash
# minimal
cargo run -p wmtoolchain -- samples/helloworld/main.wms

# explicit output
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc

# with package name and profile
cargo run -p wmtoolchain -- samples/easynovel/main.wms --package easynovel --platform native

# with binary asset
cargo run -p wmtoolchain -- samples/easynovel/main.wms --asset ui/title=assets/title.bin

# with image asset
cargo run -p wmtoolchain -- samples/easynovel/main.wms --image ui/background=assets/background.png

# with explicit resource ids
cargo run -p wmtoolchain -- samples/rpgdemo/engine/main.wms --image rpg/town-map@100=samples/rpgdemo/assets/town-map.png --audio rpg/stone-chime@203=samples/rpgdemo/assets/stone-chime.wav

# split scripts with nested imports
cargo run -p wmtoolchain -- samples/splitimport/main.wms --platform native

# run a packed archive directly
cargo run -p wmtoolchain --bin wmsruntime -- releases/helloworld-cycle.warc --platform native
```

## Direct Runtime Entry (`wmsruntime`)

```bash
cargo run -p wmtoolchain --bin wmsruntime -- <packed.warc> \
  [--platform native|wasm|egui] \
  [--step-limit N]
```

`wmsruntime` loads a packaged `.warc` archive and executes it immediately,
printing a runtime summary (package, worker, archive bytes, and last outcome).

## Output

成功時は summary を表示し、archive を出力します。

- package name
- archive byte size
- entry function
- output path

## Notes

- `--platform` は capability gate に影響します。
- profile 非対応の `ext.*` 呼び出しは compile 時に失敗します。
- `--asset` / `--image` / `--audio` は複数回指定できます。
- `NAME[@ID]=PATH` の `@ID` は省略可能です。省略時は従来どおり image/script-data が `100..`、audio が `200..` から自動採番されます。
- import を含む分割スクリプトは、entry script から再帰的に解決されます。
