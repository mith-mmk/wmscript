# Gameplay Extension Profile

## Responsibility

This document defines the script-side gameplay state conventions used by the
first RPG, simulation, and automation samples. It does not add VM opcodes or new
host extensions.

## Dependency

- Script progression uses the Writer-First message contract in
  [language.md](language.md).
- Rendering and input use the existing host APIs in [hostapi.md](hostapi.md).
- Save/load layering follows the state and VM checkpoint split in
  [hostapi.md](hostapi.md).

## v1 State Keys

Gameplay v1 uses stable string keys in `state.*` so existing frontends can run
gameplay samples without a dedicated `ext.rpg.*`, `ext.sim.*`, or
`ext.automation.*` namespace.

| Key | Meaning |
| --- | --- |
| `game.mode` | Current gameplay mode. v1 values are `map`, `field`, `menu`, `event`, and `battle`. `field` remains a compatibility value for older samples. |
| `game.turn` | Turn counter for menu, field, or battle progression. |
| `game.tick` | Simulation/automation tick counter reserved for worker-driven games. |
| `game.location` | Stable location id such as `town` or `forest`. |
| `game.last_action` | Last accepted player action id. |
| `event.current` | Current event id when `game.mode == "event"`. |
| `event.state` | Event-local state such as `entered`, `read`, or `finished`. |
| `event.return_mode` | Mode to restore after event mode exits. |
| `event.result` | Stable event outcome id. |
| `actor.<id>.hp` | Actor hit points. |
| `actor.<id>.max_hp` | Actor maximum hit points. |
| `actor.<id>.atk` | Actor attack value. |
| `actor.<id>.def` | Actor defense value. |
| `inventory.<id>` | Inventory count for stackable items. |
| `resource.<id>` | Numeric resource count such as gold or wood. |
| `job.<id>.enabled` | Automation job enabled flag. |
| `job.<id>.rate` | Automation job production rate. |
| `job.<id>.progress` | Automation job accumulated progress. |
| `job.<id>.output` | Automation job output resource id. |

## Map Mode

Map mode is the standard v1 surface for RPG field movement, 2D simulation
boards, and automation-game work areas. It still uses `state.*` and normal
message replies; no `ext.rpg.*`, `ext.sim.*`, or `ext.automation.*` namespace is
introduced.

| Key | Meaning |
| --- | --- |
| `map.current` | Stable map id such as `town`, `forest`, or `ruins`. |
| `map.projection` | Map projection id. v1 sample values are `tile2d` and `grid3d`. |
| `map.x` | Horizontal map coordinate. |
| `map.y` | Vertical 2D map coordinate. |
| `map.z` | Depth coordinate used by grid-style 3D maps. |
| `map.facing` | Facing direction such as `north`, `east`, `south`, or `west`. |
| `map.last_move` | Last accepted movement/action id. |
| `map.last_blocked` | `true` when the last movement could not enter the target cell. |
| `map.transition` | Last map transition id, for example `town_to_forest`. |
| `map.return_mode` | Mode to restore after map-triggered event or battle sequences. |

2D map choices use stable ids such as `north`, `south`, `east`, `west`,
`check`, `menu`, `status`, and `inventory`. Grid-style 3D choices use `forward`,
`back`, `turn_left`, `turn_right`, and `check`. Frontends may bind keys or
gestures to these same choice ids, but scripts should continue to read the
result through `state.get("ui.last_choice")` after `recv()`.

## Event Mode

Event mode intentionally reuses the novel-game engine surface:

1. Set `game.mode` to `event`.
2. Set `event.current`, `event.state`, and `event.return_mode`.
3. Use `text.show(...)`, `text.choices_named(...)`, and `recv()` to progress.
4. Read `state.get("ui.last_choice")` after `recv()`.
5. Set `event.result` and restore `game.mode` to `event.return_mode`, typically
   `map` for RPG map events.

The RPG, simulation, or automation layer owns why an event starts and what state
changes after it ends. The frontend continues to own rendering details,
message-window timing, input devices, and backlog display.

## v1 Sample Scope

The first sample must stay deterministic so it can run under `wmautoui`:

- RPG map, menu, event, and battle modes are covered.
- Simulation and automation games are represented by `game.tick` and `job.*`
  reserved keys only.
- The sample stores persistent game progress with `state.*` and may checkpoint
  runtime state with `ext.vm.save`.
