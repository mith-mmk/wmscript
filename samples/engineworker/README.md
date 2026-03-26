# Engine/Worker Split Sample

This sample shows the intended separation:

- the engine worker streams dialogue commands
- the ui worker owns the message window and choices

Source:

```wml
worker engine {
    send 2, "show", "Narrator", "The engine worker streams dialogue without touching window state.", "choices", "prologue", "chapter_1", "chapter_2";
}

worker ui {
    recv();
    return "UI worker owns the message window.";
}
```

Runtime behavior:

- The engine worker emits a single command payload.
- The UI worker consumes the payload and would translate it into window updates in a full engine.
- This keeps the game script focused on content, not window plumbing.
