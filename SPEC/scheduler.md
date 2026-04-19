# WML Scheduler 仕様

## 責務
- runnable worker の順番付けと step budget 管理を定義する。
- `running / waiting / sleeping / halted / error` の遷移を定義する。
- completion queue と message queue の再開条件を定義する。

## 依存
- worker 状態は [SPEC/vm.md](vm.md) に依存する。
- host 完了イベントは [SPEC/hostapi.md](hostapi.md) に依存する。
- package = execution unit の対応は [SPEC/language.md](language.md) に依存する。

## 仕様要点
- `spawn -> run -> destroy` を worker lifecycle の基本とする。
- `yield` は runnable から scheduler へ制御を返す。
- `sleep` は wake 時刻まで sleeping に遷移する。
- `recv` はメッセージが無ければ waiting に遷移する。
- 1 フレームの実行は step budget を超えない。
- `tick` は 1 回の scheduling round を実行する。
- sleeping / waiting worker は `wake(worker_id)` で再実行可能状態へ戻せる。
- `sleep` は VM 内部では一時停止状態を維持し、`wake` 後に次命令から再開する。

## Writer-First 契約（input routing と clock）

### 1. worker 間 input routing
- frontend worker は入力イベントを標準メッセージに正規化して送る。
- 標準経路:
	- `frontend -> middleware(input_router) -> backend/engine`
- middleware 未配置時は `frontend -> backend/engine` 直送を許可する。
- `recv()` 待機 worker の再開は、次のいずれかの到達で行う。
	- `advance`
	- `choice`
	- `input`
	- `auto`

### 2. scheduler clock 契約
- `tick` は simulation clock を進める最小単位。
- `sleep` と message auto progression は同一 simulation clock を参照する。
- wall-clock と同期するかは host 実装定義だが、script 観点では `tick` 進行順序の再現性を優先する。
