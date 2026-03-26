# How To Use（使い方）

このドキュメントは、`samples/` の内容と、このワークスペースで使用されているランタイム例をまとめたものです。

---

## サンプル一覧

- `helloworld/`
  - 最小構成のスクリプトサンプル。算術式を計算して返します。
- `inputlink/`
  - ホストから入力を受け取り、そのまま返すサンプル。
- `workercomm/`
  - 2つのワーカー間でメッセージ通信を行うサンプル。
- `assetload/`
  - アーカイブとリソースロードのサンプル。
- `easynovel/`
  - 章構成とナレーションを持つ簡易ノベルサンプル。

---

## Hello World サンプル

このサンプルは非常に小さく、以下を確認する目的で作られています：

- コンパイラのフロントエンド
- 定数畳み込み（constant folding）
- VMランタイム

##  ソース

```wml
export func main() {
    return 1 + 2 * 3;
}
```
実行結果
オプティマイザにより 1 + 2 * 3 は 7 に畳み込まれる
ランタイムは main の戻り値として 7 を返す
Input Link サンプル

ホストから値を受け取る基本例です。

ソース
```
export func main() {
    return input();
}
```

## 実行挙動
ホスト側が1つの文字列入力を提供
スクリプトはその値をそのまま返す
Worker Communication サンプル

- ワーカー間通信の例です。

ソース
```
worker sender {
    send 2, "hello worker";
}
```

```
worker receiver {
    return recv();
}
```
実行挙動
Worker1 が Worker2 にメッセージ送信
Worker2 が受信してそのまま返す
Asset Load サンプル

アーカイブに含まれるリソースをロードする例です。

ソース
```
export func main() {
    return load_asset(100);
}
```

## 実行挙動
アーカイブに1つのリソースが含まれている
ランタイムがアーカイブをロード
IDでリソースを解決
バイト列が ResourceManager 経由で利用可能になる

Easy Novel サンプル
- シンプルなノベル形式のサンプルです。

ソース
```
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

注意点
現在のコンパイラは return のみの関数をそのまま出力可能
将来的な分岐追加を想定したシンプル構造
実行例
```
cargo run -p wmlruntime --example easynovel
cargo run -p wmlruntime --example easynovel -- chapter_1
cargo run -p wmlruntime --example easynovel -- chapter_2
```

## コンパイルと実行

ランタイム例は現在のワークスペースを試す標準手段です。

```
cargo run -p wmlruntime --example hello_runtime
cargo run -p wmlruntime --example input_link
cargo run -p wmlruntime --example worker_comm
cargo run -p wmlruntime --example asset_load
cargo run -p wmlruntime --example easynovel
```

## ホスト関数の例

CALL_HOST はランタイム側で登録された関数を呼び出します。

```
runtime.register_host_function(wmlhost::HostFunction::new(1, 1, 1, 0), |args| {
    Ok(args.first().cloned().unwrap_or(wmlvm::Value::Nil))
});
```

## アーカイブとリソースの例
```
Runtime::load_archive
```

→ アーカイブをランタイムにロード

```
ResourceManager
```

→ リソース状態とハンドルを管理
署名付きアーカイブ
→ `wmlarchive::Archive::verify_signature` で検証可能

## 制約
コンパイラは現在、WMLScriptの一部のみ対応
samples/ はランタイム例と一致する最小構成
新規サンプル追加時：
README とランタイム例を必ず同期する
