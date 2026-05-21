# Worker Communication Sample

`main.wms` is kept as a current WMScript smoke sample:

```wms
export func main() {
    return "hello worker";
}
```

Run it with:

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/workercomm/main.wms --platform native
```

The actual low-level multi-worker send/recv example is currently a runtime
bytecode example:

```powershell
cargo run -p wmruntime --example worker_comm
```
