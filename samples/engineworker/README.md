# Engine-Style Message Control Sample

This sample shows the engine-side part of the separation:

- it owns read flags through `state`
- it controls message window mode through `ext.message`
- the frontend still owns the actual window layout and rendering
- it waits on `recv()` and branches from `state.get("ui.last_choice")`

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
        return "Narrator: The prologue is now marked as read.\nNarrator: The engine waited for the choice and branched from state.";
    } else if state.get("ui.last_choice") == "choice-2" {
        state.set("read:engineworker:chapter_1", true);
        return "Narrator: Chapter 1 is now marked as read.\nNarrator: The engine waited for the choice and branched from state.";
    } else if state.get("ui.last_choice") == "choice-3" {
        state.set("read:engineworker:chapter_2", true);
        return "Narrator: Chapter 2 is now marked as read.\nNarrator: The engine waited for the choice and branched from state.";
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
- The two-worker split example lives in `crates/wmlruntime/examples/engine_worker_split.rs`.
