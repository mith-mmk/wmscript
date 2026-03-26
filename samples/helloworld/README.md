# Hello World Sample

This sample is intentionally tiny. It exercises the compiler front end,
constant folding, and the VM runtime with a single exported entry point.

Source:

```wml
export func main() {
    return 1 + 2 * 3;
}
```

Expected result:

- The optimizer folds the expression to `7`.
- The runtime returns `7` from `main`.
