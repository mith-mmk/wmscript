# WMScript 言語仕様と関数一覧

この文書は、現在実装されている WMScript の表層文法と、
デフォルトランタイムで利用できる関数一覧をまとめたものです。

正確な仕様は次も参照してください。
- [SPEC/language.md](SPEC/language.md)
- [SPEC/vm.md](SPEC/vm.md)
- [SPEC/op.md](SPEC/op.md)
- [SPEC/hostapi.md](SPEC/hostapi.md)

## 1. 言語の概要

現在の WMScript は、モジュール単位の小さなスクリプト形式です。

- `import "path/to/module.wms";`
- `import "path/to/module.wms" as alias;`
- `export func name(params) { ... }`
- `export let name = literal;`

コンパイラ前端が現在扱える関数本体は、かなり限定されています。

- `;` で終わる式文
- 関数本体内の `let name = expr;` による局所束縛
- `return;`
- `return <expr>;`
- `if expr { ... }`
- `if expr { ... } else { ... }`
- `if expr { ... } else if expr { ... } else { ... }`
- `recv();` で frontend / worker からの次のメッセージを待つ

### 1.1 モジュール例

```wms
import "shared/ui.wms" as ui;

export let title = "My Game";

export func main() {
    return 1 + 2 * 3;
}
```

## 2. 関数本体の文法

現在の式文法は小さめです。

- 式文:
  - `expr;`
- 局所束縛:
  - `let name = expr;`
- 条件分岐:
  - `if expr { ... }`
  - `if expr { ... } else { ... }`
  - `if expr { ... } else if expr { ... } else { ... }`
- リテラル:
  - `nil`
  - `true`
  - `false`
  - 整数
  - 浮動小数
  - 文字列
- 単項演算:
  - `-expr`
  - `!expr`
- 二項演算:
  - `expr + expr`
  - `expr - expr`
  - `expr * expr`
  - `expr / expr`
  - `expr && expr`
  - `expr || expr`
- 比較:
  - `expr == expr`
  - `expr != expr`
  - `expr < expr`
  - `expr <= expr`
  - `expr > expr`
  - `expr >= expr`
- グルーピング:
  - `(expr)`
- 関数・拡張呼び出し:
  - `ext.namespace.name(expr, ...)`
  - `recv()`
  - `try_recv()`
  - `yield()`
  - `sleep()`
- 制御フロー:
  - `if expr { ... } else { ... }`
  - `loop { ... }`
  - `break;`
  - `continue;`
- 局所変数参照:
  - 同じ関数本体で前に `let` した裸の識別子

この範囲について、コンパイラは定数畳み込みと型タグ付けを行います。
拡張関数のメタデータに戻り値型ヒントがある場合は、`ext.*` 呼び出しの
型タグ付けにそれを利用します。

### 2.1 現在の制約

- `match` / `while` / `for` は未実装です。
- ユーザー定義 struct / class もまだありません。
- `export let` は現状リテラル値のみです。

## 3. 実行環境の関数一覧

デフォルトランタイムは `ext.*` 名前空間で拡張関数を公開します。
以下は現在利用できる呼び出し先です。

### 3.0 capability ゲート

コンパイラは、選択した platform profile に必要な capability が無い
`ext.*` 呼び出しを拒否します。対応は次のとおりです。

- `CAP_FILE_SYSTEM` が必要: `ext.fs.*`
- `CAP_NETWORK` が必要: `ext.net.*`
- `CAP_ASYNC_IO` が必要: `ext.llm.*` / `ext.audio.*`
- `CAP_GUI` が必要: `ext.scene.*` / `ext.message.*` / `ext.image.*`
- `state.*` と `ext.vm.*` は platform capability 不要

既定 profile の対応は次の表です。

| Profile | File system | Async I/O | GUI | Network | Web compat |
| --- | --- | --- | --- | --- | --- |
| `native` | yes | yes | yes | yes | no |
| `egui` | yes | yes | yes | yes | no |
| `wasm` | no | yes | no | no | yes |

この条件に合わない拡張を参照した場合、bytecode を出力する前に
コンパイルエラーになります。

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

### 3.5 `ext.scene`

