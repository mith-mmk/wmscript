全体像

最終形の関係はこうです。

```
VM
 └─ CALL_HOST(...)
      └─ Host API Layer
           ├─ ResourceManager
           ├─ StateManager
           ├─ Audio/Image/UI
           └─ AsyncIO / JobSystem

Scheduler
 ├─ runnable worker を回す
 ├─ sleeping / waiting / blocked_on_io を管理
 └─ IO完了イベントで worker を再開

SaveSystem
 ├─ VM公開状態を保存
 ├─ Host state を保存
 ├─ pending async を保存可能形へ正規化
 └─ load後に再構成
 ```

2. ホストAPI仕様（VM ↔ ResourceManager）

ここでは「VMに何を見せるか」を固定します。

2.1 基本原則

VMに見せるものは最小です。

値型

handle

status code

request id

event/message

VMは

ポインタを持たない

OS資源を直接持たない

ResourceManager内部構造を知らない

2.2 呼び出し規約

既存方針どおり、VMからは CALL_HOST host_id argc です。

ホスト関数の論理シグネチャ
type HostResult = Result<HostReturn, HostError>;

enum HostReturn {
    Void,
    Value(Value),
    Yield(WaitToken),
}

ただしVMに例外は持たせないので、実際の返し方は次のどちらかです。

推奨

即時完了: Value

継続待ち: VM状態を blocked にして nil か request handle を返す

失敗: nil または { ok=false, code=... }

2.3 Host APIカテゴリ

大きく分けるとこうです。

Resource

Render/UI

Audio

State

Time

Worker/Message

Save/Load

Debug/Telemetry

今回は ResourceManager 連携が主題なので、そこを中心にします。

3. Resource API の完成形
3.1 VM公開型
type ResourceId = u32;
type Handle = u64;
type RequestId = u64;

VM上では handle と request_id は int 扱いでもよいですが、
実装上は型を分けた方が安全です。

3.2 必須API一覧
3.2.1 load_resource
load_resource(resource_id: int, flags: int) -> request_id
意味

非同期ロード要求を発行

即時に request_id を返す

cached でも統一して request 完了通知経由にしてよい

flags 例

preload

stream

decode_only

upload_gpu

high_priority

3.2.2 poll_request
poll_request(request_id: int) -> table

返却例:

{ done=true, ok=true, handle=123 }
{ done=true, ok=false, error=5 }
{ done=false }
3.2.3 await_request
await_request(request_id: int) -> handle | nil
意味

未完了なら worker を block して yield

完了時に再開して結果を返す

これは便利ですが、VMに「暗黙停止」を入れるので、
明示的にしたいなら recv ベースに寄せてもよいです。

実運用の推奨

内部的には await_request を持ってよいです。
ただし実装は scheduler block + completion event に落とす。

3.2.4 retain_resource
retain_resource(handle: int) -> bool

参照カウントまたは pin を増やす。

3.2.5 release_resource
release_resource(handle: int) -> bool

参照を外す。

3.2.6 get_resource_state
get_resource_state(handle: int) -> int

例:

0 unloaded

1 loading

2 ready_cpu

3 ready_gpu

4 failed

3.2.7 get_resource_info
get_resource_info(handle: int) -> table

返却例:

{
  type = 1,
  width = 1920,
  height = 1080,
  size = 1048576
}
3.2.8 cancel_request
cancel_request(request_id: int) -> bool
注意

キャンセルは best effort

すでに完了していたら no-op

セーブ時に pending を整理するのに使える

4. ResourceManager内部仕様
4.1 リソース要求と実体を分ける

ここはかなり重要です。

実体
struct ResourceEntry {
    id: ResourceId,
    state: ResourceState,
    ref_count: u32,
    pin_count: u32,
    generation: u32,
    data: Option<ResourceData>,
    error: Option<ResourceError>,
    last_access_frame: u64,
}
要求
struct ResourceRequest {
    request_id: RequestId,
    resource_id: ResourceId,
    worker_id: u32,
    state: RequestState,
    result_handle: Option<Handle>,
    error: Option<ResourceError>,
    flags: u32,
}
分ける理由

同じ resource に複数 request が飛ぶ

1回のロード完了を複数workerが待てる

セーブ時に pending request を正規化しやすい

