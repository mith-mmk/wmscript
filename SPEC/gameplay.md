# Gameplay Extension Profile

## Responsibility

This document defines the script-side gameplay state conventions used by the
first RPG, simulation, automation, and RTS samples. It does not add VM opcodes.

## Dependency

- Script progression uses the Writer-First message contract in
  [language.md](language.md).
- Rendering and input use the existing host APIs in [hostapi.md](hostapi.md).
- Save/load layering follows the state and VM checkpoint split in
  [hostapi.md](hostapi.md).

## v1 State Keys

Gameplay v1 uses stable string keys in `state.*` so existing frontends can run
gameplay samples without a dedicated `ext.rpg.*` or `ext.sim.*` namespace.
Automation and RTS helper extensions are thin state mutators over these same
keys; they do not own separate runtime state.

| Key | Meaning |
| --- | --- |
| `game.mode` | Current gameplay mode. v1 values are `map`, `field`, `menu`, `event`, `battle`, `automation`, and `rts`. `field` remains a compatibility value for older samples. |
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
| `automation.jobs` | Pipe-separated stable job id list maintained by `ext.automation.set_job`. |
| `rts.units` | Pipe-separated stable unit id list maintained by `ext.rts.set_unit`. |
| `unit.<id>.team` | Stable RTS team/faction id. |
| `unit.<id>.x` | Unit x coordinate on the current map. |
| `unit.<id>.y` | Unit y coordinate on the current map. |
| `unit.<id>.target_x` | Last ordered target x coordinate. |
| `unit.<id>.target_y` | Last ordered target y coordinate. |
| `unit.<id>.hp` | Unit hit points. |
| `unit.<id>.last_order` | Last RTS order id, for example `spawn` or `move`. |

## Map Mode

Map mode is the standard v1 surface for RPG field movement, 2D simulation
boards, and automation-game work areas. It still uses `state.*` and normal
message replies; no `ext.rpg.*` or `ext.sim.*` namespace is introduced.
`ext.automation.*` and `ext.rts.*` remain state-key helpers rather than a
separate gameplay state model.

| Key | Meaning |
| --- | --- |
| `map.current` | Stable map id such as `town`, `forest`, or `dungeon`. |
| `map.projection` | Map projection id. v1 sample values are `tile2d` and `grid3d`. |
| `map.width` | Width of the active map grid. The RPG demo uses 64 for explorable maps. |
| `map.height` | Height of the active map grid. The RPG demo uses 64 for explorable maps. |
| `map.depth` | Depth or floor-span of a grid-style 3D map. |
| `map.x` | Horizontal map coordinate. |
| `map.y` | Vertical 2D map coordinate. |
| `map.z` | Depth coordinate used by grid-style 3D maps. |
| `map.facing` | Facing direction such as `north`, `east`, `south`, or `west`. |
| `map.last_move` | Last accepted movement/action id. |
| `map.last_blocked` | `true` when the last movement could not enter the target cell. |
| `map.transition` | Last map transition id, for example `town_to_forest`. |
| `map.return_mode` | Mode to restore after map-triggered event or battle sequences. |
| `dungeon.level` | Optional dungeon floor/depth marker for grid-style RPG maps. |

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

## Automation and RTS Extensions

`ext.automation.*` and `ext.rts.*` are convenience host APIs for deterministic
automation and RTS samples. They must update only the public `state.*` keys
listed above, so save/load compatibility remains the same as scripts that call
`state.set(...)` directly.

| API | Meaning |
| --- | --- |
| `ext.automation.resource(name)` | Reads `resource.<name>` unless `name` already starts with `resource.` or `inventory.`. |
| `ext.automation.set_resource(name, amount)` | Sets the resource counter. |
| `ext.automation.add_resource(name, delta)` | Adds to the resource counter and returns the new amount. |
| `ext.automation.set_job(id, enabled, rate, output)` | Registers a deterministic production job. |
| `ext.automation.enable_job(id, enabled)` | Enables or disables an existing job. |
| `ext.automation.tick(steps)` | Advances `game.tick` and applies enabled job production. |
| `ext.automation.job_progress(id)` | Reads accumulated job progress. |
| `ext.rts.set_unit(id, team, x, y, hp)` | Registers or replaces a unit and appends it to `rts.units`. |
| `ext.rts.move_unit(id, x, y)` | Moves a unit and records a `move` order. |
| `ext.rts.unit_x(id)` / `ext.rts.unit_y(id)` / `ext.rts.unit_hp(id)` | Read unit state. |
| `ext.rts.damage_unit(id, amount)` | Reduces unit HP to a minimum of zero and returns the new HP. |

The first implementation intentionally models production as integer output per
tick step. More advanced timing, pathfinding, fog of war, or combat resolution
should be layered above these calls or added as separate deterministic helpers
after the state keys are fixed.

## v1 Sample Scope

The first sample must stay deterministic so it can run under `wmautoui`:

- RPG map, menu, event, and battle modes are covered.
- Simulation, automation, and RTS games are represented by `game.tick`, `job.*`,
  `resource.*`, and `unit.*` state keys, with helper extensions available for
  common deterministic mutations.
- The sample stores persistent game progress with `state.*` and may checkpoint
  runtime state with `ext.vm.save`.
