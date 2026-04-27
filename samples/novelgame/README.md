# Novel Game Sample

選択肢でエンディングが変わる、日本語ノベルゲーム本文サンプルです。

- `ext.message.choices_named(...)` で安定した choice id を使います
- `background.png` を resource `100` として読み込み、背景に描画します
- 選択結果は `recv()` 後に `state.get("ui.last_choice")` から読みます
- 各ルートは `state.save(1)` で最後に到達したエンディングを保存します
- 自動テストや smoke run で確認しやすいよう、戻り値は ASCII の固定文字列です

## Routes

| Choice ID | 表示ラベル | Ending | Return |
| --- | --- | --- | --- |
| `sea` | 小舟の灯りを追う | Ending A: 霧の帰港 | `ending-fog-harbor` |
| `shelf` | 封印棚を開ける | Ending B: 白紙の目録 | `ending-blank-catalog` |
| `lamp` | 灯台の火を守る | Ending C: 灯を継ぐ司書 | `ending-keeper-light` |

## Run

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/novelgame/main.wms --platform egui --font noto --image ui/background=samples/novelgame/background.png
```

## Auto UI Smoke

```powershell
New-Item -ItemType Directory -Force .test-novelgame

cargo run -p wmtoolchain --bin wmtoolchain -- samples/novelgame/main.wms `
  --package novelgame `
  --platform egui `
  --image ui/background=samples/novelgame/background.png `
  --out .test-novelgame/novelgame.warc

cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice sea --expect ending-fog-harbor
cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice shelf --expect ending-blank-catalog
cargo run -p wmfrontend --bin wmautoui -- .test-novelgame/novelgame.warc --platform egui --choice lamp --expect ending-keeper-light
```

## Asset

`background.png` は、岬の灯台図書館を描いた 16:9 向け背景です。
生成時のプロンプトは、夜の灯台図書館、霧の海、小舟の灯り、暖かな窓明かりを指定しています。
