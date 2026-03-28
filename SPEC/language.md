# WMScript 言語仕様

## 責務
- モジュール、パッケージ、バンドルの構造を定義する。
- 文法、型、式、制御構文、組み込み API の見え方を定義する。
- import 解決、シンボル解決、ID 割当の静的ルールを定義する。
- 実行モデルとして `init -> update -> on_message` の呼び出し順を定義する。

## 依存
- 実行モデルは [SPEC/vm.md](vm.md) に依存する。
- 命令セットは [SPEC/op.md](op.md) に依存する。
- `CALL_HOST` の公開境界は [SPEC/hostapi.md](hostapi.md) に依存する。
- `worker` と `package` の実行単位は [SPEC/scheduler.md](scheduler.md) に依存する。
- 文字列・アセットの分離は [SPEC/resource.md](resource.md) と [SPEC/archive.md](archive.md) に依存する。

## 仕様検証メモ
- `init` はモジュール初期化時に一度だけ呼ぶ。
- `update` は協調スケジューリングのフレーム単位で呼ぶ。
- `on_message` は受信キューにメッセージが入った時のみ呼ぶ。
- `spawn -> run -> destroy` の worker lifecycle を前提にする。
- handle の解放は参照カウントまたは所有権消失時に host 側で回収する。
- エラーは例外にせず `nil` / status table / code で返す。
- import 解決と ID 割当はコンパイル時に完結させる。
