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
- The audio asset is synthesized in the `wmfrontend` example at runtime.

Run it with:

```bash
cargo run -p wmfrontend --example image_audio_demo
```
