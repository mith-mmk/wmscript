# WMLScript 言語仕様書 v0.1 (2026-03-17)
1. 概要

WMLScript(W-Multi Langugae) は、軽量かつ高速なゲーム向け組み込みスクリプト言語である。

本言語は以下を目的とする：

小規模 VM による高速実行

フロントエンド / バックエンド / ローダの分離実行

マルチワーカー構成による非同期処理

低コストなバイトコード実行

明確な責務分離（ロジック vs 実処理）

- ノベルゲーであれば分岐と垂れ流しだけを続ける。保存、ロード、テキストボックスやエンジンの初期化はバックグラウンドscriptが全部やる
- スクリプトは分割管理出来る様にする


## ブートストラップAPI
分割はこの3層で整理すると安定します：

1. ファイル単位（module）

→ コンパイル単位

2. ワーカー単位（package）

→ 実行単位

3. アセット単位（bundle）

# 言語仕様
→ 配布・ロード単位
1. モジュール（ファイル分割）
基本仕様
import "path/to/module";
特徴

コンパイル時に完全解決

ランタイムロードしない

循環参照は禁止

スコープルール
デフォルト：非公開
let internal_value = 10;

他モジュールから見えない

export 指定
export func add(a, b) {
    return a + b;
}

export let version = 1;
import の挙動
import "math/util";

math/util.wms を読み込む

exportされたシンボルのみ可視

名前解決
util.add(1, 2);

or

import "math/util" as m;

m.add(1, 2);
推奨仕様（高速のため）

import は IDにコンパイル

シンボルも インデックス化

つまり実行時は：

call func_id=17
2. パッケージ（ワーカー単位）

ここが今回の設計のコアです ⚙️

パッケージ定義ファイル
package "ui" {
    entry: "ui/main"
}

package "engine" {
    entry: "engine/main"
}

package "loader" {
    entry: "loader/main"
}
特徴

1パッケージ = 1ワーカー

エントリスクリプトを持つ

importはパッケージ内で完結

ワーカー生成
worker.spawn("ui", "ui/main", "init");

または起動時に固定生成

パッケージ分離の利点

メモリ分離

クラッシュ隔離

非同期実行

責務分離

3. バンドル（配布単位）

これは実装寄りですが重要です。

構造例
bundle/
  manifest.json
  scripts/
    ui/
    engine/
    loader/
  assets/
    images/
    audio/
manifest
{
  "packages": [
    { "name": "ui", "entry": "ui/main" },
    { "name": "engine", "entry": "engine/main" },
    { "name": "loader", "entry": "loader/main" }
  ]
}
ポイント

スクリプトとアセットを同梱

パスは仮想パスで解決

実行時I/Oを減らす

4. importの設計詳細（重要）

軽量化のため、importは次の制約を推奨：

4.1 動的import禁止
// NG
import path_var;

理由：

最適化不能

バンドル化困難

VM肥大化

4.2 importはトップレベルのみ
// OK
import "math/util";

// NG
if (cond) {
    import "math/util";
}
4.3 import順序保証なし

依存関係はコンパイラが解決

5. 循環依存の扱い
v0.1仕様

👉 禁止

理由：

初期化順が複雑

VM簡素化を優先

将来拡張（参考）

forward宣言

遅延バインド

6. 初期化順序
モジュール

依存関係順にロード

グローバル初期化

init() 呼び出し

パッケージ

worker生成

entryモジュールロード

init()

7. コンパイルパイプライン

分割設計と密接に関係します

フロー
parse
→ import解決
→ シンボル解決
→ IR生成
→ 最適化
→ bytecode生成
→ ID割当
最適化ポイント

関数ID固定化

グローバルインデックス化

host呼び出しID化

message type ID化

8. 実行時の見え方

実行時にはこうなります：

[Worker UI]
  module_id=1
  func_id=10

[Worker Engine]
  module_id=2
  func_id=3
9. 例
ファイル構成
engine/
  main.wms
  state.wms
  event.wms
