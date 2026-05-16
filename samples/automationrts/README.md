# Automation RTS Demo

`SPEC/gameplay.md` の automation / RTS profile に合わせた deterministic sample です。
VM opcode は増やさず、`ext.automation.*` と `ext.rts.*` が `state.*` の標準キーを更新します。

- `ext.automation.set_resource/add_resource/resource` は `resource.<id>` を扱います。
- `ext.automation.set_job/enable_job/tick` は `job.<id>.*` と `game.tick` を更新します。
- `ext.rts.set_unit/move_unit/damage_unit` は `unit.<id>.*` と `rts.units` を更新します。
- `wmautoui` で同じ choice id を流せるよう、すべての結果は deterministic です。

## Run

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/automationrts/main.wms --platform egui --font noto
```

## Auto UI Smoke

```powershell
New-Item -ItemType Directory -Force .test-automationrts

cargo run -p wmtoolchain --bin wmtoolchain -- samples/automationrts/main.wms `
  --package automationrts `
  --platform egui `
  --out .test-automationrts/automationrts.warc

cargo run -p wmfrontend --bin wmautoui -- .test-automationrts/automationrts.warc --platform egui --choices tick,build --expect automation-rts-built
cargo run -p wmfrontend --bin wmautoui -- .test-automationrts/automationrts.warc --platform egui --choice scout --expect automation-rts-scouted
```
