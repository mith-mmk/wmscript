# Worker Communication Sample

This sample demonstrates one worker sending a string to another worker.

Source:

```wml
worker sender {
    send 2, "hello worker";
}

worker receiver {
    return recv();
}
```

Runtime behavior:

- Worker 1 sends a payload to worker 2.
- Worker 2 receives the payload and returns it.
