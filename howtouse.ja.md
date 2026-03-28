# How To Use（使い方）

このドキュメントは、`samples/` の内容と、このワークスペースで使っている
ランタイム例・toolchain例をまとめたものです。

## サンプル一覧

- `helloworld/`
  - 算術式を定数畳み込みした結果を返す最小サンプル。
- `inputlink/`
  - ホスト入力をランタイム経由で受け取り、そのまま返すサンプル。
- `workercomm/`
  - 2つのワーカー間でメッセージ送受信を行うサンプル。
- `engineworker/`
  - エンジン主導のメッセージウィンドウサンプル。ページ送り、名前付き選択肢、追加入力まで含みます。
- `messagewindow/`
  - ページ送りと選択肢制御に絞ったメッセージウィンドウ専用サンプルです。
- `assetload/`
  - アーカイブとリソース読み込みを確認するサンプル。
- `imageaudio/`
  - 画像描画と音声再生を組み合わせたデモ。
- `uiimage/`
  - `samples/uiimage.png` を再現する scene layout デモ。
- `easynovel/`
  - 章構成とナレーションを持つ簡易ノベルサンプル。

## Hello World サンプル

このサンプルは非常に小さく、以下を確認する目的で作られています。

- コンパイラのフロントエンド
- 定数畳み込み
- VM ランタイム

### ソース

```wml
export func main() {
    return 1 + 2 * 3;
}
```

### 期待結果

- オプティマイザが式を `7` に畳み込む
- ランタイムが `main` の戻り値として `7` を返す

## Input Link サンプル

ホストから値を受け取る基本例です。

### ソース

```wml
export func main() {
    return input();
}
```

### 実行挙動

- ホスト側が1つの文字列入力を提供する
- スクリプトはその値をそのまま返す

## Worker Communication サンプル

ワーカー間通信の例です。

### ソース

```wml
worker sender {
    send 2, "hello worker";
}

worker receiver {
    return recv();
}
```

### 実行挙動

- Worker 1 が Worker 2 にメッセージを送信する
- Worker 2 が受信してそのまま返す

## Asset Load サンプル

アーカイブに含まれるリソースをロードする例です。

### ソース

```wml
export func main() {
    return load_asset(100);
}
```

### 実行挙動

- アーカイブに1つのアセットリソースが含まれる
- ランタイムがアーカイブをロードし、リソースを ID で解決する
- リソースのバイト列が ResourceManager から参照できる

## Easy Novel サンプル

小さな物語系スクリプトです。ビジュアルノベル風の構造を持ちながら、
現在のコンパイラが扱える `return` 中心のモデルに収めています。
ランタイム例はコマンドライン引数で実行章を切り替えます。frontend の
message window には返り値の章本文がそのまま表示されます。

### ソース

```wml
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Narrator: The last train platform is almost empty.\nNarrator: Aki stops under the lantern light and listens to the rails.\nAki: The city feels farther away than usual.";
}

export func chapter_1() {
    return "Narrator: A lantern lights the stairs down to the station.\nAki: The next train is still ten minutes away.\nNarrator: A quiet voice answers from the ticket gate.";
}

export func chapter_2() {
    return "Narrator: Aki chooses the quiet route home.\nAki: I'll take the river path tonight.\nNarrator: The station lights fade behind the empty road.";
}

export func main() {
    return "Narrator: Select a chapter from the runtime example.\nNarrator: prologue, chapter_1, or chapter_2.\nNarrator: The returned text will appear in the message window.";
}
```

### 補足

- 各関数本体が単純な `return` 式なので、現在のコンパイラで直接出力できます。
- 将来的に分岐を追加しやすい形で書いてあります。
- frontend は最終的な文字列返り値を message window に表示します。

### 実行例

- `cargo run -p wmlruntime --example easynovel`
- `cargo run -p wmlruntime --example easynovel -- chapter_1`
- `cargo run -p wmlruntime --example easynovel -- chapter_2`

## コンパイルと実行

ランタイム例は、現在のワークスペースを試す標準手段です。