4.2 RequestState
enum RequestState {
    Pending,
    InFlight,
    Completed,
    Failed,
    Cancelled,
}
5. VM ↔ Host API の返却モデル
5.1 即時完了系
play_audio(handle) -> bool
draw_image(handle, x, y) -> bool

即時で戻る。

5.2 非同期開始系
req = load_resource(id, flags)

即 request_id を返す。

5.3 待機系
handle = await_request(req)

未完了なら worker を block。

5.4 メッセージ通知系

非同期完了をメッセージキューで返す設計もよいです。

resource_complete {
  request_id,
  ok,
  handle,
  error
}
推奨

完成形では両対応がよいです。

スクリプト簡易用途: await_request

高度制御用途: completion message

内部は同じ completion source を共有する。

6. スケジューラと非同期IOの統合モデル

ここが全体の心臓です。

6.1 worker状態

最終的に worker state をこう定義するときれいです。

enum WorkerState {
    Runnable,
    Sleeping { wake_at: u64 },
    WaitingMessage,
    BlockedOnRequest { request_id: RequestId },
    Halted,
    Error,
}
6.2 非同期IOモデル
原則

VMスレッドではIOしない

Host API はIO要求を JobSystem / AsyncIO に投げる

完了イベントだけ Scheduler に返す

6.3 構成
Main Thread / Game Loop
 ├─ Scheduler
 ├─ VM workers
 ├─ ResourceManager frontend
 └─ CompletionQueue

IO Threads / Job Threads
 ├─ archive read
 ├─ decompress
 ├─ decode
 └─ optional GPU prepare stage
6.4 完了通知
enum CompletionEvent {
    ResourceRequestDone {
        request_id: RequestId,
        ok: bool,
        handle: Option<Handle>,
        error: Option<u32>,
    },
    SaveDone {
        ticket: u64,
        ok: bool,
        error: Option<u32>,
    },
}

Scheduler は毎フレーム冒頭または末尾で CompletionQueue を掃きます。

6.5 実行ループ
frame begin
  1. completion queue を処理
  2. sleeping worker を wake 判定
  3. message queue により waiting worker を revive
  4. runnable worker を step budget だけ実行
  5. eviction / streaming / maintenance
frame end
6.6 CALL_HOST(load_resource) の流れ
worker A:
  CALL_HOST load_resource(42)

Host API:
  request_id = create_request(42)
  if resource already ready:
      mark request completed
  else:
      enqueue IO/decode job
  push request_id
  continue
6.7 CALL_HOST(await_request) の流れ
worker A:
  CALL_HOST await_request(req=100)

Host API:
  if req completed:
      return handle
  else:
      set worker.state = BlockedOnRequest { 100 }
      suspend current execution

完了イベント到着後:

Scheduler:
  request 100 completed
  worker A -> Runnable

次回再開時に await_request の結果を返す、または再実行ポイントに結果を注入します。

7. 再開モデルの厳密化

ここは実装差が出やすいです。

7.1 推奨方式

Host call が block した場合、VMフレームに pending host continuation を持たせます。

struct PendingHostCall {
    host_id: u16,
    request_id: RequestId,
    dest: PendingReturnTarget,
}
再開時

await_request の続きとして戻り値をスタックに積む

pc は次命令へ進んだ状態にしておく

これは「call途中で止まる」扱いです。

7.2 代替方式

await_request を廃して、スクリプト側にこう書かせる。

req = load_resource(id, 0)
while !poll_request(req).done do
    yield()
end
h = poll_request(req).handle

これは単純ですが、記述が冗長です。

結論

VM内部実装は BlockedOnRequest

スクリプト表現としては await_request を提供
が一番バランスがよいです。

8. 非同期IOジョブの段階設計

ロード処理は1ジョブではなく段階に分けた方がよいです。

8.1 典型パイプライン
Archive Read
  -> Verify / Decode Container
  -> Decompress
  -> Decode Asset Format
  -> Build Runtime Object
  -> Optional GPU Upload
  -> Complete Request
8.2 JobState
enum JobState {
    Queued,
    Reading,
    Decoding,
    UploadPending,
    Completed,
    Failed,
}
8.3 GPUアップロード

GPU API がメインスレッド制約なら、そこだけ Scheduler 側 maintenance phase で処理します。

つまり

IO/デコード: worker thread

GPU upload: main/render thread

完了通知: completion queue

