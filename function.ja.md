# WMLScript 言語仕様と関数一覧

この文書は、現在実装されている WMLScript の表層文法と、
デフォルトランタイムで利用できる関数一覧をまとめたものです。

正確な仕様は次も参照してください。
- [SPEC/language.md](SPEC/language.md)
- [SPEC/vm.md](SPEC/vm.md)
- [SPEC/op.md](SPEC/op.md)
- [SPEC/hostapi.md](SPEC/hostapi.md)

## 1. 言語の概要

現在の WMLScript は、モジュール単位の小さなスクリプト形式です。

- `import "path/to/module.wml";`
- `import "path/to/module.wml" as alias;`
- `export func name(params) { ... }`
- `export let name = literal;`

コンパイラ前端が現在扱える関数本体は、かなり限定されています。

- `return;`
- `return <expr>;`

### 1.1 モジュール例

```wml
import "shared/ui.wml" as ui;

export let title = "My Game";

export func main() {
    return 1 + 2 * 3;
}
```

## 2. 関数本体の文法

現在の式文法は小さめです。

- リテラル:
  - `nil`
  - `true`
  - `false`
  - 整数
  - 浮動小数
  - 文字列
- 単項演算:
  - `-expr`
- 二項演算:
  - `expr + expr`
  - `expr - expr`
  - `expr * expr`
  - `expr / expr`
- グルーピング:
  - `(expr)`

この範囲について、コンパイラは定数畳み込みと型タグ付けを行います。

### 2.1 現在の制約

- `if` / `match` / `while` / `for` は未実装です。
- スクリプト表層からの関数呼び出し構文はまだありません。
- ユーザー定義 struct / class もまだありません。
- `export let` は現状リテラル値のみです。

## 3. 実行環境の関数一覧

デフォルトランタイムは `ext.*` 名前空間で拡張関数を公開します。
以下は現在利用できる呼び出し先です。

### 3.1 `ext.fs`

必要 capability: `CAP_FILE_SYSTEM`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.fs.read` | `read(path: string)` | `string` | ホストのファイルシステムからテキストを読み込みます。 |
| `ext.fs.write` | `write(path: string, contents: string)` | `nil` | ホストのファイルシステムへテキストを書き込みます。 |
| `ext.fs.exists` | `exists(path: string)` | `bool` | パスが存在するか確認します。 |

### 3.2 `ext.debug`

必要 capability: なし

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.debug.log` | `log(value)` | `nil` | 値を整形してデバッグログへ追記します。 |
| `ext.debug.inspect` | `inspect(value)` | `string` | 値の文字列表現を返します。 |

### 3.3 `ext.net`

必要 capability: `CAP_NETWORK`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.net.get` | `get(url: string)` | `string` | 設定済みのネットワークバックエンドで GET を実行します。 |
| `ext.net.post` | `post(url: string, body: string)` | `string` | 設定済みのネットワークバックエンドで POST を実行します。 |

### 3.4 `ext.llm`

必要 capability: `CAP_ASYNC_IO`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.llm.generate` | `generate(prompt: string)` | `string` | 設定済みの LLM バックエンドへプロンプトを送信します。 |

## 4. VM レベルの実行プリミティブ

これは表層の「関数」ではなく VM の opcode ですが、実行モデルを読む際に重要です。

- `send(worker_id, payload)` - 別 worker へメッセージを送ります
- `recv()` - メッセージを待って worker を待機状態にします
- `try_recv()` - 受信可能ならメッセージを1つ読み取ります
- `yield` - 自発的に worker を譲ります
- `sleep` - worker を睡眠状態に移します

## 5. 実用上の補足

- `wmltoolchain` は現在のサブセットを archive にまとめます。
- `wmlfrontend` は `native` / `wasm` / `egui` で同じプロジェクトを実行できます。
- `egui` フロントエンドの既定フォントは、日本語向けに Noto Sans 系です。

## 6. 参照サンプル

次を参照してください。
- `samples/helloworld`
- `samples/inputlink`
- `samples/workercomm`
- `samples/assetload`
- `samples/easynovel`

