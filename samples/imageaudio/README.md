# Image Audio Demo

This sample demonstrates loading and drawing images while starting audio playback.

Source:

```wms
export func main() {
    return ext.audio.playback(
        ext.audio.load(200),
        ext.image.draw(ext.image.load(100), 48, 48)
    );
}
```

Runtime behavior:

- The frontend loads two image assets and shows them in the side panel.
- The WML entrypoint loads an image, draws it, and starts looping audio.
- The script sample packages `samples/audio_and_images/sample01.jpg` as resource `100` and `sample.wav` as resource `200`.
- The `wmfrontend` built-in example still synthesizes its audio at runtime.

Run the script sample:

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

Run the built-in generated-asset example:

```powershell
cargo run -p wmfrontend --example image_audio_demo
```
