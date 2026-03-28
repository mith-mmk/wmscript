# Easy Novel Sample

This sample is a small engine-driven novel flow built on the current
message-window API.

- `main()` opens a chapter menu with `ext.message.choices_named(...)`
- the selected chapter is read from `state.get("ui.last_choice")` after `recv()`
- each chapter pages through multiple screens with `ext.message.show(...)` and `recv()`
- read flags live in `state.*`
- already-read chapters turn on `ext.message.skip(true)` until the chapter ends

Source:

```wms
export let protagonist = "Aki";
export let setting = "last train platform";

export func main() {
    ext.message.clear();
    ext.message.log_clear();
    ext.message.speed(26);
    ext.message.auto(false);
    ext.message.choices_named(
        "prologue", "Prologue",
        "chapter_1", "Chapter 1",
        "chapter_2", "Chapter 2"
    );
    ext.message.prompt("Select a chapter");
    ext.message.show(
        "Narrator",
        "The station is quiet tonight.\nChoose a chapter to open the next page."
    );
    recv();
    let chapter = state.get("ui.last_choice");

    ext.message.choices_named();
    ext.message.prompt();

    if chapter == "prologue" {
        if state.has("read:easynovel:prologue") {
            ext.message.skip(true);
        } else {
            ext.message.skip(false);
        }
        state.set("read:easynovel:prologue", true);
        ext.message.show(
            "Narrator",
            "The last train platform is almost empty.\nAki stops under the lantern light and listens to the rails."
        );
        recv();
        ext.message.show("Aki", "The city feels farther away than usual.");
        recv();
        ext.message.skip(false);
        return;
    }

    ext.message.show("Narrator", "No chapter was selected.");
    recv();
}
```

Runtime behavior:

- The chapter menu is script-driven and rendered by the frontend.
- The selected chapter id is mirrored into `ui.last_choice` and read by the script after `recv()`.
- Re-running a chapter toggles skip mode from the engine script.
- Skip mode auto-advances plain pages until the chapter reaches a choice or input.

Run examples:

- `cargo run -p wmfrontend -- samples/easynovel/main.wms --platform egui --font noto`
- `cargo run -p wmfrontend -- samples/easynovel/main.wms --platform native`
- `cargo run -p wmruntime --example easynovel`
