# todoリスト
  - \- [ ] の補完面倒なので各項目に入れてください
　- 実装が終わったらステータスを変更してください 
　- [+] コード実装が最終テスト未完のもの
　- [*] 実装途中のもの
　- [x] テストが完了し、動作チェックが終わったもの（人間がチェック）
　- [-] 実装を見合わせたもの/issue送りにしたもの（人間がチェック）
　- 仕様書はSPEC/の下にあり
　- 問題点や曖昧な仕様はissue.mdで管理

# カレント

# 実装
0. Crate分割
    - [+] .gitignoreの整理
    - [+] Workflowの作成
    - [+] 以下のcrateの作成 cargo new
    - [+] buildチェーンの作成(releasesに保管)
    - [+] VMにwasmとeguiの実装で異なる部分を吸収可能なモジュールを実装

0.1 仕様変更
 - [+] 拡張子.wmlを.wmsへ
 - [+] WMLScriptをWMScriptへ
 - [+] 名称をすべてそれに合わせる


1. 実行系 (crate wmvm, crate.ioに公開予定)
```
wmvm
├─ core
├─ scheduler
├─ memory / GC
├─ verifier
```

1. ホスト統合系（Engine Bridge,  crate.ioに公開予定）
wmhost
├─ HostAPI
├─ ResourceManager
├─ StateManager
├─ Audio/Image/UI
├─ AsyncIO

1. コンパイラ系（Script Toolchain, githubのみ, npmかも）

wmcompiler
├─ parser
├─ resolver (import解決)
├─ IR
├─ optimizer
├─ bytecode_gen
├─ symbol_table

4. バイトコード変換系（低レイヤ,  crate.ioに公開予定）
wmbytecode
├─ encoder   ← (IR → bytecode)
├─ decoder   ← (bytecode → Op)
├─ verifier
├─ disassembler（任意）

5. アーカイブ系（Distribution,  crate.ioに公開予定）
wmarchive
├─ archiver
├─ unarchiver
├─ signer
├─ verifier
├─ manifest_builder

6. リソース系（Asset Pipeline）
wmresource
├─ resource_id_resolver
├─ asset_builder
├─ compression
├─ encoding (image/audio)


# 完成形
Runtime
 ├─ wmvm
 └─ wmhost

Toolchain
 ├─ wmcompiler
 ├─ wmbytecode
 ├─ wmarchive
 └─ wmresource


