# Status And Change Policy

WMScript is currently an experimental, writer-first game scripting language.
The first target is a practical pipeline from script plus assets, through the
toolchain, into a runtime or frontend that can replay the packaged result.

## Stability Level

- Language status: draft.
- Runtime API status: draft.
- Archive and package model: draft.
- Frontend behavior: implementation-led and subject to change.
- Compatibility guarantee: none unless a specific `SPEC/` document says
  otherwise.

## Source Priority

When documents conflict, use this order:

1. Current Rust implementation for the exact checked-out revision.
2. `SPEC/*.md` for intended architecture and constraints.
3. Root documentation such as `README.md`, `functions.md`, and `howtouse.md`.
4. This `docs/llm/` pack.
5. Older examples, notes, or generated answers.

For script authoring, prefer current samples and smoke commands over abstract
syntax guesses.

## Likely Breaking Areas

The following areas are expected to change:

- Higher-level game APIs under `ext.*`.
- Capability profiles for `native`, `egui`, and `wasm`.
- Resource loading and handle/request behavior.
- Save/load and checkpoint semantics.
- UI ownership between script, runtime, and frontend.
- Archive metadata and packaging options.
- Any syntax not already used by current samples.

## LLM Guidance

- Say "current draft surface" when describing language behavior.
- Avoid promising long-term compatibility.
- Do not invent unimplemented loops, collections, classes, imports, or async
  syntax.
- If generating code, keep it close to examples under `samples/`.
- If a feature appears in a `SPEC/` file but not in current samples or code,
  describe it as planned or intended rather than implemented.
- If exact behavior matters, instruct the user or agent to verify with
  `cargo run`, `cargo test`, or the relevant sample smoke command.
