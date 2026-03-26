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
- `assetload/`
  - アーカイブとリソース読み込みを確認するサンプル。
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
ランタイム例はコマンドライン引数で実行章を切り替えます。

### ソース

```wml
export let protagonist = "Aki";
export let setting = "last train platform";

export func prologue() {
    return "Aki arrives at the last train platform.";
}

export func chapter_1() {
    return "A lantern lights the stairs down to the station.";
}

export func chapter_2() {
    return "Aki chooses the quiet route home.";
}

export func main() {
    return "Prologue";
}
```

### 補足

- 各関数本体が単純な `return` 式なので、現在のコンパイラで直接出力できます。
- 将来的に分岐を追加しやすい形で書いてあります。

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
```

## Toolchain

`wmltoolchain` は WML スクリプトをパッケージ済みアーカイブに変換し、
必要に応じてアセットも同梱できます。

### コマンドライン

```bash
wmltoolchain <script.wml> [--package NAME] [--out FILE] [--step-limit N] [--platform native|wasm|egui] [--release] [--asset NAME=PATH]
```

### 動作

- `script.wml` は入力元のスクリプトファイルです。
- `--package NAME` はパッケージ名を上書きします。未指定時はスクリプトの
  ファイル名から推定されます。
- `--out FILE` は生成アーカイブの出力先を指定します。未指定時は
  `<script>.warc` が使われます。
- `--step-limit N` は toolchain 設定に渡す VM のステップ上限です。
- `--platform native|wasm|egui` はプラットフォームプロファイルを選びます。
- `--release` は toolchain 設定の release モードを有効にします。
- `--asset NAME=PATH` はパッケージ化するアセットを追加します。複数回指定できます。

### 実行例

```bash
cargo run -p wmltoolchain -- samples/helloworld/main.wml
cargo run -p wmltoolchain -- samples/helloworld/main.wml --out samples/helloworld/main.warc
cargo run -p wmltoolchain -- samples/easynovel/main.wml --package easynovel --platform native --step-limit 256
cargo run -p wmltoolchain -- samples/easynovel/main.wml --package easynovel --out build/easynovel.warc --asset ui/title=assets/title.bin
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

- コンパイラは現在、WMLScript の一部だけに対応しています。
- `samples/` の各例は意図的に小さく、ランタイム例と対応しています。
- 新しいサンプルを追加するときは、README と対応するランタイム例を同期してください。
