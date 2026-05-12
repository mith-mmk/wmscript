## 責務
- ResourceId / handle / request の分離を定義する。
- アーカイブ由来の資産を VM に安全に渡す手続きを定義する。

## 依存
- アセット供給元は [SPEC/archive.md](archive.md) に依存する。
- host API の待機・完了通知は [SPEC/hostapi.md](hostapi.md) に依存する。

1. リソース管理の役割

リソースは単なるファイルではなく、VMから見るとこうなります：

リソース = IDで参照される外部データ + ライフサイクル + ストリーミング制御

役割は4つ：

アーカイブからのロード

メモリ管理（キャッシュ含む）

ハンドル管理（VMに渡す）

非同期処理（IO/デコード）

2. 基本モデル
2.1 リソースの種類
image
audio
binary
font
video（将来）
script外データ
2.2 VMからの見え方

VMからはこう見せます：

handle (opaque)

例：

img = load_image("bg_001")
draw(img)

内部では：

handle -> ResourceEntry -> actual data
3. ID設計（重要）
3.1 リソースID

アーカイブ内では：

type ResourceId = u32;
解決方法
(resource_id) → section_id + offset
3.2 名前は使わない
"bg_001.png"

はビルド時だけ。

実行時は：

resource_id = 42
3.3 manifestとの関係

manifestに持たせる：

resource_map {
  name_hash → resource_id
}

※VMは name を知らない
※ツールだけが使う

4. アーカイブとの統合
4.1 asset section
SectionKind::Asset

中身：

[ResourceHeader][data...]
4.2 ResourceHeader
#[repr(C)]
struct ResourceHeader {
    resource_id: u32,
    resource_type: u16,
    flags: u16,

    compression: u16,
    encoding: u16,

    unpacked_size: u32,
    packed_size: u32,

    data_offset: u32,
}
4.3 flags
STREAMING
IMMUTABLE
GPU_UPLOAD_REQUIRED
AUDIO_STREAM
5. ライフサイクル
5.1 状態
UNLOADED
LOADING
READY
FAILED
UNLOADING
5.2 ResourceEntry
struct ResourceEntry {
    id: ResourceId,
    state: ResourceState,

    ref_count: u32,

    data: Option<ResourceData>,
    last_access_frame: u64,

    flags: u32,
}
5.3 ResourceData
enum ResourceData {
    Image(ImageHandle),
    Audio(AudioHandle),
    Binary(Vec<u8>),
}
6. ハンドル設計（超重要）
6.1 VMに渡すもの
#[repr(transparent)]
struct Handle(u64);
6.2 中身
[ generation | index ]

例：

upper 32bit: generation
lower 32bit: index
6.3 テーブル
struct HandleTable {
    entries: Vec<HandleEntry>,
}
struct HandleEntry {
    resource_id: ResourceId,
    generation: u32,
}
6.4 なぜ必要か

これで防げる：

解放後アクセス（UAF）

別リソース誤参照

VMの不正アクセス

7. ロードモデル
7.1 同期禁止

VMからのロードは基本：

非同期
7.2 API
handle = load_resource(resource_id)

内部：

if cached → 即返す
else → LOADING + future登録
7.3 完了確認
is_ready(handle)

または

await_resource(handle)  // VMなら yield
8. ストリーミング
8.1 大容量対応

音声・動画・巨大画像：

partial load

8.2 ストリーム構造
```
struct StreamState {
    cursor: u64,
    buffer: Vec<u8>,
}
```
8.3 読み出し
```
read_chunk(handle, size)
```
9. メモリ管理
9.1 キャッシュ
LRU or LFU
9.2 eviction
if memory > limit:
    evict unused resources
9.3 優先度
UI > current scene > background > preload
10. GPU連携
10.1 フロー
load → decode → upload → GPU handle
10.2 状態
CPU_READY
GPU_READY
10.3 遅延アップロード

必要になるまで GPU に送らない

11. マルチワーカー対応
11.1 共有 or 分離

推奨：

ResourceManager = グローバル共有
VM = handleだけ持つ
11.2 共有ルール

データは共有

handle は worker毎に安全に

12. セキュリティ
12.1 アクセス制限

resource にも capability を持たせる：

frontend → image/audio OK
backend → NG
12.2 検証

resource_id 範囲チェック

type一致チェック

handle generationチェック

13. 圧縮 / エンコード

13.1 圧縮

LZ4 / Zstd

13.2 エンコード
image → PNG / raw / GPU format
audio → DAW exported final files (`wav`, `mp3`, `ogg`, `aac`, `m4a`)

v1 では音声を変換しない。`wmtoolchain` / `wmfrontend` は `--audio NAME=PATH`
または project config の `audio` セクションで指定されたファイルをそのまま
`ResourceType::Audio` として archive に格納する。MIDI と DAW project file は
resource 仕様の対象外で、OGG/AAC/M4A の実再生は host frontend/backend の codec
対応に依存する。

13.3 デコード層
archive → raw → decoded → runtime
14. ホットリロード
14.1 差し替え
resource_id 同一 → 新データ
14.2 generation更新
handle invalid化
15. セーブとの関係

保存するのは：

resource_id

保存しない：

handle
16. Rust構造（完成形）
```
struct ResourceManager {
    entries: Vec<ResourceEntry>,
    handle_table: HandleTable,

    memory_used: usize,
    memory_limit: usize,
}

struct ResourceEntry {
    id: ResourceId,
    state: ResourceState,
    ref_count: u32,

    data: Option<ResourceData>,
    flags: u32,

    last_access_frame: u64,
}

enum ResourceState {
    Unloaded,
    Loading,
    Ready,
    Failed,
}
```

1.  VMとの接続

```
VM命令：

CALL_HOST load_resource
CALL_HOST draw_image
CALL_HOST play_audio
```

VMは：

handleを受け取るだけ

中身は一切知らない

18. 最終設計の核

この設計の一番重要な点：

① IDベース

→ 名前を排除

② handle分離

→ VM安全

③ 非同期前提

→ フレーム落ち防止

④ キャッシュ前提

→ IO最小化

⑤ アーカイブ統合

→ 署名・検証と一体

19. よくある失敗（回避ポイント）

文字列でリソース管理 → 即ボトルネック

同期ロード → フリーズ

handleなし → バグ地獄

VMが直接リソース触る → 崩壊

キャッシュなし → IO地獄

20. 完成形まとめ

ResourceId = u32

Handle = u64（generation付き）

ResourceManager = グローバル

非同期ロード

LRUキャッシュ

GPU遅延アップロード

アーカイブ asset section から供給

capabilityで制御

VMは handleのみ扱う

ここまで揃うと、残りはかなり核心です。
次に詰めるとシステムが完成するのは：

ホストAPI仕様（VM ↔ ResourceManager）

スケジューラと非同期IOの統合モデル

セーブデータとの完全整合設計

この3つが揃うと、もうエンジンとして動く段階になります。