9. セーブデータとの完全整合設計

ここからが難所です。
一番重要な原則を先に固定します。

9.1 原則

セーブは VMメモリの生スナップショットではなく、再構成可能な論理状態を保存する。

保存するのは:

ゲーム進行状態

VM公開状態

worker状態

pending request の論理情報

Resource pin 状態

Host state の再構築情報

保存しないのは:

OSファイルハンドル

デコード途中バッファ

スレッド状態

GPU生オブジェクト

生ポインタ

一時キャッシュ

9.2 セーブ整合レベル

レベルを分けると設計しやすいです。

レベルA: 論理整合

ロード後に見た目の進行が一致すればよい
→ 最低限これを保証

レベルB: イベント整合

待機中イベントやメッセージ状態も再現
→ 推奨

レベルC: 命令境界整合

VMの停止位置まで同一
→ 実現可能なら採用

結論

完成形では 命令境界整合 + 再構成方式 がよいです。

10. セーブ対象の定義
10.1 VM側
struct SavedVmWorker {
    worker_id: u32,
    state: SavedWorkerState,

    current_module_id: u32,
    pc: u32,

    stack: Vec<Value>,
    frames: Vec<SavedCallFrame>,
    globals: Vec<Value>,

    mailbox: Vec<SavedMessage>,

    pending_host_call: Option<SavedPendingHostCall>,
}
注意

前に「完全VM snapshotは避ける」と置いていたが、
ここまで進めると命令境界の限定スナップショットは十分ありです。
ただしヒープ生ポインタを含めず、Value graph をシリアライズ可能であることが条件です。

10.2 WorkerState の保存
enum SavedWorkerState {
    Runnable,
    Sleeping { remaining_ms: u64 },
    WaitingMessage,
    BlockedOnRequest { request_key: SavedRequestKey },
    Halted,
    Error { code: u32 },
}
ポイント

sleeping は絶対時刻ではなく 残り時間 で保存。

10.3 CallFrame の保存
struct SavedCallFrame {
    func_id: u32,
    return_pc: u32,
    base_sp: u32,
    locals_count: u16,
    arg_count: u16,
}
10.4 Heap / Value の保存

Value が

int

float

bool

string

array

table

handle

なら、保存可能なのは:

int

float

bool

string

nil

array

table

handle はそのまま保存しません。
handle は resource reference に変換して保存します。

11. リソース整合の保存
11.1 handle は保存禁止

保存すると壊れます。
代わりにこう保存します。

struct SavedResourceRef {
    resource_id: u32,
    expected_type: u16,
    state_hint: u16,      // optional
    pinned: bool,
}

ロード時に再解決して新しい handle を発行します。

11.2 どこに handle があるか

handle が

VM stack

globals

tables

mailbox payload

に入り得るなら、保存時に全 Value を walk して置換します。

ルール

Value::Handle(h) を見つけたら

HandleTable から resource_id を逆引き

SavedResourceRef に変換

load後に新 handle を埋め戻す

11.3 ResourceManager 側で保存するもの
struct SavedResourceManagerState {
    pinned_resources: Vec<SavedPinnedResource>,
    pending_requests: Vec<SavedPendingRequest>,
}
pinned
struct SavedPinnedResource {
    resource_id: u32,
    pin_count: u32,
}
12. pending async の保存

ここが「完全整合」の核です。

12.1 保存方針

非同期要求は二種類に分けます。

再実行可能

resource load

archive read

decode request

→ 論理要求として保存し、load後に再発行

再実行不能

外部ネットワーク

OSネイティブ対話

途中の不可逆処理

→ 原則禁止またはセーブバリア前に解決必須

ゲーム用途なら後者を最初から host policy で抑えるべきです。

12.2 SavedPendingRequest
struct SavedPendingRequest {
    request_kind: u16,
    logical_request_id: u64,
    worker_id: u32,
    resource_id: u32,
    flags: u32,
}
ロード時

新しい runtime request_id を発行

logical_request_id -> runtime_request_id を再マップ

BlockedOnRequest worker に新requestを紐付ける

12.3 request id の扱い

request_id をそのまま保存しません。
保存するのは 論理 request key です。

struct SavedRequestKey {
    logical_id: u64,
    kind: u16,
}
13. セーブポイント規約

完全整合にするなら、セーブ可能タイミングを決める必要があります。

