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
