# VM仕様設計案（実装前提の形）
1. 設計目標

まずVMの責務を絞ります。

スクリプトの高速実行

小さい実装サイズ

ワーカー単位の独立実行

実処理はホストAPIに逃がす

実行時の名前解決をなくす

非同期はVM内部で複雑化せず、yield / sleep / message で表現する

これは、元仕様の

import の静的解決

関数ID / host_id の固定化

ワーカー分離

例外なし
という方針と一致しています。

2. VMの全体モデル
2.1 VM単位

1 Worker = 1 VM instance

各VMは以下を持つ

命令ポインタ pc

評価スタック stack

コールスタック frames

グローバル領域 globals

ヒープ heap

メッセージキュー msg_queue

状態 running / waiting / sleeping / halted / error

この分離は現行案どおりでよいです。ワーカー間メモリ共有禁止は維持した方が安全です。

2.2 実行方式

スタックベースVM

固定長命令 もしくは 準固定長命令

1フレームあたり N step だけ実行

yield, sleep, recv でスケジューラへ制御返却

推奨

固定長命令にこだわりすぎるより、次の形が実装しやすいです。

opcode: 1 byte

operand_a: 1 byte

operand_b: 2 byte

合計 4 byte / instruction

これなら

デコードが軽い

call func_id argc

jump addr

load_global idx
などをそのまま載せやすいです。

3. 値表現（Value Representation）

動的型なので、ここを最初に決めると実装が安定します。

3.1 値型

仕様上の型は以下でした。

int

float

bool

string

nil

array

table

handle

3.2 内部表現の推奨
案A: Tagged Value（素直で実装容易）
enum ValueTag {
  VAL_NIL,
  VAL_BOOL,
  VAL_INT,
  VAL_FLOAT,
  VAL_STRING,
  VAL_ARRAY,
  VAL_TABLE,
  VAL_HANDLE
};

struct Value {
  uint32_t tag;
  uint32_t aux;
  uint64_t payload;
};

利点:

実装が単純

デバッグしやすい

セーブ/ロード形式に落としやすい

欠点:

16 byte 程度になりやすい

案B: NaN boxing

より高速化寄り。ただし実装難度が上がるので、v0.1では非推奨です。

結論

最初は Tagged Value 一択でよいです。
この言語はVMの超微細最適化より、ホスト側に重い処理を逃がす設計の方が効きます。

4. メモリモデル
4.1 ヒープ

各VMごとに独立ヒープ

array / table / string / host handle wrapper を確保

他ワーカーに直接参照を渡さない

4.2 GC

元仕様では「GCは実装依存」ですが、実装開始時には方針を固定した方がいいです。

推奨

非移動 mark-sweep

実装が簡単

handle と相性がよい

ポインタ安定

FFI/ホスト連携が楽

初期ルート

評価スタック

コールフレームのローカル

グローバル

メッセージキュー内 payload

実行待ち future 的なものがあればその参照

将来

若世代GC

string intern

table の shape 最適化

は後回しで十分です。

5. コールフレーム設計
5.1 フレーム構造
struct CallFrame {
  uint32_t func_id;
  uint32_t return_pc;
  uint32_t base_sp;
  uint16_t local_count;
  uint16_t arg_count;
};
5.2 呼び出し規約

引数はスタックに積んで call func_id argc

callee 側でフレーム生成

ローカルは nil 初期化

return で戻り値 0 or 1 個

推奨ルール

多値返却なし

可変長引数なし

クロージャなし

再帰は深さ制限付き

元仕様の軽量化方針に合います。

6. グローバル / ローカル / 定数
6.1 グローバル

モジュール単位でグローバル領域を持つ

実行時には (module_id, global_index) でアクセス

load_global idx, store_global idx

6.2 ローカル

frame の base_sp 起点

load_local i, store_local i

6.3 定数プール

各モジュールごとに以下を持つ:

整数定数

浮動小数定数

文字列定数

関数参照

フィールド名ID

推奨

文字列は

コンパイル時に intern

定数プールに1回だけ載せる

7. バイトコード形式

元仕様に命令群の骨格はすでにあります。
ここではそれを実装可能な形式まで落とします。

7.1 モジュール構造
ModuleHeader
ConstPool
GlobalInfo
FunctionTable
CodeSection
DebugSection(optional)
7.2 ヘッダ例
struct ModuleHeader {
  char magic[4];      // "WBC0"
  uint16_t version;   // 0x0001
  uint16_t flags;
  uint32_t module_id;
  uint32_t const_count;
  uint32_t global_count;
  uint32_t func_count;
  uint32_t code_size;
};
7.3 関数テーブル
struct FunctionInfo {
  uint32_t func_id;
  uint32_t code_offset;
  uint16_t arg_count;
  uint16_t local_count;
  uint16_t stack_max;
  uint16_t flags;
};

stack_max を持たせると、実行前に必要スタックを確保しやすいです。

8. 命令セットの整理

元仕様の命令セットで十分ですが、いくつか整理すると扱いやすいです。

8.1 基本命令

nop

halt

8.2 定数/リテラル

push_const k

push_nil

push_true

push_false

8.3 変数

load_local i

store_local i

load_global i

store_global i

8.4 データアクセス

load_field k

store_field k

load_index

store_index

注意

load_field k の k は文字列ではなく field_id にするのがよいです。

8.5 スタック操作

pop

dup

8.6 算術・比較

add sub mul div mod neg

eq ne lt le gt ge not

追加推奨

and

or

論理演算を短絡評価にするなら、コンパイラ側で jump_if_false に落とす方が軽いです。
つまり VM命令に and/or を増やさずともよいです。

8.7 制御

jump addr

jump_if_false addr