必要 capability: `CAP_GUI`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.scene.layout` | `layout(choice_x, choice_y, choice_w, choice_h, message_x, message_y, message_w, message_h)` | `bool` | choice 窓と message 窓の配置を frontend 側に指示します。 |
| `ext.scene.reset` | `reset()` | `bool` | 既定の scene layout に戻し、現在の message window と記録済みの画像描画状態も消去します。 |
| `ext.scene.opening` | `opening(title: string)` | `bool` | メッセージ窓を使って opening タイトルカードを表示します。 |
| `ext.scene.ending` | `ending(title: string)` | `bool` | メッセージ窓を使って ending タイトルカードを表示します。 |

### 3.6 `ext.message`

必要 capability: `CAP_GUI`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.message.show` | `show(text: string)` または `show(speaker: string, text: string)` | `bool` | メッセージ窓に本文と任意の話者名を表示します。 |
| `ext.message.append` | `append(line: string)` | `bool` | 現在の本文とバックログに 1 行を追加します。 |
| `ext.message.choices` | `choices()` または `choices(label1, label2, ...)` | `bool` | メッセージ窓の選択肢一覧を更新します。引数なしなら選択肢を消去します。 |
| `ext.message.choices_named` | `choices_named()` または `choices_named(id1, label1, id2, label2, ...)` | `bool` | エンジン側で決めた安定 choice id と表示ラベルをまとめて設定します。引数なしなら選択肢を消去します。 |
| `ext.message.prompt` | `prompt()` または `prompt(text: string)` | `bool` | プレイヤー入力欄の上に表示するプロンプトを設定または消去します。 |
| `ext.message.hide` | `hide()` | `bool` | メッセージ窓を隠します。 |
| `ext.message.speed` | `speed(value)` | `bool` | メッセージ窓の文字表示速度を設定します。 |
| `ext.message.auto` | `auto(enabled)` | `bool` | メッセージ窓の auto 進行モードを切り替えます。 |
| `ext.message.skip` | `skip(enabled)` | `bool` | メッセージ窓の skip モードを切り替えます。 |
| `ext.message.log_clear` | `log_clear()` | `bool` | 現在のページ状態は維持したまま、text log / backlog だけを消去します。 |
| `ext.message.clear` | `clear()` | `bool` | メッセージ窓の本文、プロンプト、選択肢を消去します。 |
| `ext.message.box_style` | `box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | メッセージ窓パネルの塗り色と枠色を設定します。 |
| `ext.message.text_color` | `text_color(r, g, b, a)` | `bool` | 本文の文字色を設定します。 |
| `ext.message.speaker_color` | `speaker_color(r, g, b, a)` | `bool` | 話者名の文字色を設定します。 |
| `ext.message.accent_color` | `accent_color(r, g, b, a)` | `bool` | 見出しやヒントに使うアクセント色を設定します。 |
| `ext.message.font_size` | `font_size(body, speaker)` | `bool` | frontend のメッセージ窓で使う本文と話者名の文字サイズを設定します。 |
| `ext.message.reset_style` | `reset_style()` | `bool` | メッセージ窓の既定スタイルに戻します。 |
| `ext.message.frame` | `frame()` または `frame(resource_id)` | `bool` | メッセージ窓のフレーム画像として使う resource を設定または解除します。 |
| `ext.message.content_inset` | `content_inset(left, top, right, bottom)` | `bool` | 外側のフレーム画像から本文領域までの inset を設定します。 |
| `ext.message.input_box_style` | `input_box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | プレイヤー入力パネルの塗りと枠線の色を設定します。 |
| `ext.message.input_text_color` | `input_text_color(r, g, b, a)` | `bool` | 入力欄に表示される入力文字色を設定します。 |
| `ext.message.input_hint_color` | `input_hint_color(r, g, b, a)` | `bool` | 入力欄のヒント文字色を設定します。 |
| `ext.message.input_prompt_color` | `input_prompt_color(r, g, b, a)` | `bool` | 入力欄の上に表示されるプロンプト文字色を設定します。 |
| `ext.message.choice_box_style` | `choice_box_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | 選択肢パネルの塗りと枠線の色を設定します。 |
| `ext.message.choice_text_color` | `choice_text_color(r, g, b, a)` | `bool` | 選択肢ラベルの文字色を設定します。 |
| `ext.message.choice_accent_color` | `choice_accent_color(r, g, b, a)` | `bool` | 選択肢パネルの見出しやカーソルのアクセント色を設定します。 |
| `ext.message.choice_selected_style` | `choice_selected_style(fill_r, fill_g, fill_b, fill_a, stroke_r, stroke_g, stroke_b, stroke_a)` | `bool` | 選択中の行の塗りと枠線の色を設定します。 |
| `ext.message.locale` | `locale()` または `locale(code: string)` | `string` | メッセージ UI の言語 (`ja` / `en`) を取得または設定します。 |

### 3.7 `ext.image`

必要 capability: `CAP_GUI`

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.image.load` | `load(resource_id: int)` | `handle \| request_id` | 画像リソースを読み込み、準備済みなら handle を返します。 |
| `ext.image.info` | `info(handle)` | `table` | resource id / type / size / state のメタデータを返します。 |
| `ext.image.status` | `status(handle)` | `int` | 数値の resource state code を返します。 |
| `ext.image.release` | `release(handle)` | `bool` | 画像 handle を解放し、その handle に紐づく描画命令と icon sheet 状態も消去します。 |
| `ext.image.draw` | `draw(handle, x, y)` | `bool` | frontend レンダラ向けの描画命令を記録します。 |
| `ext.image.draw_part` | `draw_part(handle, sx, sy, sw, sh, dx, dy)` | `bool` | 画像の一部分を描画する命令を記録します。 |
| `ext.image.draw_ext` | `draw_ext(handle, sx, sy, sw, sh, dx, dy, dw, dh, rot, alpha)` | `bool` | 拡大縮小と回転つきの描画命令を記録します。 |
| `ext.image.set_icon_sheet` | `set_icon_sheet(handle, cell_w, cell_h)` | `bool` | スプライトシートのセル情報を保存します。 |
| `ext.image.draw_icon` | `draw_icon(handle, index, x, y)` | `bool` | 設定済みのアイコンシートからスプライトを描画します。 |

