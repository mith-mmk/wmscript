# 実行ポリシー
- テンポラリ、ワーキングディレクトリの名前は、/.test*を使うこと それ以外はgitが壊れます
- todo.md を上から順に実行する
- 各todoは「完了条件」を満たすまで実装する
- 設計はSPEC.yml.mdにある
- 不明点は SPEC/issue.md に記録しつつ継続する
- ただし「仕様不整合」はその場で停止し issue に詳細記録する

# 仕様参照ルール

仕様書は SPEC/*.md にある

優先順位:
1. VM仕様
2. オペコード仕様
3. VM実装
4. Host API仕様
5. リソース仕様
6. アーカイブ仕様
7. コンパイラ実装
8. デコンパイラ実装
9. コンパイル用サンプルスクリプト作成
10. ドキュメント作成
11. 統合テスト
12. 統合動作チェック用example

矛盾がある場合:
- issue.md に記録
- TODOを止めず、影響範囲を限定して実装継続

# 実行環境
- cargo build / test / doc / fmt を必ず使用する
- optimize 速度重視 サイズ重視（Web用）
- 必要な実行権限は最初に要求すること
- cargo build --releaseベースでベンチマークを取ること bench.mdでバージョン管理

# 実装ルール

- 命名規則:
  - Rust: snake_case
  - 型: PascalCase
  - 定数: UPPER_CASE
  - module = ファイル単位で統一

- unsafe の使用は原則禁止（必要な場合はissueに理由を書く）
- panicは回避
  
# テスト要件

必ず以下を実装:

- opcode単体テスト
- VM実行テスト（命令列）
- verifierテスト
- Host APIモックテスト

cargo test が通る状態を維持する

- Windows, MacOS, Linux, Anidroid, wasmのコンパイルが通ること

# ドキュメント

- すべてのpublic APIに doc コメントを書く
- cargo doc が通ること

# フォーマット

- cargo fmt を必ず通す

# TODO実行ルール

- TODOは途中で止めない
- ただし以下の場合のみ停止:
  - 仕様衝突
  - 実行不能
  - セキュリティ破綻

- 停止時は issue.md に以下を書く:
  - 問題内容
  - 該当仕様
  - 暫定判断

# 出力

- 変更は最小単位でコミット可能な状態にする