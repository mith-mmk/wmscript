# Easy Novel Sample

This sample is a tiny story-driven script. It keeps the structure of a visual-novel
style project, but stays within the compiler's current expression and function model.
The runtime example chooses which chapter to run via a command-line argument, and the
frontend message window renders the returned chapter text as a narration block.

Source:

```wml
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Narrator: The last train platform is almost empty.\nNarrator: Aki stops under the lantern light and listens to the rails.\nAki: The city feels farther away than usual.";
}

export func chapter_1() {
    return "Narrator: A lantern lights the stairs down to the station.\nAki: The next train is still ten minutes away.\nNarrator: A quiet voice answers from the ticket gate.";
}

export func chapter_2() {
    return "Narrator: Aki chooses the quiet route home.\nAki: I'll take the river path tonight.\nNarrator: The station lights fade behind the empty road.";
}

export func main() {
    return "Narrator: Select a chapter from the runtime example.\nNarrator: prologue, chapter_1, or chapter_2.\nNarrator: The returned text will appear in the message window.";
}
```

Notes:

- The current compiler can emit these functions directly because each body is a
  simple `return` expression.
- The sample is intentionally written to be easy to extend with branching later.
- The frontend reads the final returned string and places it in the message window.

Run examples:

- `cargo run -p wmlruntime --example easynovel`
- `cargo run -p wmlruntime --example easynovel -- chapter_1`
- `cargo run -p wmlruntime --example easynovel -- chapter_2`
- `cargo run -p wmlfrontend -- samples/easynovel/main.wml --platform egui --font noto`
