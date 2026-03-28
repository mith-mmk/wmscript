# UI Image Demo

This sample renders the reference `samples/uiimage.png` scene layout through the frontend.

## Script

```wms
export func main() {
    ext.scene.reset();
    ext.scene.layout(240, 92, 520, 180, 18, 380, 1244, 130);
    ext.image.draw_ext(ext.image.load(100), 0, 0, 784, 565, 0, 0, 1280, 720, 0, 1);
    ext.message.show(
        "Narrator",
        "Message Window(設定で大きさ、形、色、透明度を変えられる)"
    );
    ext.message.choices("プロローグ", "第一章", "第二章");
    ext.message.prompt("入力できます");
    return "UI image layout demo";
}
```

## Expected Result

- A background image fills the stage.
- A choice panel appears near the top center.
- A message window appears across the bottom.
- The input field is shown inside the message window.
