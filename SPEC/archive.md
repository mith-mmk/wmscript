# アーカイブ設計（VM統合前提）
## 責務
- パッケージング、整合性検証、署名検証、任意暗号化を定義する。
- VM と ResourceManager がロードする配布単位を定義する。

## 依存
- module と bytecode の検証は [SPEC/vm.md](vm.md) と [SPEC/op.md](op.md) に依存する。
- asset / string / manifest の意味は [SPEC/resource.md](resource.md) に依存する。

1. 目的

アーカイブ層の責務を最終的にこう固定します。

パッケージング

整合性検証

署名検証

任意で暗号化

バージョン管理

モジュール索引

実行前検証

capability 制御

ホットリロードの境界管理

2. 最終ファイル構造

拡張子は仮に .warc のままで進めます。

archive.warc
├── fixed_header
├── security_header
├── section_table
├── sections...
├── signature_block
└── optional_footer
2.1 fixed_header

固定長で先頭に置く領域です。

#[repr(C)]
struct FixedHeader {
    magic: [u8; 4],          // b"WARC"
    archive_version: u16,    // 1
    header_size: u16,

    flags: u32,              // signed / encrypted / compressed ...
    section_count: u32,

    section_table_offset: u64,
    section_table_size: u64,

    security_offset: u64,
    security_size: u64,

    signature_offset: u64,
    signature_size: u64,
}
2.2 security_header

セキュリティ方式を宣言します。

#[repr(C)]
struct SecurityHeader {
    security_version: u16,   // 1
    sig_alg: u16,            // 1=Ed25519, 2=ECDSA_P256, ...
    hash_alg: u16,           // 1=SHA-256, 2=BLAKE3
    enc_alg: u16,            // 0=None, 1=AES-256-GCM, 2=XChaCha20-Poly1305

    key_id: [u8; 16],        // 公開鍵識別子
    nonce_len: u16,
    reserved: u16,

    manifest_digest_offset: u64,
    manifest_digest_size: u64,
}
2.3 section_table

各セクションの位置と属性です。

#[repr(C)]
struct SectionEntry {
    id: u32,
    kind: u16,
    flags: u16,

    offset: u64,
    packed_size: u64,
    unpacked_size: u64,

    align: u32,
    reserved: u32,

    name_hash: u64,
}
3. セクション種別
0x0001 manifest
0x0002 module
0x0003 string_table
0x0004 const_pool
0x0005 ext_table
0x0006 asset
0x0007 debug
0x0008 dependency_table
0x0009 symbol_table
0x000A relocation_stub   ; 基本は未使用
0x000B metadata
4. manifest 完成形

manifest は JSON より、最終的には バイナリ化した方がよいです。
ただ、開発中は JSON / TOML からビルドして、出力ではバイナリ manifest に落とすのが扱いやすいです。

4.1 manifest に含める項目

- アーカイブ名
- package version
- bytecode version
- build id
- entry module
- entry function
- capability
- 依存 ext
- 依存 archive
- 対応プラットフォーム
- 最小ランタイム version
- セクションダイジェスト一覧
- ポリシー
- 署名対象範囲定義

4.2 論理表現
manifest {
  package_name
  package_version
  bytecode_version
  archive_id
  build_id

  entry_module_id
  entry_func_id

  capability_mask
  required_extensions[]
  required_archives[]

  runtime_min_version
  target_platform_mask

  section_digests[]
  policy_flags
}
5. 秘密鍵で保護する仕組み

ここが本体です。

5.1 正しい役割分担
署名

ビルドサーバまたはリリース工程が

秘密鍵で署名

ランタイムが

公開鍵で検証

効果

改ざん検出

正規ビルド確認

差し替え攻撃防止

ホットリロード時の信頼判定

5.2 推奨方式

Rust前提なら、まずはこれが素直です。

署名: Ed25519

ハッシュ: SHA-256 または BLAKE3

暗号化が必要な場合: XChaCha20-Poly1305 か AES-256-GCM

理由

Ed25519 は実装が比較的扱いやすい

鍵サイズが小さい

署名検証が軽い

Rustクレートの選択肢が安定している

6. 署名対象

ここを曖昧にすると危険です。

6.1 原則

署名対象は、署名ブロック自身を除くアーカイブ全体です。

signed_region =
    fixed_header(with signature offsets fixed)
  + security_header
  + section_table
  + all sections
  + footer without signature blob
6.2 より厳密な方法

