# Samples Run Catalog

このファイルはサンプル実行の単一入口です。

## Run Matrix

| Sample | Purpose | Script Run (wmfrontend) | Demo/Example Run | Toolchain Build |
| --- | --- | --- | --- | --- |
| `helloworld` | 定数畳み込みの最小例 | `cargo run -p wmfrontend -- samples/helloworld/main.wms --platform native` | `cargo run -p wmruntime --example hello_runtime` | `cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc` |
| `inputlink` | ホスト入力連携 | `cargo run -p wmfrontend -- samples/inputlink/main.wms --platform native` | `cargo run -p wmruntime --example input_link` | `cargo run -p wmtoolchain -- samples/inputlink/main.wms` |
| `workercomm` | worker 間通信 | `cargo run -p wmfrontend -- samples/workercomm/main.wms --platform native` | `cargo run -p wmruntime --example worker_comm` | `cargo run -p wmtoolchain -- samples/workercomm/main.wms` |
| `engineworker` | engine 主導 message/choice/input | `cargo run -p wmfrontend -- samples/engineworker/main.wms --platform egui --font noto` | `cargo run -p wmfrontend -- --demo engineworker --platform egui --font noto` | `cargo run -p wmtoolchain -- samples/engineworker/main.wms --platform egui` |
| `messagewindow` | message window 専用検証 | `cargo run -p wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto` | `cargo run -p wmfrontend -- --demo messagewindow --platform egui --font noto` | `cargo run -p wmtoolchain -- samples/messagewindow/main.wms --platform egui` |
| `assetload` | archive/resource load | `cargo run -p wmfrontend -- samples/assetload/main.wms --platform native` | `cargo run -p wmruntime --example asset_load` | `cargo run -p wmtoolchain -- samples/assetload/main.wms` |
| `uiimage` | scene/layout + image draw | `cargo run -p wmfrontend -- samples/uiimage/main.wms --platform egui --font noto --image ui/background=samples/uiimage.png` | `cargo run -p wmfrontend -- --demo uiimage --platform egui --font noto` | `cargo run -p wmtoolchain -- samples/uiimage/main.wms --platform egui --image ui/background=samples/uiimage.png` |
| `imageaudio` | image + audio 統合 | `cargo run -p wmfrontend -- samples/imageaudio/main.wms --platform egui --font noto` | `cargo run -p wmfrontend --example image_audio_demo` | `cargo run -p wmtoolchain -- samples/imageaudio/main.wms --platform egui` |
| `easynovel` | writer-first VN flow | `cargo run -p wmfrontend -- samples/easynovel/main.wms --platform egui --font noto --image ui/message_frame=samples/easynovel/message_frame.png` | `cargo run -p wmruntime --example easynovel` | `cargo run -p wmtoolchain -- samples/easynovel/main.wms --platform egui --image ui/message_frame=samples/easynovel/message_frame.png` |
| `splitimport` | 分割スクリプト + nested import | `cargo run -p wmfrontend -- samples/splitimport/main.wms --platform native` | - | `cargo run -p wmtoolchain -- samples/splitimport/main.wms --platform native` |

## End-to-End Pipeline

```bash
# 1) build archive
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc

# 2) run archive directly
cargo run -p wmfrontend -- releases/helloworld-cycle.warc --platform native
```

## Notes

- `wmfrontend` は `<script.wms>` と `<archive.warc>` の両方を受け付けます。
- `--demo` は script ファイル不要の組み込みデモです。
- writer-first 契約に合わせ、choice/input は `recv()` 後に `state.get("ui.last_choice")` / `state.get("ui.last_input")` を読む実装を推奨します。
- サンプル個別の挙動は各ディレクトリの `README.md` を参照してください。
