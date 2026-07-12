# WMScript v2 Host / Port API仕様

## 境界

VMの`HostApi`、`HostRegistry`、`CALL_HOST` ABIは変更しない。v2 compilerは標準module callを固定host IDへ変換し、GameRuntimeが次のportへ配送する。

- `InputPort`: tick単位の入力event列
- `RenderPort`: 完了した`RenderFrame`
- `AudioPort`: asset名とvolumeを持つ再生command
- `StoragePort`: persistent `WorldSnapshot`のslot保存・読込

## 標準module

| module | v2の責務 |
| --- | --- |
| `core` | collection長、compiler内部のcopy-on-write更新 |
| `game` | tickとevent lifecycle |
| `world` | entity/component/resource/query/event |
| `time` | fixed tick待機 |
| `random` | project seedに基づく乱数 |
| `input` | choice/text/command入力 |
| `scene`, `ui` | 表示状態とmessage |
| `audio`, `asset` | asset handleと再生 |
| `save` | persistent snapshot |

## セキュリティ

- `wms.toml`にないcapabilityを必要とするhost callはpackage実行前に拒否する。
- project相対pathはabsolute pathと`..`を禁止し、project root外へ出してはならない。
- 新sourceから`ext.*`、文字列keyの`state.*`、任意host ID呼出しは禁止する。
- WARC v1の旧host surfaceは`wms legacy run`プロセス内だけに隔離する。
