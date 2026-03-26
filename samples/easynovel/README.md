# Easy Novel Sample

This sample is a tiny story-driven script. It keeps the structure of a visual-novel
style project, but stays within the compiler's current expression and function model.
The runtime example chooses which chapter to run via a command-line argument.

Source:

```wml
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Aki arrives at the last train platform.";
}

export func chapter_1() {
    return "A lantern lights the stairs down to the station.";
}

export func chapter_2() {
    return "Aki chooses the quiet route home.";
}

export func main() {
    return "Prologue";
}
```

Notes:

- The current compiler can emit these functions directly because each body is a
  simple `return` expression.
- The sample is intentionally written to be easy to extend with branching later.

Run examples:

- `cargo run -p wmlruntime --example easynovel`
- `cargo run -p wmlruntime --example easynovel -- chapter_1`
- `cargo run -p wmlruntime --example easynovel -- chapter_2`
