# todoリスト
  - \- [ ] の補完面倒なので各項目に入れてください
　- 実装が終わったらステータスを変更してください 
　- [+] コード実装が最終テスト未完のもの
　- [*] 実装途中のもの
　- [x] テストが完了し、動作チェックが終わったもの（人間がチェック）
　- [-] 実装を見合わせたもの/issue送りにしたもの（人間がチェック）
　- 仕様書はSPEC/の下にあり
　- 問題点や曖昧な仕様はissue.mdで管理

0. Crate分割
    - [ ] .gitignoreの整理
    - [ ] Workflowの作成
    - [ ] 以下のcrateの作成 cargo new
    - [ ] buildチェーンの作成(releasesに保管)
    - [ ] VMにwasmとeguiの実装で異なる部分を吸収可能なモジュールを実装

```


1. 実行系 (crate wmlvm, crate.ioに公開予定)
WMLVM
├─ core
├─ scheduler
├─ memory / GC
├─ verifier

1. ホスト統合系（Engine Bridge,  crate.ioに公開予定）
WMLHost
├─ HostAPI
├─ ResourceManager
├─ StateManager
├─ Audio/Image/UI
├─ AsyncIO

1. コンパイラ系（Script Toolchain, githubのみ, npmかも）

WMLCompiler
├─ parser
├─ resolver (import解決)
├─ IR
├─ optimizer
├─ bytecode_gen
├─ symbol_table

4. バイトコード変換系（低レイヤ,  crate.ioに公開予定）
WMLBytecode
├─ encoder   ← (IR → bytecode)
├─ decoder   ← (bytecode → Op)
├─ verifier
├─ disassembler（任意）

5. アーカイブ系（Distribution,  crate.ioに公開予定）
WMLArchive
├─ archiver
├─ unarchiver
├─ signer
├─ verifier
├─ manifest_builder

6. リソース系（Asset Pipeline）
WMLResource
├─ resource_id_resolver
├─ asset_builder
├─ compression
├─ encoding (image/audio)


# 完成形
Runtime
 ├─ WMLVM
 └─ WMLHost

Toolchain
 ├─ WMLCompiler
 ├─ WMLBytecode
 ├─ WMLArchive
 └─ WMLResource


```



1. 仕様書リファクタ
1.1 ドキュメント構造分割

  仕様を以下の単位に分割
 - [ ] 言語仕様（WMLScript）
 - [ ] VM仕様
 - [ ] バイトコード仕様
 - [ ] アーカイブ仕様
 - [ ] ホストAPI仕様
 - [ ] 各仕様に責務コメント追加（何を定義するか明記）
 - [ ] 各仕様間の依存関係を明文化（例：VM→バイトコード）

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

 - TODOを機能単位に分解
 - 各TODOに「入力」「出力」「完了条件」を付与
 - 実装依存順に並び替え

1. VM実装
2.1 外部公開API（crate）

 VM生成API

 Vm::new(config)

 実行API

 vm.run_frame(step_limit)

 worker操作

 spawn / send / recv

 asset/stateアクセスAPI

2.2 内部コア構造

 Value enum実装（int/float/string/handleなど）

 Stack構造

 Frame構造

 VM struct（stack / frames / heap / pc）

2.3 デコーダ

 バイトコード → Op enum変換

 little endian読み込み統一

 命令長テーブル or decode内管理

2.4 実行ループ

 match dispatchループ実装

 CALL / RETURN処理

 CALL_HOST処理

 JUMP系処理

 stack操作安全チェック（debug）

2.5 ワーカーモデル

 workerインスタンス分離

 メッセージキュー実装

 状態管理（running/waiting/sleeping）

2.6 スケジューラ

 協調スケジューリング

 yield対応

 sleep対応

 step制限

2.7 Host API

 host_idテーブル

 call_hostディスパッチ

 capabilityチェック

2.8 WebAssembly対応

 スレッド非依存設計（worker仮想化）

 async bridge（JS連携）

 メモリ制約対応

1. VMテスト
3.1 命令テスト

 全opcode単体テスト

 stack挙動テスト

 jump整合性テスト

3.2 実行テスト

 関数呼び出し

 再帰制限確認

 エラーケース

3.3 ワーカー

 send/recvテスト

 非同期動作確認

4. VM Extender（ext API）
4.1 拡張基盤

 extension登録API

 ext_id割当

 namespace管理

4.2 サンプル実装

 ext.ffmpeg.*

 decode

 encode

 stream

5. スクリプトサンプル

 Hello World

 input連動スクリプト

 worker通信サンプル

 assetロード例

6. スクリプトコンパイラ
6.1 フロントエンド

 parser

 AST生成

6.2 中間処理

 import解決

 シンボル解決

 型タグ付け

6.3 バックエンド

 IR生成

 最適化

 bytecode生成

 ID割当

7. 逆コンパイラ

 bytecode → IR

 IR → script復元

 シンボル再構築

8. アーカイバ
8.1 基本

 bundle構造生成

 manifest生成

 ネットワークダウンロードに対応するためのファイル配置の最適化


8.2 セキュリティ

 ハッシュ生成

 署名処理

 鍵管理

9. 逆アーカイバ

 bundle展開

 署名検証

 integrityチェック

10. UI実装
10.1 共通層

 UI抽象レイヤ

10.2 egui

 描画

 入力

10.3 WebGL

 レンダリング

 JS bridge

11. ext APIサンプル

 ext.fs

 ext.net

 ext.debug

12. 総合サンプル
12.1 フロント

 メニューUI（セーブ/ロード）

12.2 スクリプト

 分岐スクリプト

12.3 バックエンド

 タイマー

12.4 リソース

 ダミー画像

 UI素材

12.5 結合テスト

 全フロー通し実行

 セーブ/ロード整合確認

13．拡張機能

```
依存関係（重要）
仕様 → VM → コンパイラ → アーカイバ
             ↓
          テスト
             ↓
        サンプル/UI
```

14. ランタイムの作成
14.1 ランタイムは以下の追加機能を追加させる。そのための薄いラッパーを被せること
    - アーカイブ暗号化（ファイル境界をまたがない様に分割）
    - アーカイブ分割 
    - 認証チェック機能
    - セーブデータ難読化
    - 要するに有料販売するためのDRM関係の機能
    - manifestで管理させよう