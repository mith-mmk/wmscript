# WMScript v2 設計インデックス

```yaml
language:
  version: 2
  source_extension: .wms
  typing: gradual-static
  declarations: [struct, enum, component, resource, event, func, task, system, on, test]
  waitable: [task, on]
  collections: [Array, Table-record]

vm:
  program_format: WMP1
  bytecode_version: 1
  change_allowed: false

runtime:
  world_order: entity-id-ascending
  event_order: fifo
  system_order: name-ascending
  clock: fixed-tick
  random: seeded
  ports: [input, render, audio, storage]

project:
  manifest: wms.toml
  paths: project-relative
  reject_unknown_keys: true
  targets: [headless, egui]

archive:
  magic: WARC
  version: 2
  manifest_magic: MNF2
  legacy_read: version-1-only

cli:
  binary: wms
  commands: [new, check, build, run, test, package, legacy-run]
```

詳細仕様は`SPEC/language.md`、`SPEC/gameplay.md`、`SPEC/scheduler.md`、`SPEC/hostapi.md`、`SPEC/archive.md`を参照する。VM/opcodeの規範は引き続き`SPEC/vm.md`と`SPEC/op.md`である。
