# WMScript LLM Documentation Pack

This directory is a compact English documentation pack for LLM-assisted reading
and script generation.

WMScript is still a draft language. The syntax, runtime API, archive format, and
frontend behavior may change without compatibility guarantees. Treat this pack
as an LLM-friendly guide, not as the normative specification.

## Recommended Loading Order

1. [status-and-change-policy.md](status-and-change-policy.md)
2. [language.md](language.md)
3. [runtime-functions.md](runtime-functions.md)
4. [examples.md](examples.md)
5. Source-of-truth references when precision matters:
   - [../../SPEC/language.md](../../SPEC/language.md)
   - [../../SPEC/vm.md](../../SPEC/vm.md)
   - [../../SPEC/hostapi.md](../../SPEC/hostapi.md)
   - [../../functions.md](../../functions.md)
   - [../../samples/README.md](../../samples/README.md)

## How To Use This Pack

- Use this pack to understand the current intended authoring surface.
- Prefer short, explicit scripts over inferred language features.
- When a detail is missing here, check the linked `SPEC/` document or the Rust
  implementation before inventing behavior.
- Do not assume JavaScript, Lua, Python, or TypeScript syntax unless it is shown
  here or in a current sample.
- Generated examples should mention that they target the current draft surface.

## Agent Prompt Samples

This pack includes samples for tools that load repository instructions:

- [AGENTS.sample.md](AGENTS.sample.md)
- [CLAUDE.sample.md](CLAUDE.sample.md)

They are examples only. They are not active repository-level instruction files.