main.wms
import "engine/state" as s;
import "engine/event" as e;

func init() {
    s.init();
}

func update() {
    e.update();
}
state.wms
export func init() {
    state.set("hp", 100);
}
event.wms
export func update() {
    if (input.key_down("ok")) {
        worker.send("ui", 1, 0);
    }
}
10. 追加したい制約（重要）

軽量化のためにかなり効きます：

10.1 グローバル数制限

→ テーブル肥大防止

10.2 モジュールサイズ制限

→ キャッシュ効率向上

10.3 再帰制限

→ スタック制御簡略化

まとめ

この言語の分割設計はこう整理すると安定します：

コア思想

モジュール = コンパイル単位

パッケージ = 実行単位（ワーカー）

バンドル = 配布単位

軽量化の鍵

importは完全静的

名前解決はすべてID化

ワーカーで責務分離

ランタイムローダを作らない

この設計にしておくと、

コンパイラがシンプル

VMが小さい

非同期処理が自然

アセット管理と相性良い

という状態になります。

次の段階としては、

バイトコードフォーマット定義

シンボルテーブル設計

パッケージローダ実装

この3つを詰めると、実装に入れる状態になります。

## 実行モデル

1. 実行モデル
2.1 ワーカー

プログラムは複数のワーカー（独立VM）上で実行される。

各ワーカーは以下を持つ：

独立したメモリ空間

実行スタック

メッセージキュー

実行状態（running / waiting / sleeping）

2.2 スケジューリング

協調スケジューリング方式

明示的な yield により制御を返す

各フレームで一定ステップ数のみ実行

2.3 エントリポイント

各スクリプトは以下の関数を定義できる：

```
func init()
func update()
func on_message(msg)

init: 初期化時に1回呼ばれる

update: 毎フレーム呼ばれる

on_message: メッセージ受信時に呼ばれる
```


3. 型システム
3.1 基本型

型名	説明
int	64bit整数
float	32bit浮動小数
bool	真偽値
string	UTF-8文字列
nil	null値

3.2 複合型
型名	説明
array	動的配列
table	キー・値マップ
handle	外部リソース参照

3.3 型特性

動的型付け

実行時タグによる型判定

nil は未定義値として扱う

4. 文法
4.1 プログラム構造

```
program      := { decl }

decl         := import_decl
              | const_decl
              | global_decl
              | func_decl
```

4.2 宣言

```
import_decl  := "import" STRING ";"

const_decl   := "const" IDENT "=" expr ";"

global_decl  := "let" IDENT [ "=" expr ] ";"
4.3 関数
func_decl    := "func" IDENT "(" [ params ] ")" block

params       := IDENT { "," IDENT }
```

4.4 文

```
stmt :=
    block
  | "if" "(" expr ")" stmt [ "else" stmt ]
  | "while" "(" expr ")" stmt
  | "loop" stmt
  | "break" ";"
  | "continue" ";"
  | "return" [ expr ] ";"
  | "select" "(" expr ")" "{"
        { "case" literal ":" { stmt } }
        [ "default" ":" { stmt } ]
    "}"
  | "let" IDENT [ "=" expr ] ";"
  | assign ";"
  | expr ";"
```

4.5 式

```
expr :=
    literal
  | IDENT
  | call
  | unary
  | binary
  | "(" expr ")"
```

4.6 関数呼び出し

```
call :=
    IDENT "(" [ args ] ")"
  | IDENT "." IDENT "(" [ args ] ")"

args := expr { "," expr }
```

4.7 代入

```
assign := lvalue "=" expr

lvalue :=
    IDENT
  | IDENT "." IDENT
  | IDENT "[" expr "]"
```


5. 演算子
5.1 算術演算

```
+  -  *  /  %
```

5.2 比較演算

```
==  !=  <  <=  >  >=
```

5.3 論理演算

```
!  &&  ||
```

6. メッセージシステム
6.1 メッセージ構造

```
message {
    type: int
    from: int
    payload: value
}
```

