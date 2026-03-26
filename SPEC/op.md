# オペコード
## 責務
- VM が解釈する opcode 番号と operand 形式を定義する。
- デコードと実行ループの共通言語を定義する。

## 依存
- 実行時の意味は [SPEC/vm.md](vm.md) に依存する。
- `CALL_HOST` の host_id 定義は [SPEC/hostapi.md](hostapi.md) に依存する。

オペコード一覧（Rust VM用）
0. 基本設計

opcode: u8

operand:

u8 / u16 / u32

命令長:

可変長（推奨）

デコードで pc += len

Rust側デコード後
enum Op {
    Nop,
    Halt,

    PushConst(u16),
    PushNil,
    PushTrue,
    PushFalse,

    LoadLocal(u8),
    StoreLocal(u8),

    LoadGlobal(u16),
    StoreGlobal(u16),

    LoadField(u16),
    StoreField(u16),

    LoadIndex,
    StoreIndex,

    Pop,
    Dup,

    Add, Sub, Mul, Div, Mod,
    Neg,

    Eq, Ne, Lt, Le, Gt, Ge,
    Not,

    Jump(u32),
    JumpIfFalse(u32),
    JumpIfTrue(u32),

    Call(u16, u8),
    CallHost(u16, u8),
    Return,

    NewArray(u16),
    NewTable(u16),

    Send(u16, u8),
    Recv,
    TryRecv,

    Yield,
    Sleep,
}
1. オペコード番号割り当て
1.1 制御系
0x00 NOP
0x01 HALT

1.2 リテラル
0x10 PUSH_CONST   u16
0x11 PUSH_NIL
0x12 PUSH_TRUE
0x13 PUSH_FALSE

1.3 ローカル変数
0x20 LOAD_LOCAL   u8
0x21 STORE_LOCAL  u8

1.4 グローバル変数
0x22 LOAD_GLOBAL  u16
0x23 STORE_GLOBAL u16

1.5 フィールド / テーブル
0x24 LOAD_FIELD   u16   ; field_id
0x25 STORE_FIELD  u16
0x26 LOAD_INDEX
0x27 STORE_INDEX

1.6 スタック操作
0x30 POP
0x31 DUP

1.7 算術
0x40 ADD
0x41 SUB
0x42 MUL
0x43 DIV
0x44 MOD
0x45 NEG

1.8 比較
0x50 EQ
0x51 NE
0x52 LT
0x53 LE
0x54 GT
0x55 GE
0x56 NOT

1.9 制御フロー
0x60 JUMP           u32
0x61 JUMP_IF_FALSE  u32
0x62 JUMP_IF_TRUE   u32

1.10 呼び出し
0x70 CALL        u16 func_id, u8 argc
0x71 CALL_HOST   u16 host_id, u8 argc
0x72 RETURN

1.11 生成
0x80 NEW_ARRAY   u16 size_hint
0x81 NEW_TABLE   u16 size_hint

1.12 ワーカー / メッセージ
0x90 SEND      u16 worker_id, u8 argc
0x91 RECV
0x92 TRY_RECV

1.13 スケジューリング
0xA0 YIELD
0xA1 SLEEP

0xB0-0xFF 将来の予約

2. バイトコードフォーマット（実用形）
例: CALL_HOST
[0x71][host_id:u16][argc:u8]

Rustデコード:

let host_id = read_u16(code, pc + 1);
let argc = code[pc + 3];
pc += 4;
例: JUMP
[0x60][addr:u32]
例: LOAD_LOCAL
[0x20][index:u8]
3. デコード設計（Rust）
3.1 高速パターン（match直）
match opcode {
    0x00 => Op::Nop,
    0x01 => Op::Halt,

    0x10 => {
        let k = read_u16(code, pc+1);
        pc += 3;
        Op::PushConst(k)
    }

    0x70 => {
        let func = read_u16(code, pc+1);
        let argc = code[pc+3];
        pc += 4;
        Op::Call(func, argc)
    }

    _ => panic!("invalid opcode"),
}
3.2 事前デコード（推奨）
struct Decoded {
    op: Op,
}

ロード時に全部デコード:

実行時分岐削減

verifierと統合できる

4. スタック動作仕様（重要）
4.1 CALL

スタック:

[arg1][arg2]...[argN]

実行後:

フレーム作成

ローカル初期化

引数は local[0..argc]

4.2 CALL_HOST
[arg1..argN] -> host

戻り値:

[result] or nothing
4.3 LOAD_FIELD
[obj] -> [value]
4.4 STORE_FIELD
[obj][value] -> []
4.5 SEND
[val1..valN] -> send
5. 最低限の追加候補（必要なら）

Rust実装で後からほぼ必ず欲しくなる：

0xA2 LEN
0xA3 TYPE_OF

ただし最初は host に逃がしてOK。

6. 実装でハマるポイント（重要）
6.1 u8 / u16 / u32 の統一

endian: little endian固定

6.2 pc更新ミス

命令長をテーブル化するか

decode関数に閉じ込める

6.3 stack underflow

debug buildで必ずチェック

6.4 field_id

文字列じゃなく 整数ID
（ここ外すとVMが一気に重くなる）

6.5 error handle
- devided zeroなど
- Errorを保管する場所
- Errorをトラップする場所

7. 最終まとめ（Rust向け）

opcode = u8

operand = little endian

decodeして enum Op に変換

match で実行

workerごとにVMインスタンス

host呼び出しは CALL_HOST(host_id, argc)

次にやると実装が一気に進むのはこれ：

Value enum（Rust版）

VM struct（stack / frame / heap）

execute loop（match dispatch）

ここまで来ると、もう「動くVM」になる段階。
