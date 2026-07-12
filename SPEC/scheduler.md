# WMScript v2 Scheduler仕様

## 固定tick

- `wms.toml`の`game.tick_hz`は1以上の整数とする。
- runtime tickは0から開始し、入力取込、`game.tick`発行、event drain、render frame生成の順に処理する。
- headlessとeguiは同じtick処理を使用し、port adapterだけを交換する。

## task待機

- `await input.choice/text`はprompt host call後にVM `Recv`へloweringする。
- `await time.sleep`はwake予約後にVM `Sleep`へloweringする。
- `await game.next_tick`はVM `Yield`へloweringする。
- VM frameとlocal stackが継続状態を保持するため、VM opcodeやValue表現は変更しない。
- 待機中taskへ届いたmessageはworker inboxのFIFO順で再開値になる。

## 制限

- 1 VM frameのstep budgetと1 tickのevent budgetを別々に設定する。
- budget到達はpanicにせず、再scheduleまたは明示runtime errorにする。
- system、通常`func`、test funcからの待機は禁止する。
