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
  [--asset NAME=PATH] \
  [--image NAME=PATH]
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
```

## Output

成功時は summary を表示し、archive を出力します。

- package name
- archive byte size
- entry function
- output path

## Notes

- `--platform` は capability gate に影響します。
- profile 非対応の `ext.*` 呼び出しは compile 時に失敗します。
- `--asset` / `--image` は複数回指定できます。
