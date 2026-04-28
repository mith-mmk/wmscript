# Novel Game Sample

`SPEC/spec-append.md` の package 分離に合わせた、日本語ノベルゲーム本文サンプルです。

- `engine/main.wms` は物語、分岐、ending、scene image の切替だけを担当します
- `ui/main.wms` は text box、choice layout、右クリック menu、Shift fast を設定します
- `loader/main.wms` は v1 の最小 asset preload package です
- `wmfrontend.toml` で `ui` / `loader` / `engine` packages と4つの scene image を明示します
- 選択結果は `recv()` 後に `state.get("ui.last_choice")` から読みます
- 戻り値は smoke run で確認しやすいよう ASCII の固定文字列です

## Routes

| Choice ID | 表示ラベル | Scene image | Return |
| --- | --- | --- | --- |
| `sea` | 小舟の灯りを追う | `scene/sea` | `ending-fog-harbor` |
| `shelf` | 封印棚を開ける | `scene/shelf` | `ending-blank-catalog` |
| `lamp` | 灯台の火を守る | `scene/lamp` | `ending-keeper-light` |

## Run

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/novelgame
```

## Auto UI Smoke

```powershell
New-Item -ItemType Directory -Force .test-novelgame

cargo run -p wmtoolchain --bin wmtoolchain -- samples/novelgame/engine/main.wms `
  --package novelgame `
  --platform egui `
  --ui samples/novelgame/ui/main.wms `
  --loader samples/novelgame/loader/main.wms `
  --image scene/common=samples/novelgame/background.png `
  --image scene/sea=samples/novelgame/sea.png `
  --image scene/shelf=samples/novelgame/shelf.png `
  --image scene/lamp=samples/novelgame/lamp.png `
  --out .test-novelgame/novelgame.warc

cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice sea --expect ending-fog-harbor --expect-image-resource 101
cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice shelf --expect ending-blank-catalog --expect-image-resource 102
cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice lamp --expect ending-keeper-light --expect-image-resource 103
```

## Assets

`scene/common`, `scene/sea`, `scene/shelf`, `scene/lamp` を別 resource として package します。
現在の route 画像は smoke 用の初期アセットなので、見た目の差分は同じ名前で差し替えできます。