### 3.8 `ext.audio`

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

### 3.9 `ext.vm`

必要 capability: なし

| 関数 | シグネチャ | 戻り値 | 説明 |
| --- | --- | --- | --- |
| `ext.vm.save` | `save(slot: int)` | `bool` | ランタイムのチェックポイントをメモリに保存します。 |
| `ext.vm.load` | `load(slot: int)` | `bool` | 以前保存したチェックポイントを復元します。 |

### 3.10 `state`

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
- `yield()` - 自発的に worker を譲ります
- `sleep()` - worker を睡眠状態に移します

## 5. 実用上の補足

- `wmtoolchain` は現在のサブセットを archive にまとめます。
- `wmfrontend` は `native` / `wasm` / `egui` で同じプロジェクトを実行できます。
- `egui` フロントエンドの既定フォントは、日本語向けに Noto Sans 系です。
- 既読フラグは `state` で管理するのが素直です。たとえば
  `read:chapter_1:0001` のようなキーを `state.set(...)` で保存し、
  既読判定では `state.has(...)` を見る、というルールにすると
  エンジン側で制御しやすくなります。
- エンジン主導のメッセージ窓では、`ext.message.choices_named(...)` を出し、
  `recv()` で応答を待って、`state.get("ui.last_choice")` /
  `state.get("ui.last_input")` を読む形が扱いやすいです。
- メッセージ窓の色や文字サイズも、`ext.message.box_style(...)`、
  `text_color(...)`、`speaker_color(...)`、`accent_color(...)`、
  `font_size(...)`、`reset_style()` で script 側から決められます。
- 互換用に、frontend は最新の応答を `ui.last_choice`、`ui.last_input`、
  `ui.last_reply` にも保存します。

## 6. 参照サンプル

次を参照してください。
- `samples/helloworld`
- `samples/inputlink`
- `samples/workercomm`
- `samples/engineworker`
- `samples/assetload`
- `samples/imageaudio`
- `samples/uiimage`
- `samples/easynovel`