6.2 送受信

```
worker.send(target, type, payload)
worker.recv()
worker.try_recv()
```

6.3 同期制御

```
system.yield()
system.sleep(ms)
```

7. 組み込み API
7.1 system

```
system.exit(code)
system.debug(value)
system.time()
system.random()
system.crypt_random()
system.yield()
system.sleep(ms)
```

7.2 worker

```
worker.spawn(name, script, entry)
worker.id()
worker.send(target, type, payload)
worker.recv()
worker.try_recv()
worker.name()
```

7.3 state

```
state.save(slot)
state.load(slot)
state.has(key)
state.get(key)
state.set(key, value)
state.erase(key)
```

7.4 asset

```
asset.request(path)
asset.preload(path)
asset.status(handle)
asset.release(handle)
asset.pin(handle)
asset.unpin(handle)
```

7.5 img

```
img.screen(w, h)
img.info(handle)
img.draw(handle, x, y)
img.draw_part(handle, sx, sy, sw, sh, dx, dy)
img.draw_ext(handle, sx, sy, sw, sh, dx, dy, dw, dh, rot, alpha)
img.set_icon_sheet(handle, cell_w, cell_h)
img.draw_icon(handle, index, x, y)
```

7.6 audio

```
audio.load(path)
audio.play(handle, loop=false)
audio.stop(handle)
audio.pause(handle)
audio.seek(handle, ms)
audio.volume(handle, v)
audio.release(handle)
```

7.7 text

```
text.box(x, y, w, h)
text.font(box, name, size)
text.print(box, str)
text.rich(box, rich)
text.clear(box)
text.release(box)
```

7.8 input

```
input.key(code)
input.key_down(code)
input.key_up(code)
input.mouse_x()
input.mouse_y()
input.click(button)
```

8. VM仕様
8.1 モデル

スタックベース VM

バイトコード実行

固定長命令

8.2 命令セット

```
nop
halt

push_const k
push_nil
push_true
push_false

load_local i
store_local i
load_global i
store_global i

load_field k
store_field k
load_index
store_index

pop
dup

add sub mul div mod neg

eq ne lt le gt ge not

jump addr
jump_if_false addr
jump_if_true addr

call func_id argc
call_host host_id argc
return

send worker_id msg_id argc
recv
try_recv

yield
sleep

new_array n
new_table n
```

9. メモリモデル

各ワーカーは独立ヒープ

GCは実装依存

handleはVM外リソース参照

10. エラー処理

例外機構は持たない

エラーは以下で扱う：

nil
statusコード
戻り値

11. 制約

ワーカー間のメモリ共有は禁止

外部リソースは handle 経由でのみアクセス

組み込みAPI以外のI/Oは禁止

12. 設計原則

言語は最小限に保つ

重い処理はホスト側に委譲

非同期処理はワーカー分離で実現

文字列解決はコンパイル時にID化

実行時分岐を減らす

13. 典型実行フロー
```
init()
↓
update() (毎フレーム)
↓
yield / sleep / recv
↓
再開
```
14. 例

```
func update() {
    if (input.key_down("ok")) {
        worker.send("engine", 1, 0);
    }

    system.yield();
}
```

## APIの拡張(Extend API API)
 API拡張はこの3層で分けるのが安定です：

1. Core API（固定）

→ 変更しない

2. Native Extension（ホスト拡張）

→ エンジン側で追加

3. Script Extension（スクリプト合成）

→ 言語内で拡張

1. Core API（固定仕様）

これは v0.1 で定義済みのもの：

system

worker

state

asset

img

audio

text

input

👉 ここは増やさない・壊さない

理由：

VM最適化対象

ID固定できる

バージョン互換の核

2. Native Extension（最重要）
基本構造
ext.<namespace>.<function>()
例
ext.physics.raycast(x1, y1, x2, y2)
ext.net.send(addr, data)
ext.ai.eval(state)
VM側の扱い
call_host ext_id, argc