各 section の digest を作り、その digest 群を含んだ manifest を署名対象にします。

section -> digest
all digests -> manifest
manifest -> signature
利点

部分検証しやすい

キャッシュしやすい

ホットリロード差分検証しやすい

結論

完成形としては、両方あるのが強いです。

section digest 一覧

archive 全体署名

7. signature_block
#[repr(C)]
struct SignatureBlockHeader {
    sig_count: u16,
    reserved: u16,
    entries_offset: u32,
}

#[repr(C)]
struct SignatureEntry {
    sig_alg: u16,
    hash_alg: u16,
    signer_type: u16,     // dev / release / patch
    flags: u16,

    key_id: [u8; 16],
    signed_offset: u64,
    signed_size: u64,

    sig_offset: u32,
    sig_size: u32,
}
7.1 複数署名対応

完成形では複数署名対応にしておくと運用が楽です。

開発署名

公式リリース署名

パッチ署名

DLC署名

実行ポリシー例

開発ビルドでは dev key を許可

製品版では release key のみ許可

mod ローダでは mod key ring を別扱い

8. 鍵管理
8.1 ランタイム側

持つのは 公開鍵 だけです。

struct TrustAnchor {
    key_id: [u8; 16],
    public_key: [u8; 32], // Ed25519
    flags: u32,
}
flags 例

allow_dev

allow_release

allow_patch

allow_mod

8.2 ビルド側

持つのは 秘密鍵 です。
これはランタイムやクライアントに絶対入れません。

CI/CD の秘密管理領域で保持

オフライン署名機で保持

開発鍵と製品鍵を分離

8.3 key_id

公開鍵そのものを毎回比較するのではなく、識別子を持たせます。

例:

公開鍵の先頭 16 byte ではなく

公開鍵全体のハッシュの先頭 16 byte

9. 検証フロー

ロード時はこうなります。

1. fixed_header 読み込み
2. magic / version / 範囲チェック
3. security_header 読み込み
4. section_table 読み込み
5. section境界チェック
6. digest計算
7. signature_block 読み込み
8. key_id に対応する公開鍵を trust store から取得
9. 署名検証
10. manifest検証
11. capability / ext / runtime version チェック
12. module verifier 実行
13. VM登録
10. 失敗理由の分類

ランタイムで雑に invalid archive にすると運用しにくいです。

enum ArchiveError {
    InvalidMagic,
    UnsupportedVersion,
    BrokenLayout,
    SectionOutOfRange,
    InvalidDigest,
    MissingSignature,
    UnknownKey,
    SignatureMismatch,
    PolicyRejected,
    CapabilityRejected,
    RuntimeVersionMismatch,
    InvalidModule,
    InvalidBytecode,
}
11. 内容秘匿まで必要な場合

署名だけでは中身は読めます。
非公開シナリオや商用配布で抽出を難しくしたいなら暗号化を足します。

ただし、ここは冷静に見る必要があります。

11.1 クライアント復号型の限界

クライアントが実行できる以上、どこかで復号鍵を持つので、
完全秘匿はできません。

できるのは：

直接展開しにくくする

雑な改造を防ぐ

正規手順以外の解析コストを上げる

11.2 暗号化方式
実用形

アーカイブ本体を共通鍵で暗号化

その共通鍵をプラットフォーム鍵やセッション鍵で保護

archive payload --(content key)--> encrypted
content key --(platform key / session key)--> wrapped_key
ただし

秘密鍵で直接暗号化する、は採りません。
公開鍵暗号を使うとしても、通常は

公開鍵で鍵を包む

秘密鍵で復号する

です。
配布アーカイブではこの形もあまり向きません。クライアントに秘密鍵を置けないためです。

12. 実用的な保護レベル

ゲーム用途なら、実際にはこの3段階が現実的です。

レベル1: 署名のみ

改ざん防止

正規データ確認

最も重要

レベル2: 署名 + 圧縮 + 軽難読化

生ファイル直読を少し難しくする

開発コスト低い

レベル3: 署名 + セクション暗号化

商用配布向け

復号器がクライアント内に必要

完全防御ではない

完成形としては、必須は署名、暗号化は 任意機能 にするのがよいです。

13. ホットリロードと署名

ホットリロード対応するなら、差し替えモジュールにも署名判定が必要です。

13.1 方法

アーカイブ全体署名

もしくはモジュール単位署名

推奨

完成形では

本体アーカイブ署名

パッチアーカイブ署名
の二系統に分けます。

