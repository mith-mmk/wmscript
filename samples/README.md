# Samples Run Catalog

このファイルはサンプル実行の単一入口です。コピーして動く smoke コマンドだけを載せています。
archive を作るサンプルはすべて `.test-samples/` に出力します。

## Setup

```powershell
New-Item -ItemType Directory -Force .test-samples
```

## Basic Scripts

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/helloworld/main.wms --platform native

cargo run -p wmfrontend --bin wmautoui -- samples/inputlink/main.wms `
  --platform egui `
  --input AI-INPUT `
  --expect AI-INPUT `
  --quiet

cargo run -p wmfrontend --bin wmfrontend -- samples/workercomm/main.wms --platform native

cargo run -p wmruntime --example worker_comm

cargo run -p wmfrontend --bin wmautoui -- samples/splitimport/main.wms `
  --platform egui `
  --expect "split import ok" `
  --quiet
```

## Message UI

```powershell
cargo run -p wmfrontend --bin wmautoui -- samples/messagewindow/main.wms `
  --platform egui `
  --choice north `
  --input Mika `
  --quiet

cargo run -p wmfrontend --bin wmautoui -- samples/engineworker/main.wms `
  --platform egui `
  --choice prologue `
  --input Aki `
  --quiet
```

## Asset / Image / Audio

```powershell
cargo run -p wmtoolchain --bin wmtoolchain -- samples/assetload/main.wms `
  --package assetload `
  --platform egui `
  --asset data/payload@100=samples/assetload/payload.txt `
  --out .test-samples/assetload.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/assetload.warc `
  --platform egui `
  --expect assetload-ok `
  --quiet

cargo run -p wmtoolchain --bin wmtoolchain -- samples/uiimage/main.wms `
  --package uiimage `
  --platform egui `
  --image ui/background@100=samples/uiimage.png `
  --out .test-samples/uiimage.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/uiimage.warc `
  --platform egui `
  --expect "UI image layout demo" `
  --expect-image-resource 100 `
  --quiet

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

## Novel / Game Samples

```powershell
cargo run -p wmtoolchain --bin wmtoolchain -- samples/easynovel/main.wms `
  --package easynovel `
  --platform egui `
  --image ui/message_frame@100=samples/easynovel/message_frame.png `
  --out .test-samples/easynovel.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/easynovel.warc `
  --platform egui `
  --choice prologue `
  --quiet

cargo run -p wmtoolchain --bin wmtoolchain -- samples/novelgame/engine/main.wms `
  --package novelgame `
  --platform egui `
  --ui samples/novelgame/ui/main.wms `
  --loader samples/novelgame/loader/main.wms `
  --image scene/common@100=samples/novelgame/background.png `
  --image scene/sea@101=samples/novelgame/sea.png `
  --image scene/shelf@102=samples/novelgame/shelf.png `
  --image scene/lamp@103=samples/novelgame/lamp.png `
  --out .test-samples/novelgame.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/novelgame.warc `
  --platform egui `
  --choice sea `
  --expect ending-fog-harbor `
  --expect-image-resource 101 `
  --quiet

cargo run -p wmtoolchain --bin wmtoolchain -- samples/toolchainnovel/main.wms `
  --package toolchainnovel `
  --platform egui `
  --asset story/guide@100=samples/toolchainnovel/guide.txt `
  --image ui/background@101=samples/uiimage.png `
  --out .test-samples/toolchainnovel.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/toolchainnovel.warc `
  --platform egui `
  --choice repair `
  --input lumen `
  --expect signal-restored `
  --quiet
```

## Gameplay Samples

```powershell
cargo run -p wmtoolchain --bin wmtoolchain -- samples/automationrts/main.wms `
  --package automationrts `
  --platform egui `
  --out .test-samples/automationrts.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/automationrts.warc `
  --platform egui `
  --choices tick,build `
  --expect automation-rts-built `
  --quiet

cargo run -p wmtoolchain --bin wmtoolchain -- samples/rpgdemo/engine/main.wms `
  --package rpgdemo `
  --platform egui `
  --ui samples/rpgdemo/ui/main.wms `
  --loader samples/rpgdemo/loader/main.wms `
  --image rpg/town-map@100=samples/rpgdemo/assets/town-map.png `
  --image rpg/forest-map@101=samples/rpgdemo/assets/forest-map.png `
  --image rpg/dungeon-map@102=samples/rpgdemo/assets/dungeon-map.png `
  --image rpg/battle-slime@103=samples/rpgdemo/assets/battle-slime.png `
  --image rpg/actor-icons@104=samples/rpgdemo/assets/actor-icons.png `
  --image rpg/landmark-icons@105=samples/rpgdemo/assets/landmark-icons.png `
  --image rpg/dungeon-view@106=samples/rpgdemo/assets/dungeon-view.png `
  --audio rpg/stone-chime@203=samples/rpgdemo/assets/stone-chime.wav `
  --audio rpg/battle-hit@204=samples/rpgdemo/assets/battle-hit.wav `
  --out .test-samples/rpgdemo.warc

cargo run -p wmfrontend --bin wmautoui -- .test-samples/rpgdemo.warc `
  --platform egui `
  --choices east,east,forest,south,south,forward,turn_right,forward,check,end_demo `
  --expect rpg-dungeon-depth `
  --quiet
```

## Interactive Launch

GUI で触る場合は、project config を持つサンプルは directory 指定で起動できます。

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/novelgame
cargo run -p wmfrontend --bin wmfrontend -- samples/rpgdemo
```

## Cleanup

```powershell
Remove-Item -LiteralPath .test-samples -Recurse -Force
```

## Notes

- `wmautoui` は script/archive の自動応答用です。asset/image/audio を渡す smoke は先に `wmtoolchain` で `.warc` を作ってから実行します。
- `NAME@ID=PATH` は resource id を固定する書式です。省略時は従来どおり image/script-data が `100..`、audio が `200..` から自動採番されます。
- サンプル個別の挙動は各ディレクトリの `README.md` も参照してください。
