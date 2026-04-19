# How To Use（使い方）

このページは導線だけを持つ軽量版です。実行コマンドの正本は `samples/README.md` です。

## まず読む場所

1. 全体入口: [README.md](README.md)
2. サンプル実行カタログ: [samples/README.md](samples/README.md)
3. toolchain CLI: [crates/wmtoolchain/README.md](crates/wmtoolchain/README.md)
4. 言語/API一覧: [function.ja.md](function.ja.md)

## すぐ試すコマンド

```bash
# script を直接実行
cargo run -p wmfrontend -- samples/messagewindow/main.wms --platform egui --font noto

# package を作る
cargo run -p wmtoolchain -- samples/helloworld/main.wms --out releases/helloworld-cycle.warc

# package を実行
cargo run -p wmfrontend -- releases/helloworld-cycle.warc --platform native
```





