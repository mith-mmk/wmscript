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

### 3.5 `ext.message`

必要 capability: `CAP_GUI`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.message.show` | `show(text: string)` または `show(speaker: string, text: string)` | `bool` | メッセージ窓に本文と任意の話者名を表示します。 |
| `ext.message.append` | `append(line: string)` | `bool` | 現在の本文とバックログに 1 行を追加します。 |
| `ext.message.choices` | `choices(label1, label2, ...)` | `bool` | メッセージ窓の選択肢一覧を更新します。 |
| `ext.message.prompt` | `prompt(text: string)` | `bool` | プレイヤー入力欄の上に表示するプロンプトを設定します。 |
| `ext.message.hide` | `hide()` | `bool` | メッセージ窓を隠します。 |
| `ext.message.clear` | `clear()` | `bool` | メッセージ窓の本文、プロンプト、選択肢を消去します。 |

### 3.6 `ext.image`

必要 capability: `CAP_GUI`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.image.load` | `load(resource_id: int)` | `handle \| request_id` | 画像リソースを読み込み、準備済みなら handle を返します。 |
| `ext.image.info` | `info(handle)` | `table` | resource id / type / size / state のメタデータを返します。 |
| `ext.image.status` | `status(handle)` | `int` | 数値の resource state code を返します。 |
| `ext.image.release` | `release(handle)` | `bool` | 画像 handle を解放します。 |
| `ext.image.draw` | `draw(handle, x, y)` | `bool` | frontend レンダラ向けの描画命令を記録します。 |
| `ext.image.draw_part` | `draw_part(handle, sx, sy, sw, sh, dx, dy)` | `bool` | 画像の一部分を描画する命令を記録します。 |
| `ext.image.draw_ext` | `draw_ext(handle, sx, sy, sw, sh, dx, dy, dw, dh, rot, alpha)` | `bool` | 拡大縮小と回転つきの描画命令を記録します。 |
| `ext.image.set_icon_sheet` | `set_icon_sheet(handle, cell_w, cell_h)` | `bool` | スプライトシートのセル情報を保存します。 |
| `ext.image.draw_icon` | `draw_icon(handle, index, x, y)` | `bool` | 設定済みのアイコンシートからスプライトを描画します。 |

### 3.7 `ext.audio`

必要 capability: `CAP_ASYNC_IO`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.audio.load` | `load(resource_id: int)` | `handle \| request_id` | 音声リソースを読み込み、準備済みなら handle を返します。 |
| `ext.audio.play` | `play(handle, loop=false)` | `bool` | 再生を開始または再開します。 |
| `ext.audio.playback` | `playback(handle, loop=false)` | `bool` | 高レベルな表層から使う `play` の別名です。 |
| `ext.audio.pause` | `pause(handle)` | `bool` | 再生を一時停止します。 |
| `ext.audio.stop` | `stop(handle)` | `bool` | 再生を停止して先頭へ戻します。 |
| `ext.audio.seek` | `seek(handle, position_ms)` | `bool` | 再生位置を移動します。 |
| `ext.audio.volume` | `volume(handle, value)` | `bool` | 再生音量を更新します。 |
| `ext.audio.status` | `status(handle)` | `int` | 現在の再生状態コードを返します。 |
| `ext.audio.release` | `release(handle)` | `bool` | 音声 handle を解放します。 |

### 3.8 `ext.vm`

必要 capability: なし

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.vm.save` | `save(slot: int)` | `bool` | ランタイムのチェックポイントをメモリに保存します。 |
| `ext.vm.load` | `load(slot: int)` | `bool` | 以前保存したチェックポイントを復元します。 |

### 3.9 `state`

必要 capability: なし

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `state.save` | `save(slot: int)` | `bool` | 現在の永続キー値状態をスロットに保存します。 |
| `state.load` | `load(slot: int)` | `bool` | スロットから永続キー値状態を復元します。 |
| `state.has` | `has(key: string)` | `bool` | 現在の状態にキーが存在するか確認します。 |
| `state.get` | `get(key: string)` | `value` | 現在のキー値を返します。存在しない場合は `nil` です。 |
| `state.set` | `set(key: string, value)` | `bool` | 現在の状態に値を書き込みます。 |
| `state.erase` | `erase(key: string)` | `bool` | 現在の状態からキーを削除します。 |

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
