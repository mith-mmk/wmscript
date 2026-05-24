# WMScript Language Guide For LLMs

This page summarizes the current draft authoring surface. It is intentionally
conservative: when in doubt, use only syntax shown here or in current samples.

## File And Module Shape

WMScript source files use the `.wms` extension.

A module may contain:

- Static imports.
- Function declarations.
- Top-level `let` declarations.

```wms
import "shared/ui.wms" as ui;

export let title = "My Game";

export func main() {
    return 1 + 2 * 3;
}
```

## Imports

Imports are static string paths and must end with `;`.

```wms
import "chapter/part1.wms";
import "chapter/part1.wms" as chapter;
```

Do not generate dynamic imports. The compile model expects import resolution to
finish before runtime.

## Functions

Functions use `func name(params) { ... }`. Exported entry points use
`export func`.

```wms
export func main() {
    return "ok";
}
```

Current samples use `main()` as the primary entry point.

## Statements

The current function body surface includes:

- Expression statement: `expr;`
- Local binding: `let name = expr;`
- Empty return: `return;`
- Value return: `return expr;`
- Conditional blocks: `if expr { ... }`
- `else` and `else if`
- Infinite loop block: `loop { ... }`
- `break;`
- `continue;`

```wms
export func main() {
    let route = state.get("ui.last_choice");
    if route == "north" {
        return "north";
    } else if route == "south" {
        return "south";
    }
    return "unknown";
}
```

Avoid `while`, `for`, `match`, classes, closures, and user-defined structs. They
are not part of the current safe authoring surface.

## Expressions

Supported expression forms include:

- `nil`, `true`, `false`
- Integer and floating-point literals.
- String literals.
- Local variable references.
- Grouping with `(expr)`.
- Unary `-expr` and `!expr`.
- Binary arithmetic: `+`, `-`, `*`, `/`.
- Boolean operators: `&&`, `||`.
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`.
- Calls to supported host/runtime functions.

```wms
export func main() {
    let value = 1 + 2 * 3;
    if value >= 7 && value != 0 {
        return "ok";
    }
    return "ng";
}
```

## Calls

Runtime-facing calls are usually under `ext.*` or `state.*`.

```wms
ext.message.show("Narrator", "Hello.");
recv();
let choice = state.get("ui.last_choice");
```

Special VM-level calls used by the current surface include:

- `recv()`: wait for frontend/user/message progression.
- `try_recv()`: receive if available.
- `yield()`: voluntarily yield.
- `sleep()`: move the worker into sleeping state.

Prefer `recv()` for message-window progression in writer-facing examples.

## Comments And Formatting

Line comments use `//`.

```wms
// Show a line and wait for player progression.
ext.message.show("Guide", "Choose a route.");
recv();
```

Generated scripts should use simple indentation, explicit semicolons, and small
functions.

## Current Limitations

- Top-level `export let` is currently limited to literal-style values.
- No dynamic import or runtime module loading.
- No user-defined aggregate types in the current authoring surface.
- No exception model; errors usually return `nil`, `false`, a status value, or a
  status table depending on the runtime API.
- Do not assume arrays/tables have stable source syntax unless a current sample
  or implementation confirms it.
