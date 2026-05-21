# Asset Load Sample

This sample shows a bundled asset being packaged into an archive and then loaded
through the runtime resource manager.

Source:

```wms
export func main() {
    asset.preload(100);
    return "assetload-ok";
}
```

Runtime behavior:

- The archive contains a single asset resource.
- The runtime loads the archive and resolves the resource by id.
- The resource bytes become available through the resource manager.

Run:

```powershell
New-Item -ItemType Directory -Force .test-samples

cargo run -p wmtoolchain --bin wmtoolchain -- samples/assetload/main.wms `
  --package assetload `
  --platform egui `
  --asset data/payload@100=samples/assetload/payload.txt `
  --out .test-samples/assetload.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/assetload.warc `
  --platform egui `
  --expect assetload-ok `
  --quiet
```
