# WMScript v2 言語仕様

## 責務と固定境界

WMScript v2はノベルゲーム、RPG、RTS、シミュレーションを同じ言語と実行モデルで記述するための段階的静的型付き言語である。コンパイラはすべての構文をbytecode v1へ変換し、`wmvm`、`wmbytecode`、`wmverifier`の変更を要求してはならない。

## モジュール

- source extensionは`.wms`、project entryは`wms.toml`で指定する。
- `import "path" as name;`はcompile時だけ解決する。dynamic importはない。
- top-levelには`struct`、`enum`、`component`、`resource`、`event`、`func`、`task`、`system`、`on`、`test func`を宣言できる。
- import graphの循環、重複宣言、非公開symbol参照はcompile errorとする。

## 型

標準型は`nil`、`bool`、`int`、`float`、`string`、`handle`、`any`、`Array<T>`、`Option<T>`である。

- `struct`、`component`、`resource`、payload付き`event`は固定fieldを持つrecordとして型検査する。
- `enum`はvariant tagと任意payloadを持つ。
- local bindingは初期値から推論できる。公開関数parameter、公開return、永続component/resource fieldは明示型を必須とする。
- `any`から具体型への暗黙narrowingを禁止する。typed host APIを利用して具体型を取得する。
- `nil`は`Option<T>`または`any`だけに代入できる。
- `int`から`float`だけを安全な暗黙数値変換とする。

VM loweringではArrayを`Value::Array`、recordをcompile時に割り当てた`u16` field IDを持つ`Value::Table`、enumをtag fieldとpayload fieldを持つTableへ変換する。

## 宣言

```wms
component Position persistent {
    x: int,
    y: int,
}

resource Clock persistent {
    tick: int = 0,
}

event Move {
    entity: int,
    dx: int,
    dy: int,
}

enum Direction { North, East, South, West }
```

- `persistent`を付けたcomponent/resourceだけがsave対象になる。
- transient型からpersistent型への参照は禁止する。
- Entity IDはscript上`int`、runtime上は単調増加する`u64`として扱う。

## 関数、task、system

- `func`: 同期関数。`await`、`yield`、event waitは禁止。
- `task`: 中断可能関数。`await`を使用でき、コンパイラが再開状態機械へ変換する。
- `system`: fixed tickまたはeventからruntimeが呼ぶ同期関数。副作用はWorld、event emit、標準portだけに限定し、`await`を禁止する。
- `on start|tick|input|message|save|load`: runtime entry handler。handler bodyはtaskと同じ待機規則を持つ。
- `test func`: headless runnerだけが検出して実行する同期test entry。

```wms
task intro() -> string {
    ui.say("Guide", "Choose a route");
    let route = await input.choice(["north", "south"]);
    return route;
}

system movement(event: Move) {
    let position = world.get<Position>(event.entity);
    position.x = position.x + event.dx;
    position.y = position.y + event.dy;
    world.set(event.entity, position);
}

on start {
    await intro();
}
```

## 文と式

- bindingと代入: `let name = expr;`、`let name: Type = expr;`、`target = expr;`
- 制御: `if/else`、`match`、`while`、`for name in expr`、`break`、`continue`、`return`
- expression: literal、array/record constructor、field/index、unary、binary、call、`await`
- `&&`と`||`はshort circuitする。比較は互換型間だけ許可する。
- array反復はindex昇順、world queryはEntity ID昇順、matchはsource記述順に評価する。

## 待機のlowering

- `await input.*`と`await message.*`は継続状態保存後に`Recv`を発行する。
- `await time.sleep(ticks)`は継続状態保存後に`Sleep`を発行する。
- `await game.next_tick()`は継続状態保存後に`Yield`を発行する。
- VM frame、local stack、program counterを継続状態として保持し、再開時に同じcall frameを継続する。
- systemとfunc内の`await`はcompile errorであり、暗黙blockingは禁止する。

## 標準モジュール

`core`、`game`、`world`、`time`、`random`、`input`、`scene`、`ui`、`audio`、`asset`、`save`を予約済み標準moduleとする。新sourceから`ext.*`と文字列key形式の`state.*`は参照できない。これらはWARC v1 legacy runtimeだけが提供する。

## 診断

diagnosticはcode、severity、message、source path、UTF-8 byte span、補助labelを持つ。parserは回復可能な位置で同期し、一度の`check`で複数diagnosticを返す。未知symbol、型不一致、永続化不能型、待機禁止、capability不足はpackage生成前に拒否する。
