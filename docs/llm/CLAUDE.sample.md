# CLAUDE.md Sample For WMScript Docs

This file is a sample prompt for Claude-style coding agents. It is documentation
only and does not change repository behavior.

## Project Context

WMScript is a draft writer-first game scripting language and runtime. The
current goal is to make scripts plus assets compile, package, and replay through
the WMScript toolchain and frontend.

## Required Reading

Read these files before generating WMScript:

- `docs/llm/status-and-change-policy.md`
- `docs/llm/language.md`
- `docs/llm/runtime-functions.md`
- `docs/llm/examples.md`

For exact behavior, also read:

- `SPEC/language.md`
- `SPEC/vm.md`
- `SPEC/hostapi.md`
- `functions.md`
- `samples/README.md`

## Behavior

- Keep answers and examples grounded in the current repository.
- State that generated scripts target the current draft surface.
- Do not assume compatibility with JavaScript, Lua, Python, or TypeScript.
- Do not introduce new language features as if they already exist.
- Prefer compact examples that can be checked with existing sample commands.
- Use `.test*` directories for all temporary outputs.

## Safe Defaults

- Entry point: `export func main()`.
- UI progression: `ext.message.show(...)`, then `recv()`.
- Choices: `ext.message.choices_named(...)`, then `state.get("ui.last_choice")`.
- Text input: `ext.message.prompt(...)`, then `state.get("ui.last_input")`.
- Packaging checks: follow `samples/README.md`.
