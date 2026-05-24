# Canonical Draft Examples

These examples are compact patterns for LLM-generated scripts. They are based on
current samples, but shortened for clarity.

## Minimal Return

```wms
export func main() {
    return 1 + 2 * 3;
}
```

Reference: [../../samples/helloworld/main.wms](../../samples/helloworld/main.wms)

## Message And Input

```wms
export func main() {
    ext.message.clear();
    ext.message.log_clear();
    ext.message.show("InputLink", "Enter text and submit it.");
    ext.message.prompt("Type any text");
    recv();

    let value = state.get("ui.last_input");
    ext.message.prompt();
    return value;
}
```

Reference: [../../samples/inputlink/main.wms](../../samples/inputlink/main.wms)

## Named Choices

```wms
export func main() {
    ext.message.clear();
    ext.message.show("Guide", "Choose a route.");
    ext.message.choices_named(
        "north", "Go North",
        "south", "Go South"
    );
    recv();

    let route = state.get("ui.last_choice");
    ext.message.choices_named();

    if route == "north" {
        ext.message.show("Guide", "North road selected.");
        recv();
        return "north";
    } else if route == "south" {
        ext.message.show("Guide", "South road selected.");
        recv();
        return "south";
    }

    return "none";
}
```

Reference: [../../samples/messagewindow/main.wms](../../samples/messagewindow/main.wms)

## Static Import

```wms
import "chapter/part1.wms" as chapter;

export func main() {
    ext.message.clear();
    ext.message.show("SplitImport", "Import graph was resolved.");
    recv();
    return "split import ok";
}
```

Reference: [../../samples/splitimport/main.wms](../../samples/splitimport/main.wms)

## Smoke Commands

Use `.test*` directories for generated archives and temporary files.

```powershell
New-Item -ItemType Directory -Force .test-samples

cargo run -p wmfrontend --bin wmautoui -- samples/inputlink/main.wms `
  --platform egui `
  --input AI-INPUT `
  --expect AI-INPUT `
  --quiet

cargo run -p wmtoolchain --bin wmtoolchain -- samples/helloworld/main.wms `
  --out .test-samples/helloworld-cycle.warc

cargo run -p wmfrontend --bin wmfrontend -- .test-samples/helloworld-cycle.warc `
  --platform native
```

Full catalog: [../../samples/README.md](../../samples/README.md)

## Generation Checklist

- Use `export func main()` unless a sample requires another entry point.
- Use semicolons.
- Use `recv()` after message windows when waiting for user progression.
- Read frontend replies through `state.get("ui.last_choice")` and
  `state.get("ui.last_input")`.
- Keep imports static and relative.
- Avoid unimplemented syntax even if it exists in other languages.
- Label generated code as targeting the current draft WMScript surface.
