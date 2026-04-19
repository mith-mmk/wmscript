# Issue log

## 2026-04-19 Writer-First recovery status

- In progress: first target を writer-first 契約固定へ再定義。
- Moved to SPEC baseline:
	- recv()/message progression 契約（language）
	- input return ABI（language）
	- save/load layering と UI ownership（hostapi）
	- worker input routing と scheduler clock 契約（scheduler）
	- platform capability matrix（language）
- Next action:
	- 上記5点を samples/easynovel と samples/messagewindow の検証手順へ接続
	- profile dependent capability（audio/network/async_io on wasm）の具体値を host 実装で確定

- `AGENT.md` references `spec.yml.md`, and that file does exist at the repository root. The split files under `SPEC/*.md` are still the more detailed source set, so any overlapping rules between `spec.yml.md` and `SPEC/*.md` should be kept in sync explicitly.
- `todo.md` recheck (2026-03-28) shows several implemented areas whose design contract is still missing or under-specified in `SPEC/*.md`. The current code works, but the following points should be fixed in the written spec before the next implementation wave.
- Missing design: platform profile to capability matrix. `native` / `egui` / `wasm` currently differ in `file_system`, `async_io`, `gui`, `network`, and `web_compat`, and the compiler now rejects `ext.*` calls when the selected profile lacks the required capability. This compile-time rejection rule and the default matrix are not written down in `SPEC/language.md` or `SPEC/hostapi.md`.
- Missing design: high-level UI contract for `ext.scene.*` and `ext.message.*`. The repository now has a concrete engine-to-UI flow, but `SPEC` still only describes generic host/resource behavior. We need one place that fixes which side owns window/layout state, what the engine worker is allowed to emit, and which UI state is renderer-owned versus script-owned.
- Missing design: frontend-to-engine input return path. Current samples and frontend use `recv()` plus state keys like `ui.last_choice`, `ui.last_input`, and `ui.last_reply`, but this protocol is not specified anywhere. We need to decide whether these keys are standard ABI, sample-only convention, or should be replaced by a typed message/event contract.
- Missing design: message window progression semantics. `text_speed`, `auto`, `skip`, backlog/log display, and read-flag behavior exist in code and samples, but `SPEC` does not define reveal timing, what auto waits for, whether skip is all-text or read-only, or whether read-state is a `state.*` convention or a first-class runtime feature.
- Missing design: surface contract for `ext.image.*` and `ext.audio.*`. `SPEC/resource.md` and `SPEC/hostapi.md` define generic resource/request flows, but not the higher-level script-visible APIs now present in code: draw queue semantics, sprite sheet metadata, playback state, stop/pause/seek persistence, and what must survive `ext.vm.save/load`.
- Missing design: checkpoint layering between `state.save/load` and `ext.vm.save/load`. `SPEC/hostapi.md` covers generic save/load restoration, but the script-facing distinction is still unclear: what is persistent game state, what is transient runtime checkpoint state, and which pending requests, UI states, audio states, and resource handles are required to restore.
- Missing design: time control API and scheduler clock contract. `todo.md` still has an open time control item (`wait`/key wait/time wait/tick/sleep). `SPEC/scheduler.md` mentions `sleep`, but there is no language- or host-level definition for frame ticks, input wait, wall-clock versus simulation time, or how `auto` progression should consume the same clock.
- Missing design: wasm/WebGL frontend contract. `todo.md` still leaves `10.3 WebGL` open, and `SPEC` does not currently fix the JS bridge boundary, asset upload ownership, or how the `gui` capability maps to browser-backed rendering versus native `egui`.
