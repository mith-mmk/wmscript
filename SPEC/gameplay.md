# WMScript v2 Gameplay / World仕様

## World

- Entity IDは`u64`の単調増加値で、削除後も再利用しない。
- component storeとresource storeはschema名で識別する。
- query結果は常にEntity ID昇順。同じtick内で追加されたentityも次のqueryからこの順序に従う。
- `persistent` component/resourceだけをsnapshotへ含める。handle、`any`、transient参照はpersistent schemaで禁止する。

## EventとSystem

- eventには単調増加するsequenceと発行tickを付与し、FIFOで処理する。
- systemは名前の辞書順で実行する。同名systemは登録エラーとする。
- systemはevent処理中にeventを追加できる。追加eventは現在のFIFO末尾へ入る。
- 1 tickのevent上限を超えた場合はruntime errorとして停止し、無限event loopを防ぐ。
- systemは`await`できない。入力や時間待ちはtask/on handlerが担当する。

## 決定性

- fixed tick、Entity ID順query、FIFO event、system名順、明示seedの乱数を決定性の契約とする。
- wall clock、OS乱数、unordered collection iterationをgame logicから直接参照してはならない。
- 同一package、seed、初期snapshot、tick数、入力event列は同一World snapshotを生成しなければならない。

## ジャンル対応

- ノベル: `task`、`await input.*`、scene/ui/saveを使用する。
- RPG: Position/Health等のcomponent、command event、同期systemを使用する。
- RTS: unit component、resource、fixed-tick production systemを使用する。
- シミュレーション: agent component、calendar resource、seed乱数、replay可能なeventを使用する。