13.2 差し替え条件

package_id 一致

module_id 一致

bytecode_version 一致

signer policy 一致

capability 逸脱なし

14. MOD対応まで見据えた設計

将来 mod を許可するなら、信頼レベルを分けた方がよいです。

enum TrustLevel {
    FirstParty,
    PartnerSigned,
    UserMod,
    UnsignedDev,
}

これに応じて

使える capability

使える ext

asset 参照範囲

セーブデータ互換
を制御します。

15. Rust の型イメージ
pub struct Archive<'a> {
    pub data: &'a [u8],
    pub header: FixedHeader,
    pub security: SecurityHeader,
    pub sections: Vec<SectionEntry>,
}

pub struct VerifiedArchive<'a> {
    pub archive: Archive<'a>,
    pub manifest: Manifest,
    pub trust: TrustLevel,
}

pub struct SignatureInfo {
    pub key_id: [u8; 16],
    pub sig_alg: SigAlg,
    pub hash_alg: HashAlg,
    pub signer_type: SignerType,
    pub signature: Vec<u8>,
}

pub enum SigAlg {
    Ed25519,
}

pub enum HashAlg {
    Sha256,
    Blake3,
}
16. 実装ポリシー
16.1 絶対にやる

署名検証

section 範囲検証

manifest digest 検証

bytecode verifier

key ring による許可判定

16.2 最初はやらなくていい

証明書チェーン

オンライン失効確認

複雑な鍵ローテーション自動化

高度な DRM

17. 完成形の最終仕様
アーカイブ

単一ファイル （ゲームシステムの分割・統合はラッパーで行う）

fixed header + security header + section table + sections + signature block

section 単位アクセス

mmap 可能

streaming reader 可能

fixed header / security header / section table を先に読めば、
後続 section は `Read + Seek` で逐次ロードできる前提にする

ID 参照のみ

manifest 中心管理

18.1 ストリーミング実装前提

- archive loader は fixed header と section table を先に読み、section offset を確定させる
- module section と manifest section はオンデマンドで個別に読める
- asset section は section 単位で順次ロードできる
- 署名検証や digest 検証は section payload を丸ごと読む必要はあるが、archive 全体の常駐は必須にしない
- `Read + Seek` が使える実装では、巨大 archive でも header + active section だけを保持する

セキュリティ

秘密鍵で署名

公開鍵で検証

section digest + archive 署名

trust store による許可信頼管理

任意で暗号化

ランタイム

ロード前に完全検証

検証成功後のみ VM 登録

capability と signer policy を連動

パッチ / ホットリロードも同じ検証系を通す

18. 一番大事な結論

この設計での「秘密鍵による保護」は、正確にはこうです。

改ざん防止は
秘密鍵署名 + 公開鍵検証

秘匿は
別途共通鍵暗号

実運用で最重要なのは署名

ゲーム配布で暗号化は補助機能

# 署名対象バイト列の厳密定義
1. 署名対象バイト列の厳密定義

まず原則です。

1.1 基本原則

署名対象は アーカイブの意味内容が変わる全バイト列 です。
逆に、署名そのものや後付け可能な可変領域は除外します。

つまり完成形では、署名対象を次のように固定します。

signed_message =
    canonical_fixed_header
  + canonical_security_header
  + canonical_section_table
  + section_payload_bytes
  + canonical_manifest_digest_table

ただし、実装しやすさのため、実際には以下の二層に分けるのが安全です。

層A: 各セクションの digest を取る

層B: manifest と digest table をまとめて署名する

これで、

セクション単位検証ができる

巨大アーカイブでも扱いやすい

ホットリロード差分検証がしやすい

1.2 推奨する署名モデル

完成形では次の2本立てを推奨します。

方式1: セクション digest 署名

各セクションについて digest を計算し、その一覧を manifest に入れる。
その manifest セクション自体を含めたメタデータ領域 を署名する。

方式2: 全体 canonical 署名

署名ブロックを除く全体から canonical byte stream を組み立てて署名する。

1.3 実務上の結論

最初の実装は 方式1だけで十分 です。
そのうえで製品版では 方式2も追加すると強いです。

2. 署名対象範囲の厳密ルール

ここから完全に固定します。

2.1 署名対象に含めるもの

含める:

FixedHeader のうち 署名結果で変動しないフィールド

SecurityHeader のうち 署名アルゴリズムや key_id など意味のある宣言

SectionTable

各 section の平文 payload

ManifestDigestTable

manifest 本体

