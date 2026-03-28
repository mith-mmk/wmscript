# Engine-Style Message Control Sample

This sample shows the engine-side part of the separation:

- it owns read flags through `state`
- it controls message window mode through `ext.message`
- it resets the text log with `ext.message.log_clear()` before the scene starts
- the frontend still owns the actual window layout and rendering
- it pages plain message screens with `recv()`
- it uses named choices and then asks for a second text input

Source:

```wms
export func main() {
    ext.message.log_clear();
    if state.has("read:engineworker:intro") {
        ext.message.skip(true);
    } else {
        ext.message.skip(false);
    }
    state.set("read:engineworker:intro", true);
    ext.message.speed(30);
    ext.message.auto(false);
    ext.message.show(
        "Engine",
        "This sample drives the message window from the engine script.\nPress Next or Enter to continue."
    );
    recv();
    ext.message.show(
        "Engine",
        "The frontend still owns rendering and layout.\nThe script only emits message, choice, and input state."
    );
    recv();
    ext.message.choices_named(
        "prologue", "Prologue",
        "chapter_1", "Chapter 1",
        "chapter_2", "Chapter 2"
    );
    ext.message.prompt("Choose a chapter");
    let choice = recv();
    if choice == "prologue" {
        state.set("read:engineworker:prologue", true);
        ext.message.show("Engine", "Prologue selected.\nEnter the hero name.");
        ext.message.prompt("Hero name");
        let hero_name = recv();
        if hero_name == "Aki" {
            ext.message.show("Narrator", "Prologue selected.\nAki: I'm ready to start.");
        } else {
            ext.message.show(
                "Narrator",
                "Prologue selected.\nThe hero name was not Aki."
            );
        }
        recv();
        return;
    } else if choice == "chapter_1" {
        state.set("read:engineworker:chapter_1", true);
        ext.message.show("Engine", "Chapter 1 selected.\nEnter the scene name.");
        ext.message.prompt("Scene name");
        let scene_name = recv();
        if scene_name == "station" {
            ext.message.show("Narrator", "Chapter 1 selected.\nThe station scene opens.");
        } else {
            ext.message.show(
                "Narrator",
                "Chapter 1 selected.\nThe scene name was different."
            );
        }
        recv();
        return;
    } else if choice == "chapter_2" {
        state.set("read:engineworker:chapter_2", true);
        ext.message.show("Engine", "Chapter 2 selected.\nEnter the route name.");
        ext.message.prompt("Route name");
        let route_name = recv();
        if route_name == "river" {
            ext.message.show("Narrator", "Chapter 2 selected.\nThe river route opens.");
        } else {
            ext.message.show(
                "Narrator",
                "Chapter 2 selected.\nThe route name was different."
            );
        }
        recv();
        return;
    }
    ext.message.show("Narrator", "No chapter was selected.");
    recv();
    return;
}
```

Runtime behavior:

- The script marks the intro as read in `state`.
- Re-running it turns on skip mode for the frontend message window.
- Skip mode now auto-advances plain pages until a choice or input prompt appears.
- The frontend still renders the actual choice and message panels.
- The plain message pages advance with `Next` or `Enter`; if the page is still animating, the first action reveals it immediately.
- `ext.message.choices_named(...)` gives the engine stable choice ids such as `prologue` and `chapter_1`.
- Selecting a choice or submitting input wakes the waiting worker and returns that payload from `recv()`.
- The two-worker split example lives in `crates/wmruntime/examples/engine_worker_split.rs`.