13.1 推奨

命令境界かつ scheduler safe point でのみ保存可能

safe point 条件:

実行中 worker が host call の途中でない

completion queue 処理中でない

ResourceManager 内部更新中でない

GC中でない

13.2 セーブ手順
1. scheduler を save barrier に入れる
2. 新規 host call / request 発行を一時停止
3. completion queue を排水
4. runnable worker を命令境界で停止
5. blocked/sleeping/waiting 状態を確定
6. resource handles を SavedResourceRef に変換
7. pending requests を論理表現へ正規化
8. state + vm + resource + metadata を書き出す
9. barrier解除
14. ロード手順
1. archive / build / signature / version を検証
2. ResourceManager を初期化
3. saved pinned resources を再ロード予約
4. saved pending requests を再発行
5. VM workers を復元
6. saved resource refs を新handleへ解決
7. blocked workers の request mapping を再接続
8. scheduler 開始
15. Saveデータの構造
15.1 全体
struct SaveFile {
    magic: [u8; 4],       // b"SAV1"
    version: u16,
    flags: u16,

    archive_id: u128,
    build_id: u128,
    save_compat_version: u16,
    reserved: u16,

    game_state: SavedGameState,
    vm_state: SavedVmState,
    resource_state: SavedResourceManagerState,
    host_state: SavedHostState,
}
15.2 host state

Resource以外も将来必要なので分けます。

struct SavedHostState {
    audio: SavedAudioState,
    ui: SavedUiState,
    custom: Vec<SavedHostBlob>,
}
例

BGM再生位置

現在表示中立ち絵

UI選択状態

フェード途中なら残り時間

16. セーブとメッセージキューの整合

メッセージキューも保存対象です。

struct SavedMessage {
    msg_type: u32,
    from_worker: u32,
    payload: SavedValue,
}
注意

payload中の handle も SavedResourceRef 化する。

17. ホストAPIとセーブ互換規約

Host API はセーブ可能性を宣言した方がよいです。

struct HostFuncMeta {
    host_id: u16,
    flags: u32,
}
flags 例

PURE

MAY_BLOCK

RETURNS_HANDLE

SAVE_SAFE

SAVE_REQUIRES_REPLAY

NON_REPLAYABLE

重要

NON_REPLAYABLE な host call が pending のときは保存拒否。
これを仕様に入れておくと破綻しにくいです。

18. 典型フロー
18.1 画像ロード中に待つ
req = load_resource(bg_01, PRELOAD | UPLOAD_GPU)
bg = await_request(req)
draw_image(bg, 0, 0)
内部

request 作成

worker が BlockedOnRequest

decode/upload 完了

completion queue

worker 再開

bg に新handle

18.2 この途中でセーブ

保存時に

worker state = BlockedOnRequest { logical_id=77 }

pending request = resource_id=bg_01

handle はまだ未存在

ロード時に

同じ logical request を再発行

完了したら worker を再開

見た目としては自然につながる。

19. 実装の要点
19.1 Host API 戻り値の統一

VMに返す型を乱さない。

推奨は:

即時成功: Value

即時失敗: ErrorValue table or nil

非同期待ち: scheduler state change + deferred return

19.2 handle と request を混同しない

resource handle = 実体参照

request id = 進行中操作参照

19.3 セーブ時は runtime id を保存しない

handle

request_id

OS object id

は保存禁止。

20. 3要素の最終統合仕様
ホストAPI仕様

CALL_HOST host_id argc

Resource系は load_resource, poll_request, await_request, retain_resource, release_resource, get_resource_info

request と handle を分離

非同期結果は completion event で管理

スケジューラと非同期IO

worker state に BlockedOnRequest

IOは別ジョブ系

completion queue を毎フレーム処理

await_request は内部的に worker block として実装

セーブ完全整合

命令境界 safe point で保存

VM状態は論理スナップショット

handle は SavedResourceRef へ変換

pending async は論理 request として保存し再発行

non-replayable host call は保存拒否

21. 最終結論

この設計で一番大事なのは、
「非同期処理をそのまま保存しない。保存可能な論理要求へ正規化する」 ことです。

これを守ると

VM

ResourceManager

Scheduler

Save/Load

がきれいにつながります。

次に詰めるなら、かなり実装直前です。
Rust の trait / struct 単位のインターフェース定義まで落とせます。