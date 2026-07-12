# WARC v2仕様

## 互換方針

- outer magicは`WARC`を維持する。
- archive version 2はmanifest magic `MNF2`を使用する。
- VM programは既存`WMP1` / bytecode version 1のまま格納する。
- v2 writerはv1を生成しない。v1 readerは`wms legacy run`専用とする。

## v2レイアウト

little endianで次の順に格納する。

1. `WARC`、archive version `u16 = 2`
2. `MNF2`
3. package、package version、entry system
4. tick_hz、seed、save compatibility version
5. capability文字列一覧
6. length-prefixed WMP1 program
7. length-prefixed schema
8. asset countと各assetのID、kind、name、payload

文字列とblobは`u32 length + bytes`とする。decoderはtrailing bytes、invalid UTF-8、未知asset kind、重複asset ID/name、範囲外lengthを拒否する。

## Schema

schemaはrecord kind、名前、persistent flag、各fieldの固定`u16` ID、名前、型を含む。field ID collisionはcompile errorとし、archiveを生成しない。

## Legacy

`detect_format`はmagicとversionだけを安全に検査する。v1は既存署名・digest・section検証を経由し、v2 runtimeへ旧`ext.*`状態を混入させない。
