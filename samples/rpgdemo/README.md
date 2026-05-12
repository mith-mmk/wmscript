# RPG Demo

`SPEC/gameplay.md` の gameplay v1 profile に合わせた、RPG / map mode /
event mode / battle mode / automation-state のサンプルです。

- `engine/main.wms` は town 64x64 2D map、forest 64x64 2D map、dungeon 疑似3D grid map、
  event、battle の進行を担当します。
- `ui/main.wms` は既存ノベルゲームエンジンの message / choice UI を RPG 用に設定します。
- `loader/main.wms` は背景、アイコン、map 用 resource を preload します。
- map mode は `text.choices_named(...)` の方向IDを使います。GUIでは矢印キー/WASDが同じ choice id に変換され、方向だけの移動時は選択パネルを閉じます。
- audio は DAW 書き出しを想定した `Audio` resource です。このサンプルでは deterministic WAV を同梱し、BGM/SE 差し替えの基準にしています。
- event mode は `text.show(...)`、`text.choices_named(...)`、`recv()`、
  `state.get("ui.last_choice")` を使い、既存ノベルゲームUIをそのまま再利用します。

## Gameplay Keys

| Key | Example |
| --- | --- |
| `game.mode` | `map`, `menu`, `event`, `battle` |
| `game.location` | `town`, `forest`, `dungeon` |
| `map.current` | `town`, `forest`, `dungeon` |
| `map.projection` | `tile2d`, `grid3d` |
| `map.width` / `map.height` / `map.depth` | `64`, `64`, `64` |
| `map.x` / `map.y` / `map.z` | `0..63` |
| `map.facing` | `north`, `east` |
| `dungeon.level` | `1`, `2` |
| `event.current` | `forest_stone` |
| `actor.hero.hp` | `30` |
| `inventory.potion` | `1` |
| `resource.gold` | `12` |
| `job.herb_garden.enabled` | `true` after reading the stone event |
| Audio resource | `200..204` for town/forest/dungeon BGM and event/battle SE |

## Run

```powershell
cargo run -p wmfrontend --bin wmfrontend -- samples/rpgdemo
```

## Auto UI Smoke

```powershell
New-Item -ItemType Directory -Force .test-rpgdemo

cargo run -p wmtoolchain --bin wmtoolchain -- samples/rpgdemo/engine/main.wms `
  --package rpgdemo `
  --platform egui `
  --ui samples/rpgdemo/ui/main.wms `
  --loader samples/rpgdemo/loader/main.wms `
  --image rpg/town=samples/rpgdemo/assets/town.png `
  --image rpg/forest=samples/rpgdemo/assets/forest.png `
  --image rpg/stone-event=samples/rpgdemo/assets/stone-event.png `
  --image rpg/battle-slime=samples/rpgdemo/assets/battle-slime.png `
  --image rpg/icons=samples/rpgdemo/assets/rpg-icons.png `
  --image rpg/map-icons=samples/rpgdemo/assets/map-icons.png `
  --image rpg/ruins-3d=samples/rpgdemo/assets/ruins-3d.png `
  --audio rpg/town-loop=samples/rpgdemo/assets/town-loop.wav `
  --audio rpg/forest-loop=samples/rpgdemo/assets/forest-loop.wav `
  --audio rpg/dungeon-loop=samples/rpgdemo/assets/dungeon-loop.wav `
  --audio rpg/stone-chime=samples/rpgdemo/assets/stone-chime.wav `
  --audio rpg/battle-hit=samples/rpgdemo/assets/battle-hit.wav `
  --out .test-rpgdemo/rpgdemo.warc

cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,forest,west,town,end_demo --expect rpg-map-switch
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,forest,north,check,read,end_demo --expect rpg-stone-read
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,forest,east,attack,attack,attack --expect rpg-victory --expect-audio-resource 204
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,forest,south,forward,turn_right,forward,check,end_demo --expect rpg-dungeon-depth --expect-audio-resource 202
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices status --expect rpg-status
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices inventory --expect rpg-inventory
```

## Generated Image Prompts

The original background and item assets were generated with the built-in image
generation tool using this prompt:

```text
Use case: illustration-story
Asset type: browser visual novel / RPG demo asset pack
Primary request: Create a clean stylized 2D JRPG demo asset sheet with five separate labeled panels arranged in a simple grid: town background, forest background, ancient stone event background, slime battle background, and a small icon strip with potion, gold coin, sword, shield. No text inside the artwork except tiny non-readable panel separation is allowed; do not include labels or watermarks.
Scene/backdrop: cozy fantasy RPG world suitable for a Japanese WMScript sample.
Style/medium: polished 2D game illustration, painterly but readable, bright colors, no photorealism.
Composition/framing: four landscape scene thumbnails plus one horizontal icon strip, each clearly separated with clean margins so they can be cropped later.
Lighting/mood: adventurous, friendly, not dark or horror.
Constraints: no logos, no text, no watermark, no UI overlay, no characters in foreground, keep scenes reusable as backgrounds.
```

The map icon sheet and pseudo-3D ruins background were generated with the
built-in image generation tool using this prompt, then cropped into
`map-icons.png` and `ruins-3d.png`:

```text
Use case: illustration-story
Asset type: browser JRPG demo map asset pack for WMScript
Primary request: Create two separate game assets in one clean 2D JRPG style image: (1) a 256x64 horizontal icon sheet with four 64x64 cells for a small hero marker, a town gate marker, an ancient stone marker, and a slime marker; (2) a 1280x720 pseudo-3D first-person ancient ruins corridor background suitable for grid movement. Put the icon sheet and corridor as clearly separated panels with enough clean margins so they can be cropped into separate PNG files later.
Scene/backdrop: friendly fantasy RPG world, town/forest/ruins exploration.
Style/medium: polished 2D game illustration, painterly but readable, bright adventure colors, no photorealism.
Composition/framing: icon sheet must be a straight horizontal strip with four equal square cells; ruins corridor must be wide landscape first-person view with visible stone walls and path.
Lighting/mood: adventurous and clear, not dark horror.
Constraints: no text, no labels, no watermark, no UI overlay, no foreground character, reusable as game background and icons.
```

## Generated Audio Strategy

The bundled audio files are deterministic WAV placeholders generated from simple
tone clusters so the sample does not require a DAW or external codec during
tests. They are intentionally named as final DAW exports and can be replaced by
real `wav`, `mp3`, `ogg`, `aac`, or `m4a` files without script changes:

- `town-loop.wav` / resource 200: bright town loop.
- `forest-loop.wav` / resource 201: soft forest loop.
- `dungeon-loop.wav` / resource 202: low dungeon loop.
- `stone-chime.wav` / resource 203: short event chime.
- `battle-hit.wav` / resource 204: short battle hit SE.

Recommended DAW export prompt/direction:

```text
Create lightweight JRPG demo audio exports: a warm town loop, a calm forest
loop, a low ancient-dungeon loop, a clear magic stone chime, and a short soft
battle hit. Keep loops simple, non-vocal, and suitable for repeated playback.
Export final game-ready files; do not include MIDI or DAW project files.
```