含めない:

SignatureBlock 本体

署名生成時にしか決まらないオフセット類で、署名によって自己参照になるもの

実行時キャッシュ

mmap都合のパディングで意味を持たない未使用領域

2.2 固定ヘッダの canonical 化

FixedHeader はそのまま全バイトを署名対象にすると、
signature_offset や signature_size が自己参照になって壊れやすいです。

なので、署名対象用には canonical header を別定義します。

署名対象用 canonical fixed header
#[repr(C)]
struct CanonicalFixedHeader {
    magic: [u8; 4],           // b"WARC"
    archive_version: u16,
    header_size: u16,

    flags: u32,               // signed/encrypted/compressed...
    section_count: u32,

    section_table_offset: u64,
    section_table_size: u64,

    security_offset: u64,
    security_size: u64,

    // signature related fields are ZEROED
    signature_offset: u64,    // 0
    signature_size: u64,      // 0
}
ルール

実ファイル上の signature_offset, signature_size は存在してよい

署名時は 必ず 0 として canonical stream に入れる

2.3 security header の canonical 化

SecurityHeader も可変領域参照を直接持つと危険なので、
digest 対象と署名アルゴリズム宣言を中心に固定します。

#[repr(C)]
struct CanonicalSecurityHeader {
    security_version: u16,
    sig_alg: u16,
    hash_alg: u16,
    enc_alg: u16,

    key_id: [u8; 16],

    nonce_len: u16,
    reserved: u16,

    manifest_digest_table_offset: u64,
    manifest_digest_table_size: u64,
}
ルール

署名生成時に書き換わらない宣言だけを入れる

一時 nonce 本体や wrapped key のような可変 blob は section 側へ逃がす

3. セクション payload の digest 仕様

これが一番重要です。

3.1 digest 対象

各 SectionEntry に対して、digest は section payload の論理内容 を対象にします。

原則

圧縮されているなら展開後の平文

暗号化されているなら復号後の平文

アラインメント用パディングは含めない

つまり digest は 意味内容 に対して取ります。

3.2 section digest の定義
section_digest[i] = HASH(
    section_kind
  || section_flags_canonical
  || unpacked_size
  || payload_plaintext_bytes
)
なぜ metadata も入れるか

payload だけをハッシュすると、

module を asset と偽装

flags をすり替え

unpacked_size を改ざん

の検出が甘くなります。

3.3 canonical section flags

flags はそのままだと「署名に関係あるもの」「関係ないもの」が混ざるので分けます。

digest に含めるべき flags

compressed

encrypted

readonly

executable

manifest_critical

digest に含めない方がよい flags

cache_hint

preload_hint

memory_map_hint

つまり:

const SECTION_FLAGS_SIGNED_MASK: u16 = 
      FLAG_COMPRESSED
    | FLAG_ENCRYPTED
    | FLAG_READONLY
    | FLAG_EXECUTABLE
    | FLAG_MANIFEST_CRITICAL;
4. ManifestDigestTable の厳密定義

manifest に全部埋め込んでもよいですが、実装分離のため専用テーブルにした方がきれいです。

4.1 レイアウト
#[repr(C)]
struct ManifestDigestTableHeader {
    version: u16,          // 1
    hash_alg: u16,         // 1=SHA-256, 2=BLAKE3
    entry_count: u32,
}

#[repr(C)]
struct ManifestDigestEntry {
    section_id: u32,
    section_kind: u16,
    flags_canonical: u16,
    unpacked_size: u64,
    digest_size: u16,      // e.g. 32
    reserved: u16,
    digest_offset: u32,    // from start of digest blob area
}

この後ろに digest blob を連結します。

4.2 並び順

並び順が揺れると署名が壊れるので固定します。

ルール

ManifestDigestEntry は必ず次の昇順:

(section_id ASC, section_kind ASC)

同一 section_id 重複は禁止。

5. 署名メッセージの厳密定義

ここでは方式1: digest table 中心署名を厳密化します。

5.1 signed_message_v1
signed_message_v1 =
    "WARC-SIG\0"                             // 8 bytes domain separator
  || u16_le(signature_format_version = 1)
  || CanonicalFixedHeader
  || CanonicalSecurityHeader
  || CanonicalSectionTable
  || ManifestSectionCanonicalBytes
  || ManifestDigestTableCanonicalBytes
5.2 domain separator

これは必須です。

"WARC-SIG\0"

を先頭に必ず付ける。
別用途の署名値流用を防ぐためです。