```bash
cargo run -p wmlruntime --example hello_runtime
cargo run -p wmlruntime --example input_link
cargo run -p wmlruntime --example worker_comm
cargo run -p wmlruntime --example asset_load
cargo run -p wmlruntime --example easynovel
cargo run -p wmlfrontend -- samples/easynovel/main.wms --platform egui --font noto
cargo run -p wmlfrontend -- samples/messagewindow/main.wms --platform egui --font noto
cargo run -p wmlfrontend -- --demo uiimage --platform egui --font noto
cargo run -p wmlfrontend -- --demo image-audio --platform egui --font noto
cargo run -p wmlfrontend -- --demo engineworker --platform egui --font noto
cargo run -p wmlfrontend -- --demo messagewindow --platform egui --font noto
```

egui フロントエンドの既定フォントは、日本語表示を優先して Noto Sans 系にしています。
`--font default`、`--font noto`、`--font mono` で切り替えられます。

コンパイラは、選択した platform profile に無い capability を必要とする
`ext.*` 呼び出しを拒否します。たとえば `wasm` では `ext.fs.*` と
`ext.net.*` は使えませんが、`state.*` と `ext.vm.*` はそのまま使えます。

`wmlfrontend` には組み込みデモ起動もあります。

- `--demo uiimage` でスクリプトファイルを読まずに scene layout デモを起動します。
- `--demo image-audio` でスクリプトファイルを読まずに画像/音声デモを起動します。
- `--demo engineworker` でスクリプトファイルを読まずに worker 分離デモを起動します。
- `--demo messagewindow` でスクリプトファイルを読まずにメッセージウィンドウ専用デモを起動します。
- `--package NAME` でデモのパッケージ名を上書きできます。
- `--platform native|wasm|egui` で実行プロファイルを選べます。
- `--image NAME=PATH` と `--asset NAME=PATH` はファイル指定の通常モードで追加アセットを付けるためのオプションです。

## Toolchain

`wmltoolchain` は WML スクリプトをパッケージ済みアーカイブに変換し、
必要に応じてアセットも同梱できます。

### コマンドライン

```bash
wmltoolchain <script.wms> [--package NAME] [--out FILE] [--step-limit N] [--platform native|wasm|egui] [--release] [--asset NAME=PATH] [--image NAME=PATH]
```

### 動作

- `script.wms` は入力元のスクリプトファイルです。
- `--package NAME` はパッケージ名を上書きします。未指定時はスクリプトの
  ファイル名から推定されます。
- `--out FILE` は生成アーカイブの出力先を指定します。未指定時は
  `<script>.warc` が使われます。
- `--step-limit N` は toolchain 設定に渡す VM のステップ上限です。
- `--platform native|wasm|egui` はプラットフォームプロファイルを選びます。
- `--release` は toolchain 設定の release モードを有効にします。
- `--asset NAME=PATH` はパッケージ化するアセットを追加します。複数回指定できます。
- `--image NAME=PATH` は画像アセットを追加します。メッセージウィンドウ、立ち絵、
  背景などに使う画像をフロントエンドへ渡したいときに使います。

### 実行例

```bash
cargo run -p wmltoolchain -- samples/helloworld/main.wms
cargo run -p wmltoolchain -- samples/helloworld/main.wms --out samples/helloworld/main.warc
cargo run -p wmltoolchain -- samples/easynovel/main.wms --package easynovel --platform native --step-limit 256
cargo run -p wmltoolchain -- samples/easynovel/main.wms --package easynovel --out build/easynovel.warc --asset ui/title=assets/title.bin
cargo run -p wmltoolchain -- samples/easynovel/main.wms --package easynovel --out build/easynovel.warc --image ui/background=assets/background.png
```

## ホスト関数の例

`CALL_HOST` はランタイム側で登録したホスト関数を呼び出します。

```rust
runtime.register_host_function(wmlhost::HostFunction::new(1, 1, 1, 0), |args| {
    Ok(args.first().cloned().unwrap_or(wmlvm::Value::Nil))
});
```

## アーカイブとリソースの例

- `Runtime::load_archive` はバンドルをランタイムに読み込みます。
- `ResourceManager` はリソース状態とハンドルを公開します。
- 署名付きアーカイブは `wmlarchive::Archive::verify_signature` で検証できます。

## 制約

- コンパイラは現在、WMScript の一部だけに対応しています。
- `samples/` の各例は意図的に小さく、ランタイム例と対応しています。
- 新しいサンプルを追加するときは、README と対応するランタイム例を同期してください。
