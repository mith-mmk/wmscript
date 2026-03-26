# Input Link Sample

This sample shows a script that reads a value from a host callback and returns it.

Source:

```wml
export func main() {
    return input();
}
```

Runtime behavior:

- The host provides a single string input.
- The script returns that string unchanged.
