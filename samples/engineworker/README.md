# Engine-Style Message Control Sample

This sample shows the engine-side part of the separation:

- it owns read flags through `state`
- it controls message window mode through `ext.message`
- the frontend still owns the actual window layout and rendering
- it waits on `recv()` and branches from `state.get("ui.last_choice")`
- it then asks for a second input and branches from `state.get("ui.last_input")`

Source:

```wml
export func main() {
    if state.has("read:engineworker:intro") {
        ext.message.skip(true);
    } else {
        ext.message.skip(false);
    }
    state.set("read:engineworker:intro", true);
    ext.message.show(
        "Engine",
        "The engine-side script keeps the read flag in state and leaves window layout to the frontend."
    );
    ext.message.choices("prologue", "chapter_1", "chapter_2");
    ext.message.prompt("Choose a chapter");
    recv();
    if state.get("ui.last_choice") == "choice-1" {
        state.set("read:engineworker:prologue", true);
        ext.message.show("Engine", "Prologue selected.");
        ext.message.prompt("Enter the hero name");
        recv();
        if state.get("ui.last_input") == "Aki" {
            return "Narrator: Prologue selected.\nAki: I'm ready to start.";
        }
        return "Narrator: Prologue selected.\nNarrator: The hero name was not Aki.";
    } else if state.get("ui.last_choice") == "choice-2" {
        state.set("read:engineworker:chapter_1", true);
        ext.message.show("Engine", "Chapter 1 selected.");
        ext.message.prompt("Enter the scene name");
        recv();
        if state.get("ui.last_input") == "station" {
            return "Narrator: Chapter 1 selected.\nNarrator: The station scene opens.";
        }
        return "Narrator: Chapter 1 selected.\nNarrator: The scene name was different.";
    } else if state.get("ui.last_choice") == "choice-3" {
        state.set("read:engineworker:chapter_2", true);
        ext.message.show("Engine", "Chapter 2 selected.");
        ext.message.prompt("Enter the route name");
        recv();
        if state.get("ui.last_input") == "river" {
            return "Narrator: Chapter 2 selected.\nNarrator: The river route opens.";
        }
        return "Narrator: Chapter 2 selected.\nNarrator: The route name was different.";
    }
    return "Narrator: No chapter was selected.";
}
```

Runtime behavior:

- The script marks the intro as read in `state`.
- Re-running it turns on skip mode for the frontend message window.
- The frontend still renders the actual choice and message panels.
- Clicking a choice stores the selection in `ui.last_choice` and wakes the waiting worker.
- The worker resumes, reads `state.get("ui.last_choice")`, and branches to the selected chapter text.
- The sample then asks for a second input and reads it from `ui.last_input`.
- The two-worker split example lives in `crates/wmlruntime/examples/engine_worker_split.rs`.