```



1. 仕様書リファクタ
1.1 ドキュメント構造分割

  仕様を以下の単位に分割
 - [+] 言語仕様（WMScript）
 - [+] VM仕様
 - [+] バイトコード仕様
 - [+] アーカイブ仕様
 - [+] ホストAPI仕様
 - [+] 各仕様に責務コメント追加（何を定義するか明記）
 - [+] 各仕様間の依存関係を明文化（例：VM→バイトコード）

1.2 相互リンク整理

 命令セット ↔ VM実行モデルのリンク追加

 API ↔ CALL_HOST仕様のリンク追加

 package / worker ↔ scheduler のリンク追加

1.3 仕様検証（抜け・矛盾チェック）

 init/update/on_messageの呼び出し順の明文化

 worker lifecycle（spawn→run→destroy）の定義

 メモリ解放タイミング（handle含む）の定義

 エラー時の挙動（nil/return）の統一

 import解決とID割当のフロー明確化

1.4 TODO整理

 - 整理方針
   - 上流から下流へ並べる
   - 1項目は1成果物に絞る
   - 各TODOに「入力」「出力」「完了条件」を付与する
 - 実装順のバックログ
   - [ ] 仕様固定の最終確認
     - 入力: `SPEC/language.md`, `SPEC/vm.md`, `SPEC/op.md`, `SPEC/hostapi.md`, `SPEC/resource.md`, `SPEC/archive.md`, `SPEC/scheduler.md`
     - 出力: 仕様間リンクと依存関係が明文化された状態
     - 完了条件: 仕様の矛盾が `SPEC/issue.md` に切り出され、実装前提が確定する
   - [+] VMコア最小型の定義
     - 入力: `SPEC/vm.md`, `SPEC/op.md`
     - 出力: `Value`, `Stack`, `Frame`, `VmConfig`, `Vm`
     - 完了条件: 型定義だけで `cargo check` が通り、VMの骨格が共有できる
   - [+] バイトコードデコード層
     - 入力: opcode表、bytecode buffer
     - 出力: `Op` enum への decode 関数、little endian 読み取りヘルパ
     - 完了条件: invalid opcode / eof / endian の単体テストが揃う
   - [+] VM実行ループ
     - 入力: `Op`, `Vm`, `HostRegistry`
     - 出力: `run_frame`、`CALL/RETURN/JUMP/CALL_HOST` の実装
     - 完了条件: 基本命令列の実行テストが通る
   - [+] worker と scheduler
     - 入力: `Vm`, message queue, sleep/request state
     - 出力: worker state machine、step budget scheduler
     - 完了条件: `spawn/run/destroy` と `yield/sleep/recv` の遷移が確認できる
   - [+] Host API ブリッジ
     - 入力: host_id table、capability table
     - 出力: host dispatch、権限制御、mock interface
     - 完了条件: `CALL_HOST` のモックテストが書ける
   - [+] Verifier
     - 入力: bytecode module、func table、const table、jump targets、host_id table
     - 出力: 検証結果、エラー分類
     - 完了条件: 不正 opcode / 範囲外 jump / 無効 host_id を検出できる
   - [+] VMテスト
     - 入力: 命令列、VM状態、host mock
     - 出力: opcode 単体テスト、実行テスト、worker テスト
     - 完了条件: `cargo test --workspace` が安定して通る
   - [+] Ext API 基盤
     - 入力: host registry、namespace policy
     - 出力: ext_id 割当、namespace 管理
     - 完了条件: `ext.*` の名前解決がコンパイル時 ID に落ちる
   - [+] コンパイラ基盤
     - 入力: AST/IR 方針、import 解決規則
     - 出力: parser、resolver、IR の骨格
     - 完了条件: 静的 import 解決と symbol table の流れが作れる
   - [+] アーカイブ / リソース基盤
     - 入力: manifest、section table、resource id policy
     - 出力: archive encode/decode、verify、resource handle の橋渡し
     - 完了条件: bundle load までの経路が仕様で閉じる
   - [+] UI / サンプル / ランタイム
     - 入力: host UI/audio/image API、VM と compiler の成果物
     - 出力: サンプルスクリプト、総合サンプル、runtime wrapper
     - 完了条件: end-to-end の最小例が説明できる

1. VM実装
2.1 外部公開API（crate）

 [+] VM生成API

 [+] Vm::new(config)

 [+] 実行API

 [+] vm.run_frame(step_limit)

 [+] worker操作（send / recv）

 [+] send / recv

 [ ] time 時間コントロールAPI(wait キー待ち or 時間待ち /tick/sleep)

 asset/stateアクセスAPI

2.2 内部コア構造

 [+] Value enum実装（int/float/string/handleなど）

 [+] Stack構造

 [+] Frame構造

 [+] VM struct（stack / frames / heap / pc）

2.3 デコーダ

 [+] バイトコード → Op enum変換

 [+] little endian読み込み統一

 [+] 命令長テーブル or decode内管理

2.4 実行ループ

 [+] match dispatchループ実装

 [+] CALL / RETURN処理

 [+] CALL_HOST処理

 [+] JUMP系処理

 [+] stack操作安全チェック（debug）

2.5 ワーカーモデル

 [+] workerインスタンス分離

 [+] メッセージキュー実装

 [+] 状態管理（running/waiting/sleeping）

2.6 スケジューラ

 [+] 協調スケジューリング

 [+] yield対応

 [+] sleep対応

 [+] step制限

2.7 Host API

 [+] host_idテーブル

 [+] call_hostディスパッチ

 [+] capabilityチェック

2.8 WebAssembly対応

 スレッド非依存設計（worker仮想化）

 async bridge（JS連携）

 メモリ制約対応

1. VMテスト
3.1 命令テスト

 [+] 全opcode単体テスト

 [+] stack挙動テスト

 [+] jump整合性テスト

3.2 実行テスト

 [+] 関数呼び出し

 再帰制限確認

 エラーケース

3.3 ワーカー

 [+] send/recvテスト

 非同期動作確認

4. VM Extender（ext API）
4.1 拡張基盤

 [+] extension登録API

 [+] ext_id割当

 [+] namespace管理

4.2 サンプル実装

 ext.ffmpeg.*

 decode

 encode

 stream

5. スクリプトサンプル

 [+] Hello World

 [+] input連動スクリプト

 [+] worker通信サンプル

 [+] assetロード例

 [+] easynovel

6. スクリプトコンパイラ
6.1 フロントエンド

 [+] parser

 [+] AST生成

6.2 中間処理

 [+] import解決

 [+] シンボル解決

 [+] 型タグ付け

6.3 バックエンド

 [+] IR生成

 [+] 最適化

 [+] bytecode生成

 [+] ID割当

7. 逆コンパイラ

 bytecode → IR

 IR → script復元

 シンボル再構築

8. アーカイバ
8.1 基本

 [+] bundle構造生成

 [+] manifest生成
 [+] compile済みmoduleを archive に格納して .warc 単体起動を可能化
 [+] streaming reader で archive を section 単位ロード可能化

 ネットワークダウンロードに対応するためのファイル配置の最適化


8.2 セキュリティ

 [+] ハッシュ生成

 [+] 署名処理

 [+] 鍵管理

9. 逆アーカイバ

 [+] bundle展開

 [+] 署名検証

 [+] integrityチェック

10. UI実装
10.1 共通層

 [+] UI抽象レイヤ

 [+] メッセージウィンドウ / 画像スロットのUI状態
 [+] text log / backlog制御

10.2 egui

 [+] 描画
 [+] 画像ロード
 [+] 入力
 [+] 音声再生バックエンド
 [+] auto / skip進行制御
 [+] scene.reset で message window と描画済みゲーム画面を初期化
 [+] image.release で解放済み handle の描画状態を除去

10.3 WebGL

 レンダリング

 JS bridge

10.4 paintcore+WML実装 環境非依存
- https://github.com/mith-mmk/wasm-paint

11. ext APIサンプル

 [+] ext.fs

 [+] ext.net // ネットワークダウンロード, ネットワークアクセス

 [+] ext.debug

 [+] ext.llm  llama-cpp +  0.6B-0.8Bの軽量LLM([qwen3](https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/blob/main/Qwen3.5-0.8B-Q8_0.gguf))で人工無能

 [+] ext.image

 [+] ext.audio

 [+] ext.vm(save, load)

 [+] state.save/load

 [+] ext.image.draw

 [+] ext.image.draw_part/draw_ext

 [+] ext.image.set_icon_sheet/draw_icon

 [+] ext.audio.playback

 [+] ext.message

12. 総合サンプル
12.1 フロント
 [+] メニューUI（セーブ/ロード）
 [+] image/audioデモ

12.2 スクリプト
 [+] 分岐スクリプト
 [+] image/audioスクリプト

12.3 バックエンド
 [+] タイマー

12.4 リソース
 [+] ダミー画像
 [+] UI素材

12.5 結合テスト
 [+] 全フロー通し実行
 [+] セーブ/ロード整合確認

13．拡張機能
13.1 FFIラッパー
- C/C++, .net, node, pythonからも呼べる様にする
- Unity統合
- Unreal Engineに統合

```
依存関係（重要）
仕様 → VM → コンパイラ → アーカイバ
             ↓
          テスト
             ↓
        サンプル/UI
```

13.2.  ランタイムの作成
    - [+] ランタイムは素のランタイムと有料ランタイムを作る

14.1 ランタイムは以下の追加機能を追加させる。そのための薄いラッパーを被せること
    - アーカイブ分割 （ファイル境界をまたがない様に分割）
    - アーカイブ暗号化
    - 認証チェック機能
    - セーブデータ難読化
    - 要するに有料販売するためのDRM関係の機能
    - 恐らくバイナリ+外部用dll(so)の組み合わせ
    - android, iOS だと パッケージ化？
    - これ有料ライセンスにしようか(issue)