5.3 SectionTable の canonical bytes

section table も raw bytes をそのまま署名対象にするより、
不要フィールドを落とした canonical record にした方が安全です。

#[repr(C)]
struct CanonicalSectionEntry {
    id: u32,
    kind: u16,
    flags_signed: u16,

    offset: u64,
    packed_size: u64,
    unpacked_size: u64,

    align: u32,
    reserved: u32, // always 0 in canonical form
    name_hash: u64,
}
ルール

reserved は canonical 化時に 0

並び順は実ファイル順ではなく section_id 昇順

section_id は一意

6. manifest のバイナリレイアウト

ここから本題です。
manifest は 最小限の固定長 + 可変長テーブル にします。

JSON/TOML をそのまま持つのは開発中だけにして、配布物はバイナリ化します。

6.1 設計方針

manifest に必要なのは:

パッケージ識別

バージョン

実行条件

エントリポイント

capability

ext 依存

セクション digest 参照

ポリシー

つまり、文字列の生埋め込みを減らし、ID と offset 中心にします。

6.2 manifest セクション全体構造
#[repr(C)]
struct ManifestHeader {
    magic: [u8; 4],          // b"MNF1"
    version: u16,            // 1
    header_size: u16,

    total_size: u32,

    string_table_offset: u32,
    string_table_size: u32,

    ext_table_offset: u32,
    ext_count: u32,

    archive_dep_offset: u32,
    archive_dep_count: u32,

    digest_ref_offset: u32,
    digest_ref_count: u32,

    policy_offset: u32,
    policy_size: u32,
}

このあとに固定長の ManifestCore を置きます。

6.3 ManifestCore
#[repr(C)]
struct ManifestCore {
    archive_id: u128,            // build/package unique id
    build_id: u128,              // concrete build instance id

    package_name_str: u32,       // string table offset
    package_version_major: u16,
    package_version_minor: u16,
    package_version_patch: u16,
    package_version_extra: u16,  // prerelease/build metadata index or 0

    bytecode_version: u16,
    runtime_min_version: u16,

    target_platform_mask: u64,
    capability_mask: u64,
    policy_flags: u64,

    entry_module_id: u32,
    entry_func_id: u32,

    signer_policy: u16,
    trust_policy: u16,

    reserved0: u32,
}
6.4 string table

manifest 内の文字列は、全部ここに集約します。

フォーマット
#[repr(C)]
struct ManifestStringTableHeader {
    size: u32,
    count: u32,
}

後ろに:

[StringEntry...][UTF-8 bytes...]
#[repr(C)]
struct StringEntry {
    offset: u32,   // from start of utf8 blob
    len: u32,
}
参照方法

package_name_str は StringEntry の index にする方が安全

つまり名前が offset ではなく index を指す

なので先ほどの package_name_str は正確には:

package_name_str_index: u32

に直した方がよいです。

6.5 ext dependency table

ランタイム拡張依存です。

#[repr(C)]
struct ManifestExtEntry {
    ext_name_str_index: u32,
    min_version: u16,
    max_version: u16,      // 0xFFFF = no upper bound
    required_flags: u32,
}
6.6 archive dependency table

別アーカイブ依存です。

#[repr(C)]
struct ManifestArchiveDepEntry {
    archive_id: u128,
    package_name_str_index: u32,
    min_version_major: u16,
    min_version_minor: u16,
    min_version_patch: u16,
    flags: u16,
}
6.7 digest reference table

manifest 自体は digest 本体を持たず、
ManifestDigestTable への参照だけ持つ形にすると分離がきれいです。

#[repr(C)]
struct ManifestDigestRefEntry {
    section_id: u32,
    digest_index: u32,     // index into ManifestDigestTable entries
}
ルール

section_id と digest_index は 1:1

digest table 側の entry_count と整合していること

6.8 policy block

実行ポリシーです。

#[repr(C)]
struct ManifestPolicyBlock {
    save_compat_version: u16,
    hot_reload_policy: u16,
    mod_policy: u16,
    encryption_policy: u16,

    max_worker_count: u16,
    max_message_size_kib: u16,
    max_heap_size_mib: u16,
    max_stack_depth: u16,

    reserved: [u8; 16],
}
7. manifest section の canonical bytes

署名対象にする manifest も、そのまま raw bytes を取るより
reserved を 0 化した canonical bytes を取る方が安定します。

7.1 canonical 化ルール

reserved* はすべて 0

