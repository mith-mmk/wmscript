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

## Writer-First 契約（first target）

本節は writer が frontend script に専念できる最小契約を定義する。
低レイヤの同期実装詳細は [SPEC/scheduler.md](scheduler.md) と [SPEC/hostapi.md](hostapi.md) を参照する。

### 1. recv()/message progression 契約
- `recv()` は「入力待機点」であり、次のいずれかで復帰する。
	- ユーザーによる advance（決定/クリック/タップ）
	- choice の確定
	- input prompt の submit
	- auto 進行の待機条件成立
- `ext.message.auto(true)` の待機は同一 time 基準を使う。time 基準の定義は [SPEC/scheduler.md](scheduler.md) に従う。
- `ext.message.skip(true)` の対象範囲は runtime 設定で read-only / all を切替可能とし、profile ごとの差分は実装定義とする。

### 2. input return ABI（標準キー）
- frontend 入力結果は `state` の以下キーに正規化して公開する。
	- `ui.last_choice`: `string | nil`
	- `ui.last_input`: `string | nil`
	- `ui.last_reply`: `table | nil`
- `ui.last_reply` の最小構造:
	- `kind`: `"advance" | "choice" | "input" | "auto"`
	- `choice`: `string | nil`
	- `input`: `string | nil`
- 同一 `recv()` 復帰サイクルで有効な最新値のみを保証する。履歴管理は script 側（例: backlog）で行う。

### 3. platform capability matrix（script 観点）

| capability | native | egui | wasm |
|---|---|---|---|
| gui | yes | yes | yes |
| input | yes | yes | yes |
| audio | yes | yes | profile dependent |
| file_system | yes | yes | no |
| network | yes | yes | profile dependent |
| async_io | yes | yes | profile dependent |

- コンパイラは対象 profile が未対応 capability を要求した `ext.*` 呼び出しを拒否してよい。
- matrix の詳細は host 実装ごとに [SPEC/hostapi.md](hostapi.md) で上書き可能だが、ここで定義する最小制約を破ってはならない。
