# WMScript v2 実装 TODO

## ステータス

- `[ ]`: 未着手
- `[*]`: 実装中
- `[+]`: コード実装済み・最終確認前
- `[x]`: テストと人間による動作確認が完了
- `[-]`: 実装見合わせ、`SPEC/issue.md`へ移動

各項目は上から順に実行し、「完了条件」を満たすまで次へ進まない。

## 1. VM三層の固定

- [+] `wmvm`・`wmbytecode`・`wmverifier`のbaseline固定
  - 入力: 現行公開API、bytecode v1、既存118テスト
  - 出力: VM公開APIとbinary codecのgolden回帰テスト
  - 完了条件: 三crateの実装を変更せず`cargo test --workspace`が通る

## 2. v2仕様

- [+] 言語・型・task/event/system仕様
  - 入力: `SPEC/language.md`、`SPEC/vm.md`、`SPEC/op.md`
  - 出力: v2構文、型規則、待機可能性、VM lowering契約
  - 完了条件: VM/opcode追加なしで全構文のlowering先が定義される
- [+] world・固定tick・save仕様
  - 入力: `SPEC/gameplay.md`、`SPEC/scheduler.md`、`SPEC/hostapi.md`
  - 出力: entity/component/resource/event、決定順序、永続化境界
  - 完了条件: 同一seedと入力列から同一状態になる規則が定義される
- [+] project・WARC v2・legacy仕様
  - 入力: `SPEC/archive.md`、既存WARC v1
  - 出力: `wms.toml`とWARC v2、v1読込専用経路
  - 完了条件: v2生成とv1読込の責務が分離される

## 3. コンパイラ

- [+] lexer・parser・diagnostic
  - 入力: v2言語仕様
  - 出力: span付きtoken、AST、複数diagnostic
  - 完了条件: 全宣言・式・制御構文の正常系と異常系テストが通る
- [+] resolver・型検査・typed IR
  - 入力: AST、標準モジュールsignature
  - 出力: 解決済みsymbol、推論済み型、typed IR
  - 完了条件: 型不一致、未知symbol、待機禁止位置をcompile errorにできる
- [+] VM bytecode lowering
  - 入力: typed IR、bytecode v1
  - 出力: `wmvm::Program`、task再開状態、event entry table
  - 完了条件: VM三層を変更せず関数・構造体・collection・taskが実行できる

## 4. ゲームランタイム

- [+] Worldとイベントキュー
  - 入力: component/resource/event schema
  - 出力: Entity ID、型付きcomponent store、resource、FIFO event queue
  - 完了条件: queryとevent配信が常に決定的な順序になる
- [+] 固定tick・乱数・save/load
  - 入力: `tick_hz`、seed、永続schema
  - 出力: fixed-step scheduler、seed RNG、永続snapshot
  - 完了条件: replayとsave/load round-tripで同じworld状態になる
- [+] 標準portとmodule
  - 入力: runtime capability
  - 出力: input/render/audio/storage port、標準module dispatch
  - 完了条件: headlessとeguiが同じruntime coreを利用できる

## 5. Project・配布・CLI

- [+] `wms.toml`
  - 入力: project仕様
  - 出力: strict manifest loader、相対path解決、asset ID検査
  - 完了条件: 未知key、重複ID、不正pathをdiagnosticにできる
- [+] WARC v2とlegacy v1
  - 入力: v2 manifest、bytecode v1、既存WARC v1
  - 出力: v2 writer/reader、隔離されたv1 reader
  - 完了条件: v2 round-tripと生成したv1 fixtureのlegacy実行が通る
- [+] 統一`wms` CLI
  - 入力: compiler、runtime、project、archive
  - 出力: `new/check/build/run/test/package/legacy run`
  - 完了条件: 全commandが同じ`wms.toml`とdiagnostic形式を使う

## 6. 実行adapter

- [+] headless runner
  - 入力: GameRuntime、scripted input
  - 出力: deterministic reportとtest runner
  - 完了条件: GUIなしで全サンプルを最後まで検証できる
- [+] egui adapter
  - 入力: render/input/audio port
  - 出力: 対話用window
  - 完了条件: headlessと同じ入力列で同じworld/resultになる

## 7. サンプルと文書

- [+] ノベルゲーム
  - 入力: task/await、ui、save
  - 出力: 選択分岐を持つ最小project
  - 完了条件: check/test/run/package/package-runが通る
- [+] RPG
  - 入力: entity/component、map、event
  - 出力: 移動と戦闘を持つ最小project
  - 完了条件: check/test/run/package/package-runが通る
- [+] RTS
  - 入力: system、fixed tick、command event
  - 出力: unitと生産を持つ最小project
  - 完了条件: check/test/run/package/package-runが通る
- [+] シミュレーション
  - 入力: agent、時間、seed RNG、save
  - 出力: 再現可能な最小project
  - 完了条件: check/test/run/package/package-runが通る
- [+] 利用文書
  - 入力: 確定したCLIと言語
  - 出力: README、language guide、4sample guide
  - 完了条件: 文書記載commandがE2Eで検証される

## 8. 最終ゲート

- [+] 全検証
  - 入力: 全成果物
  - 出力: fmt/check/test/doc/WASM/sample E2E結果
  - 完了条件: `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo doc --no-deps`、WASM compile check、4sample E2Eがすべて成功する

## 9. 次期項目

- [ ] Web browser bootstrapと描画adapter
  - 入力: WASM境界仕様、GameRuntime port
  - 出力: browser runner
  - 完了条件: headlessと同一replay結果をbrowserで再現できる
