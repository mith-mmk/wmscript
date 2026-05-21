# RPG Demo

`SPEC/gameplay.md` の gameplay profile に合わせた RPG / map mode /
event mode / battle mode の操作デモです。固定分岐のテキストアドベンチャーではなく、
64x64 の 2D マップ移動と、疑似 3D グリッド移動を確認できます。

- `engine/main.wms` は町 64x64 map、森 64x64 map、地下遺跡 grid3d map、石碑 event、スライム battle を担当します。
- `ui/main.wms` は既存ノベルゲームエンジンの message / choice UI を RPG 用に設定します。
- `loader/main.wms` は image `100..106` と SE audio `203..204` だけを preload します。
- map mode は `text.choices_named(...)` の方向 ID を使います。GUI では矢印キー/WASD が同じ choice id に変換され、方向だけの移動画面では選択パネルを閉じます。
- BGM は同梱しません。壊れた deterministic loop は削除済みで、実 BGM は DAW 書き出し音源を外部指定して package してください。

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
| `event.current` | `forest_stone` |
| `actor.hero.hp` | `30` |
| `inventory.potion` | `1` |
| `resource.gold` | `12` |
| `job.herb_garden.enabled` | `true` after reading the stone event |

## Resource Map

| Resource | File | Purpose |
| --- | --- | --- |
| `100` | `assets/town-map.png` | 64x64 town tile map |
| `101` | `assets/forest-map.png` | 64x64 forest tile map |
| `102` | `assets/dungeon-map.png` | 64x64 dungeon minimap |
| `103` | `assets/battle-slime.png` | Slime battle background |
| `104` | `assets/actor-icons.png` | Hero icon sheet |
| `105` | `assets/landmark-icons.png` | Gate / stone / slime / stairs markers |
| `106` | `assets/dungeon-view.png` | Pseudo-3D dungeon view |
| `203` | `assets/stone-chime.wav` | Stone event SE |
| `204` | `assets/battle-hit.wav` | Battle hit SE |

Audio resource `200..202` are reserved for real DAW BGM exports, but the default
demo does not package or play them.

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
  --image rpg/town-map@100=samples/rpgdemo/assets/town-map.png `
  --image rpg/forest-map@101=samples/rpgdemo/assets/forest-map.png `
  --image rpg/dungeon-map@102=samples/rpgdemo/assets/dungeon-map.png `
  --image rpg/battle-slime@103=samples/rpgdemo/assets/battle-slime.png `
  --image rpg/actor-icons@104=samples/rpgdemo/assets/actor-icons.png `
  --image rpg/landmark-icons@105=samples/rpgdemo/assets/landmark-icons.png `
  --image rpg/dungeon-view@106=samples/rpgdemo/assets/dungeon-view.png `
  --audio rpg/stone-chime@203=samples/rpgdemo/assets/stone-chime.wav `
  --audio rpg/battle-hit@204=samples/rpgdemo/assets/battle-hit.wav `
  --out .test-rpgdemo/rpgdemo.warc

cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,east,forest,west,town,end_demo --expect rpg-map-switch
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,east,forest,north,north,check,read,end_demo --expect rpg-stone-read --expect-audio-resource 203
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,east,forest,east,east,attack,attack,attack --expect rpg-victory --expect-audio-resource 204
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices east,east,forest,south,south,forward,turn_right,forward,check,end_demo --expect rpg-dungeon-depth
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices status --expect rpg-status
cargo run -p wmfrontend --bin wmautoui -- .test-rpgdemo/rpgdemo.warc --platform egui --choices inventory --expect rpg-inventory
```

## DAW BGM Slots

Use real DAW exports when BGM is needed. The engine does not auto-play BGM by
default; add explicit script calls and package the files intentionally.

Recommended reserved names:

- `rpg/town-loop` / resource `200`: town BGM.
- `rpg/forest-loop` / resource `201`: forest BGM.
- `rpg/dungeon-loop` / resource `202`: dungeon BGM.

Recommended formats: `wav`, `mp3`, `ogg`, `aac`, or `m4a`. MIDI and DAW project
files are outside v1 packaging.

Example packaging flags:

```powershell
--audio rpg/town-loop@200=path/to/town-loop.wav `
--audio rpg/forest-loop@201=path/to/forest-loop.ogg `
--audio rpg/dungeon-loop@202=path/to/dungeon-loop.m4a
```

## Asset Generation Notes

The checked-in image assets are deterministic placeholder game art generated
locally for this sample. They are intentionally simple and replaceable:

- `town-map.png`: 2048x2048 64x64 tile town map.
- `forest-map.png`: 2048x2048 64x64 tile forest map.
- `dungeon-map.png`: 2048x2048 64x64 dungeon minimap.
- `battle-slime.png`: 1280x720 battle background.
- `dungeon-view.png`: 1280x720 pseudo-3D dungeon view.
- `actor-icons.png` / `landmark-icons.png`: 64px icon sheets.

The bundled SE are deterministic short WAV effects only:

- `stone-chime.wav`: short chime for event mode.
- `battle-hit.wav`: short hit sound for battle mode.
