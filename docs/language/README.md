# WMScript v2 authoring notes

Use typed persistent schemas for saved game data, tasks for waiting flows, and
synchronous systems for deterministic world updates. The executable examples
under `samples/` are the canonical source reference.

Do not invent JavaScript/Python syntax or use legacy `ext.*` calls. Validate
generated source with `cargo run -p wms -- check <project>`.