jump_if_true addr

call func_id argc

call_host host_id argc

return

8.8 ワーカー/同期

send worker_id msg_type argc

recv

try_recv

yield

sleep

8.9 生成

new_array n

new_table n

追加推奨

len

type_of

ただし、最初は host API に寄せてもよいです。

9. メッセージモデル

元仕様では message は以下でした。

message {
    type: int
    from: int
    payload: value
}

この形で問題ありません。
ただしVM実装では次を決めた方がよいです。

9.1 送信仕様

send worker_id msg_type argc

スタック末尾の argc 個を payload とする

payload は

単一値

もしくは array に束ねる

推奨

payloadは常に1 value
複数引数はコンパイラで array/table に包む。

理由:

VM命令が簡単

メッセージキューが単純

互換性維持が楽

9.2 ワーカー間コピー

プリミティブは値コピー

array/table は 深いコピー か 送信禁止

handle は原則送信禁止、またはシリアライズ不能

推奨

v0.1では

int/float/bool/string/nil のみ安全送信

array/table は将来対応

handle は送信禁止

これが最も事故が少ないです。

10. Host API 呼び出し

仕様の要点は 実行時文字列解決禁止、ID呼び出しのみ です。ここは絶対維持です。

10.1 host_id テーブル

起動時に package ごとに host API を確定します。

struct HostFuncInfo {
  uint16_t host_id;
  uint16_t min_argc;
  uint16_t max_argc;
  uint16_t flags;   // pure, async, may_yield, capability
  HostFuncPtr fn;
};
10.2 呼び出し契約

VMがスタックから argc 個を読む

必要ならデバッグ時のみ型検査

戻り値は 0 or 1 個

エラーは例外ではなく nil / status table / error code

元仕様どおり、例外機構は持たない方がVMはきれいです。

10.3 capability

これはかなり重要です。
workerごとに

frontend: img/audio/input 可

backend: state 可

loader: asset/ext.fs 可

のように制限する設計は非常に良いです。

VMレベルでは

if (!(worker->capability_mask & host_func.required_capability)) {
    runtime_error(ERR_CAPABILITY);
}

で十分です。

11. エラー処理

例外なしという前提は維持。

ただし、VM内部エラー と スクリプト上の通常エラー を分けた方がよいです。

11.1 通常エラー

スクリプトが受け取るもの

nil

false

{ ok=false, error=... }

status code

11.2 VM致命エラー

VM自体が停止するもの

不正opcode

スタックアンダーフロー

無効func_id

capability違反

壊れたbytecode

許可されない型操作

推奨

ワーカー状態を error にし、エンジン側へ通知

worker_crash {
  worker_id,
  code,
  pc,
  func_id
}

クラッシュ隔離の思想と合います。

12. 検証器（Verifier）

実装前に、ロード時検証 を入れるとかなり安定します。

検証項目:

opcode 範囲

jump先の正当性

func_id の存在確認

定数インデックス範囲

stack_max の整合

host_id の存在確認

capability 要件との整合

これは軽量VMでもかなり効果があります。
「VMを賢くしない」方針を壊さず、ロード時だけ安全確認できます。

13. スケジューラ仕様
13.1 状態遷移

running

waiting_msg

sleeping(until_time)

halted

error

13.2 実行ループ
for each frame:
  for each runnable worker:
    execute up to step_budget
    if yield/sleep/recv/halt/error then stop
13.3 recv の意味

recv

メッセージがなければ waiting_msg に遷移

try_recv

メッセージがなければ nil

この仕様で十分明確です。

14. セーブ/ロードとの関係

ノベルゲーム用途を考えると、ここは早めに決める価値があります。
元仕様でも state/save/load は重視されています。

推奨

VMそのものの完全スナップショット保存は、v0.1ではやらない。

保存対象:

state テーブル

必要な進行位置

シナリオ側の明示状態

保存しない:

生の call stack

host handle

sleep中タイマ

生メッセージキュー

理由:

VM snapshot は実装が重い

バージョン互換が壊れやすい

デバッグが難しい

つまり
セーブはゲーム状態保存であってVM保存ではない
と割り切るのが安全です。

15. 最低限必要な制約

元仕様にも一部ありますが、VM視点で明文化するとこうです。

最大スタック深さ

最大再帰深さ

最大locals数 / function

最大globals数 / module

最大const数 / module

最大array要素数

最大tableエントリ数

1フレームあたり最大命令数

1メッセージ最大payloadサイズ

最大文字列長

推奨初期値

stack depth: 1024

call depth: 64

locals: 255

globals: 65535

consts: 65535

string length: 1MB 未満

message payload: 64KB 程度

16. v0.1で削るべきもの

軽量VMを維持するなら、最初は切った方がいいものがあります。

例外

クロージャ

GC世代管理

共有メモリ

反射

動的import

実行時名前解決

ブロッキングI/O

VMレベル await

複雑な継続オブジェクト

これは現行方針と一致しています。

実装用に固めた最終形
VMコア仕様

スタックベース

4byte命令

1 Worker = 1 VM

各VMは独立ヒープ/スタック/メッセージキュー

動的型、Tagged Value

非移動 mark-sweep GC

例外なし

host API は call_host host_id argc

capability によるAPI制限

ロード時 verifier あり

協調スケジューリング

1 payload message モデル

ここを次に定義すると実装へ進める

バイトコードバイナリ形式

FunctionInfo / ModuleHeader の正確な構造体

Value の内部表現

host_id / ext_id 割当表

verifier の検査仕様

GCルート列挙仕様

メッセージで送れる型の制限

必要なら次は
「命令セットを opcode 番号付きで表に落とした版」
として、そのまま C/C++ 実装に入れる形で切ります。
