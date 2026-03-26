# 目的
- ゲームスクリプトとそれを実行するVMの作成
- 2DゲームスクリプトUIサンプル samples/uiimage.png 
  - backend scriptで一から製作可能/ main scriptと切り離せる
  - middleware worker はリアルタイム処理に使える

# UNIT MODEL
```yml
units:
  module:
    role: compile_unit
    rules:
      - static_import_only
      - no_runtime_loading
      - no_circular_dependency

  package:
    role: execution_unit
    mapping: "1 package = 1 worker"
    contains:
      - entry_module

  bundle:
    role: distribution_unit
    contains:
      - scripts
      - assets
      - manifest
```

# VM MODEL
```yml
value:
  types:
    - int64
    - float32
    - bool
    - string
    - nil
    - array
    - table
    - handle

  representation:
    type: tagged_value
    size: 16_bytes
```

# BYTECODE
```yml
bytecode:
  format:
    opcode: u8
    operands: variable_length

  instruction_groups:

    literal:
      - push_const
      - push_nil
      - push_true
      - push_false

    variable:
      - load_local
      - store_local
      - load_global
      - store_global

    data_access:
      - load_field
      - store_field
      - load_index
      - store_index

    stack:
      - pop
      - dup

    arithmetic:
      - add
      - sub
      - mul
      - div
      - mod
      - neg

    compare:
      - eq
      - ne
      - lt
      - le
      - gt
      - ge
      - not

    control:
      - jump
      - jump_if_false
      - jump_if_true

    call:
      - call(func_id, argc)
      - call_host(host_id, argc)
      - return

    concurrency:
      - send(worker_id, msg_type, argc)
      - recv
      - try_recv
      - yield
      - sleep

    allocation:
      - new_array
      - new_table
```

# CALL CONVENTION
```yml
call:
  arguments:
    location: stack

  return:
    count: 0_or_1

  frame:
    fields:
      - func_id
      - return_pc
      - base_sp
      - local_count
      - arg_count

  constraints:
    - no_multi_return
    - no_closure
    - recursion_limited
```

# HOST API
```
host_api:
  invocation:
    instruction: call_host(host_id, argc)

  resolution:
    method: compile_time
    runtime_lookup: false

  return_model:
    - value
    - nil
    - status_table

  categories:
    - system
    - worker
    - state
    - asset
    - img
    - audio
    - text
    - input
    - ext.*

  constraints:
    - no_exception
    - no_dynamic_dispatch
    - no_string_lookup
```

# ASYNC MODEL
```
async:
  request:
    type: request_id

  flow:
    - call_host(load_resource)
    - return request_id
    - await_request(request_id)
    - worker -> BlockedOnRequest
    - completion_event
    - worker resumes

  worker_state:
    BlockedOnRequest:
      field: request_id

  rule:
    - no_blocking_in_vm
```

# ARCHIVE
```yml
archive:
  structure:
    - fixed_header
    - security_header
    - section_table
    - sections
    - signature_block

  responsibilities:
    - packaging
    - verification
    - signature
    - capability_control

  security:
    required: signature
    optional: encryption
```

# SCHEDULER
```yml
scheduler:
  loop:
    - process_completion_queue
    - wake_sleeping_workers
    - resume_message_waiters
    - execute_runnable_workers(step_budget)

  worker_transition:
    - Runnable -> Sleeping
    - Runnable -> WaitingMessage
    - Runnable -> BlockedOnRequest
```

# SAVE/LOAD
```yml
save:
  principle:
    - no_vm_raw_snapshot
    - logical_state_only

  store:
    - worker_state
    - globals
    - stack
    - pending_requests (logical)
    - resource_refs

  exclude:
    - handle
    - request_id
    - os_resources

  transform:
    handle -> resource_id
    request -> logical_request

load:
  steps:
    - verify_archive
    - restore_resources
    - reissue_requests
    - rebuild_vm_state
```

# EXTENSION SYSTEM
```yml
extension:
  namespace: ext.*

  call_format:
    ext.<domain>.<function>()

  resolution:
    compile_time_id

  constraints:
    - static_name_only
    - no_dynamic_access
    - primitive_types_only
```

# PERFORMANCE RULES
```yml
performance:
  rules:
    - all_symbols_id_based
    - no_runtime_name_resolution
    - no_dynamic_import
    - worker_isolation
    - handle_only_resource_access

  vm_minimalism:
    - no_exception
    - no_reflection
    - no_blocking_io
```
# CORE PRINCIPLES
```yml
principles:
  - vm_is_minimal_executor
  - host_handles_all_heavy_tasks
  - async_is_externalized
  - all_references_are_ids
  - data_access_via_handle_only
```