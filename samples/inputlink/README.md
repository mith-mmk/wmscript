# Input Link Sample

This sample shows a script that receives text input via the message window
and returns the submitted value.

Source:

```wms
export func main() {
    ext.message.clear();
    ext.message.log_clear();
    ext.message.prompt("Type any text");
    ext.message.show("InputLink", "Enter text and submit to return it from script.");
    recv();
    let value = state.get("ui.last_input");
    ext.message.prompt();
    return value;
}
```

Runtime behavior:

- The frontend shows a prompt and waits at `recv()`.
- Submitted input is normalized to `ui.last_input`.
- The script returns the submitted value.

Run examples:

- `cargo run -p wmfrontend -- samples/inputlink/main.wms --platform native`
- `cargo run -p wmruntime --example input_link`