可変長テーブルの並び順を固定

string table は UTF-8 バイト列そのもの を保存

正規化はしない

同一文字列の重複は許可してもよいが、ビルド時 dedup 推奨

7.2 テーブル並び順
ext table

ext_name_str_index ASC

archive dependency table

archive_id ASC

digest reference table

section_id ASC

8. バイトオーダーとアラインメント

ここも固定します。

8.1 エンディアン

すべて little endian

8.2 アラインメント

セクション本体は 16 byte align 推奨

manifest 内部構造は packed で扱わず、読み出し時に個別 decode

Rust の #[repr(C)] は仕様表現用

実際の読み込みは from_le_bytes でやる

これはかなり重要です。
unsafe に struct cast し始めると移植性と壊れやすさが一気に増えます。

9. Rust 実装向けの厳密ルール
9.1 署名メッセージ生成手順
1. fixed_header を読む
2. signature_offset/signature_size を 0 にした canonical_fixed_header を構築
3. security_header を canonical 化
4. section_table を section_id 昇順で canonical 化
5. manifest section を canonical bytes 化
6. digest table を canonical bytes 化
7. "WARC-SIG\0" + version + 各 canonical bytes を連結
8. その結果に対して署名
9.2 検証手順
1. archive から manifest と digest table を読む
2. 各 section の平文 digest を再計算
3. digest table と一致確認
4. canonical signed_message_v1 を再構築
5. signature block の署名を公開鍵で検証
6. signer policy / trust policy を確認
10. 署名対象から除外するものの明文化

ここも仕様書に書いておくべきです。

除外対象:

SignatureBlock の全バイト

セクション末尾パディング

ファイル末尾の余剰未使用領域

実行時再配置情報

キャッシュ生成物

OS依存メタデータ

11. 推奨する最終仕様
署名対象バイト列

以下の canonical 連結列:

"WARC-SIG\0"
+ u16_le(1)
+ CanonicalFixedHeader
+ CanonicalSecurityHeader
+ CanonicalSectionTable(section_id ascending)
+ CanonicalManifestSection
+ CanonicalManifestDigestTable
各セクション digest

以下のハッシュ:

HASH(
    u32_le(section_id)
 || u16_le(section_kind)
 || u16_le(flags_canonical)
 || u64_le(unpacked_size)
 || plaintext_payload_bytes
)

※ section_id を含める版の方が、セクション入れ替え攻撃に強いです。
先ほどの定義より、最終的には section_id を含める方がよいです。

12. manifest バイナリレイアウト最終形
構成順
[ManifestHeader]
[ManifestCore]
[ManifestStringTableHeader]
[StringEntry * N]
[UTF8 Blob]
[ManifestExtEntry * ext_count]
[ManifestArchiveDepEntry * archive_dep_count]
[ManifestDigestRefEntry * digest_ref_count]
[ManifestPolicyBlock]
最終版の主要構造
#[repr(C)]
struct ManifestHeader {
    magic: [u8; 4],      // b"MNF1"
    version: u16,
    header_size: u16,
    total_size: u32,

    core_offset: u32,

    string_table_offset: u32,
    string_table_size: u32,

    ext_table_offset: u32,
    ext_count: u32,

    archive_dep_offset: u32,
    archive_dep_count: u32,

    digest_ref_offset: u32,
    digest_ref_count: u32,

    policy_offset: u32,
    policy_size: u32,
}

#[repr(C)]
struct ManifestCore {
    archive_id: u128,
    build_id: u128,

    package_name_str_index: u32,

    package_version_major: u16,
    package_version_minor: u16,
    package_version_patch: u16,
    package_version_extra_str_index: u16,

    bytecode_version: u16,
    runtime_min_version: u16,

    target_platform_mask: u64,
    capability_mask: u64,
    policy_flags: u64,

    entry_module_id: u32,
    entry_func_id: u32,

    signer_policy: u16,
    trust_policy: u16,

    reserved0: u32,
}
13. 一番重要な実務上の注意

**署名対象は「ファイルの見た目」ではなく「canonical 化された意味内容」**に寄せる。
これを守ると、

オフセット再配置

パディング差

section 並び差

署名ブロック位置差

で署名が壊れにくいです。

逆に、雑に「ファイル全部をそのまま署名」は、最初は楽でも後でほぼ確実に詰まります。

次に詰めるべきは、かなり明確です。
Rust の decode/encode 用の実コード骨格と、署名生成アルゴリズムの擬似コードです。
