# Toolchain Novel Sample

Writer-First 契約と toolchain 導線を確認するための、日本語ノベルゲームサンプルです。
表示テキストと選択肢は日本語、choice id と自動テスト用の戻り値は ASCII のままにしています。

- `guide.txt` を resource `100`、`samples/uiimage.png` を背景 resource `101` として `.warc` に同梱します
- スクリプト先頭で `ext.scene.reset()` と `ext.image.draw_ext(ext.image.load(101), ...)` を呼び、背景画像を描画します
- メッセージページは明示的な `recv()` でだけ進みます
- 選択肢は `ext.message.choices_named(...)` の安定 id を使います
- 選択結果は `state.get("ui.last_choice")` から読みます
- テキスト入力は `state.get("ui.last_input")` から読みます
- 永続状態は `state.save(1)`、実行チェックポイントは `ext.vm.save(1)` を使います

現時点の compiler は script からの汎用 `load_asset(...)` を公開 API として解決しません。
そのため、テキスト asset の同梱は toolchain/package 側の契約として確認します。
画像 asset は `ext.image.load(101)` で読み込み、script 側からも描画契約を確認します。

## Web Distribution Smoke

`ui/background` は Web 配信最適化用の smoke corpus でもあります。
toolchain のテストでは、この画像 section が `.warc` 内に互換用 payload として残りつつ、
manifest に外部 location `assets/uiimage.png` / cache key `sha256:toolchainnovel-bg` を持つこと、
および section digest で payload 検証できることを確認します。

## Pipeline

```powershell
New-Item -ItemType Directory -Force .test-toolchainnovel

cargo run -p wmtoolchain --bin wmtoolchain -- samples/toolchainnovel/main.wms `
  --package toolchainnovel `
  --platform egui `
  --asset story/guide=samples/toolchainnovel/guide.txt `
  --image ui/background=samples/uiimage.png `
  --out .test-toolchainnovel/toolchainnovel.warc

cargo run -p wmfrontend --bin wmautoui -- .test-toolchainnovel/toolchainnovel.warc `
  --platform egui `
  --choice repair `
  --input lumen `
  --expect signal-restored

cargo run -p wmfrontend -- .test-toolchainnovel/toolchainnovel.warc --platform egui --font noto
```

`.test-toolchainnovel` は repo-wide の `.test*` ルールで `.gitignore` 対象です。
ローカル確認後に削除できます。
