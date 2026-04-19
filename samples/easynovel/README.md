# Easy Novel Sample

This sample is a small engine-driven novel flow built on the current
message-window API.

- `main()` opens a chapter menu with `ext.message.choices_named(...)`
- the selected chapter is read from `state.get("ui.last_choice")` after `recv()`
- the script records save/load layering by writing persistent state with `state.save(1)` and runtime checkpoint with `ext.vm.save(1)`
- each chapter pages through multiple screens with `ext.message.show(...)` and `recv()`
- read flags live in `state.*`
- the script swaps in a framed message window through `ext.message.frame(100)` and `ext.message.content_inset(...)`
- already-read chapters turn on `ext.message.skip(true)` until the chapter ends

Source:

```wms
export let protagonist = "Aki";
export let setting = "last train platform";

export func main() {
    ext.message.clear();
    ext.message.log_clear();
    ext.message.reset_style();
    ext.message.box_style(10, 16, 26, 228, 120, 188, 148, 255);
    ext.message.text_color(244, 246, 250, 255);
    ext.message.speaker_color(255, 232, 188, 255);
    ext.message.accent_color(162, 224, 206, 255);
    ext.message.font_size(20, 24);
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

    if chapter != nil {
        state.set("save.last_chapter", chapter);
        state.save(1);
        ext.vm.save(1);
    }

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
- The sample demonstrates layering: `state.save(1)` for persistent chapter markers and `ext.vm.save(1)` for runtime checkpoint resume.
- Re-running a chapter toggles skip mode from the engine script.
- Skip mode auto-advances plain pages until the chapter reaches a choice or input.

Run examples:

- `cargo run -p wmfrontend -- samples/easynovel/main.wms --platform egui --font noto --image ui/message_frame=samples/easynovel/message_frame.png`
- `cargo run -p wmfrontend -- samples/easynovel/main.wms --platform native --image ui/message_frame=samples/easynovel/message_frame.png`
- `cargo run -p wmruntime --example easynovel`






The sample ships with `message_frame.png`. When launched through `wmfrontend`, the first `--image` asset gets resource id `100`, so the script can bind it directly with `ext.message.frame(100)`.
