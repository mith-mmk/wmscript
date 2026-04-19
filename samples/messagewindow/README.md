# Message Window Sample

This sample is a focused engine-side message window example.

- it uses `ext.message.show(...)` for plain pages
- it advances those pages with `recv()`
- it uses `ext.message.choices_named(...)` for stable engine-defined choice ids
- it uses `ext.message.prompt(...)` for follow-up input
- it reads standardized input ABI keys (`ui.last_choice`, `ui.last_input`) after `recv()`
- it uses `ext.message.log_clear()` to reset the text log at scene start
- it clears prompt and choice state explicitly from the script

Source:

```wms
export func main() {
    ext.message.clear();
    ext.message.log_clear();
    ext.message.speed(28);
    ext.message.auto(false);
    ext.message.show(
        "Narrator",
        "This sample is focused on the engine-driven message window.\nClick, press Enter, or use Next to advance."
    );
    recv();

    ext.message.show(
        "Narrator",
        "The engine script decides when to show text, choices, and input.\nThe frontend only renders the window."
    );
    recv();

    ext.message.choices_named(
        "north", "Go North",
        "south", "Go South"
    );
    ext.message.prompt("Choose a route");
    recv();
    let route = state.get("ui.last_choice");

    ext.message.choices_named();
    ext.message.prompt();

    if route == "north" {
        ext.message.show("Guide", "North road selected.\nEnter the companion name.");
        ext.message.prompt("Companion name");
        recv();
        let name = state.get("ui.last_input");
        ext.message.prompt();
        if name == "Mika" {
            ext.message.show("Narrator", "North road selected.\nMika joins the trip.");
        } else {
            ext.message.show("Narrator", "North road selected.\nA different companion joins the trip.");
        }
        recv();
        return;
    } else if route == "south" {
        ext.message.show("Guide", "South road selected.\nEnter the weather.");
        ext.message.prompt("Weather");
        let weather = recv();
        ext.message.prompt();
        if weather == "rain" {
            ext.message.show("Narrator", "South road selected.\nRain starts over the bridge.");
        } else {
            ext.message.show("Narrator", "South road selected.\nThe road stays clear.");
        }
        recv();
        return;
    }

    ext.message.show("Narrator", "No route was selected.");
    recv();
    return;
}
```

Runtime behavior:

- Plain text pages wait on `recv()`, so the engine script controls when the next page starts.
- `choices_named(...)` result is read from `state.get("ui.last_choice")` after `recv()`.
- Input is read from `state.get("ui.last_input")` after `recv()`.
- The sample uses `choices_named()` and `prompt()` with no args to clear those UI states explicitly.

Run examples:

- `cargo run -p wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto`
- `cargo run -p wmfrontend -- --demo messagewindow --platform egui --font noto`
