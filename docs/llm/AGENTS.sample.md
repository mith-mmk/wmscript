# AGENTS.md Sample For WMScript Docs

This is a sample instruction file for agents working on WMScript scripts or
documentation. It is not active unless copied or adapted by a repository owner.

## Read Order

Before generating or editing WMScript, read:

1. `docs/llm/status-and-change-policy.md`
2. `docs/llm/language.md`
3. `docs/llm/runtime-functions.md`
4. `docs/llm/examples.md`
5. `SPEC/language.md` and `functions.md` when exact behavior matters.

## Working Rules

- Treat WMScript as a draft language.
- Do not invent syntax beyond the current docs, samples, or implementation.
- Prefer current samples under `samples/` as executable examples.
- Put temporary files, generated archives, and scratch data under `.test*`.
- Do not write environment-specific absolute paths, secrets, API keys, or logs
  outside ignored scratch locations.
- If docs and implementation conflict, trust the implementation first and record
  the conflict in the task result.

## Script Generation Rules

- Prefer `export func main()` for examples.
- Use explicit semicolons and simple control flow.
- Use `ext.message.*`, `recv()`, and `state.get("ui.last_*")` for message UI
  flows.
- Avoid `for`, `while`, `match`, classes, closures, dynamic imports, and
  collection literals unless current implementation evidence confirms them.
- Mention the target platform profile when using capability-gated APIs.

## Verification

Use existing smoke commands from `samples/README.md`. For generated archives,
write outputs under `.test-samples/` or another `.test*` directory and remove
them after use when they are no longer needed.
