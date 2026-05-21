# Audio And Images Assets

このディレクトリは image/audio 系サンプルで使う参照アセット置き場です。

- `sample01.jpg`
- `sample02.jpg`
- `sample.wav`

実行は次を使ってください。

```bash
cargo run -p wmfrontend --bin wmfrontend -- --demo image-audio --platform egui --font noto
```

または script サンプルを使う場合:

```powershell
New-Item -ItemType Directory -Force .test-samples

cargo run -p wmtoolchain --bin wmtoolchain -- samples/imageaudio/main.wms `
  --package imageaudio `
  --platform egui `
  --image demo/sample@100=samples/audio_and_images/sample01.jpg `
  --audio demo/chime@200=samples/audio_and_images/sample.wav `
  --out .test-samples/imageaudio.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/imageaudio.warc `
  --platform egui `
  --expect-audio-resource 200 `
  --quiet
```