ext_id はコンパイル時に解決

名前解決は実行時に行わない

登録モデル

エンジン側で：

register_extension("physics", {
    "raycast": fn_ptr_1,
    "overlap": fn_ptr_2
});
重要制約
1. 名前は固定文字列
// OK
ext.physics.raycast()

// NG
ext[name].func()

→ コンパイル時ID化のため

2. 型はプリミティブ＋handleのみ
int / float / bool / string / handle / array / table

→ VM簡素化

3. 同期/非同期を明確化
ext.fs.read(path)        // 同期（小サイズ）
ext.fs.read_async(path)  // 非同期

または：

let req = ext.fs.read_async(path);
await(req);
3. Script Extension（言語内拡張）
単純ラップ
export func play_se(name) {
    let h = audio.load(name);
    audio.play(h);
}
モジュール合成
import "ui/button";
import "ui/layout";

export func create_menu() {
    // 組み合わせ
}
ここで重要なルール

👉 Script Extension は ゼロコストであること

つまり：

インライン化可能

追加VM命令を増やさない

4. Namespace設計（重要）

拡張APIが増えても崩れない構造にする

推奨構造
system.*
worker.*
state.*

asset.*
img.*
audio.*
text.*
input.*

ext.*
ext 内の階層
ext.physics.*
ext.net.*
ext.fs.*
ext.debug.*
ext.platform.*
5. バージョニング戦略

ここを決めておくと後が楽です 📦

方式
ext.physics@1.raycast()
ext.physics@2.raycast()

または

import "ext/physics@1";
軽量優先なら

👉 エンジン側で固定
（スクリプトには見せない）

6. Capability制御（重要）

ワーカーごとに API を制限する

例
frontend worker
img / audio / input OK
state NG
fs NG
backend worker
state OK
img NG
loader worker
asset OK
img decode OK
実装方法

VMに「許可テーブル」を持たせる

if (!capability[func_id]) {
    error
}
7. 高速化のための設計

ここがかなり効きます 🚀

7.1 API呼び出しを整数化
img.draw → host_id=12
7.2 引数チェックを最小化

デバッグビルドのみチェック

リリースはスキップ

7.3 分岐を減らす
call_host 12, argc=3

→ switchで直接関数呼び出し

7.4 データコピーを避ける

handle中心

array/tableは参照渡し

8. 非同期APIパターン

マルチワーカー設計と統合する

パターン1：メッセージ型
worker.send("loader", MSG_LOAD, path)
パターン2：ハンドル型
let h = asset.request(path)

while (asset.status(h) != READY) {
    system.yield()
}
パターン3：Future型（拡張）
let f = ext.net.fetch(url)
let result = await(f)
9. エラー設計

例外を使わない前提

パターン
let h = asset.request(path)

if (h == nil) {
    // エラー
}

または

let r = ext.fs.read(path)

if (r.ok) {
    r.value
} else {
    r.error
}
10. API拡張の制約まとめ

軽量維持のためのルール：

動的ディスパッチ禁止

文字列検索禁止（実行時）

reflection禁止

GC圧迫するAPI禁止

ブロッキングI/O禁止

11. 最小拡張テンプレ

新しいAPIを追加するときの形：

ext.<domain>.<action>(args...)

例：

ext.camera.move(x, y)
ext.camera.zoom(v)
ext.camera.shake(power, time)
12. まとめ

API拡張の本質はここです：

軽量を保つ鍵

名前解決はコンパイル時

実行時はID呼び出しのみ

VMは一切賢くしない

拡張性の鍵

ext 名前空間

ホスト登録方式

ワーカーごとの capability

スケールさせる鍵

module分割

worker分離

非同期前提

この設計にしておくと、

機能をどれだけ増やしてもVMは肥大化しない

エンジン側で自由に拡張できる

スクリプトは常に軽い

状態を維持できます。

次に詰めると一気に実装に入れるのは：

host_id割当仕様

ext登録テーブル構造

await/futureの正式仕様

このあたりです。