# AGENTS.md

This repository needs architecture-aware edits.

Read [docs/ARCHITECTURE_REFACTOR.md](docs/ARCHITECTURE_REFACTOR.md) before making substantial app-layer changes.

## Primary Rule

The main architectural problem is excessive responsibility concentrated in:

- `crates/app/src/search/window.rs`

Do not continue growing that file unless the change is strictly about Win32 message entry or minimal bootstrap glue.

## Required Placement Rules

- Search flow changes belong in an orchestration-oriented file, not directly in `window.rs`.
- Plugin registration and plugin result mapping belong in a dedicated plugin bridge/composition file.
- Engine startup and engine event forwarding belong in a dedicated engine bridge file.
- Renderer changes should stay rendering-focused and should not take on search or plugin orchestration.
- State transition helpers should be extracted into app-state-oriented files.

## If You Need To Add New Code

Prefer creating or extending files in these buckets:

- `crates/app/src/search/window/`
- `crates/app/src/search/app_state/`
- `crates/app/src/search/orchestration/`
- `crates/app/src/search/ui/`
- `crates/app/src/search/platform/`

If those folders do not exist yet, create them as part of the refactor instead of adding more logic to the monolith.

## Safe Refactor Order

When touching the app architecture, prefer this sequence:

1. Extract constants, payload types, and pure helpers.
2. Extract plugin router construction and result mapping.
3. Extract debounced search flow and deferred-query management.
4. Extract engine event bridging.
5. Extract state transition helpers.
6. Only then shrink the remaining window procedure.

## Review Standard

Treat these as regressions:

- new business logic added directly to `window.rs`
- renderer gaining engine or router knowledge
- plugin code depending on UI window state
- Win32 callback code performing large multi-step orchestration inline

## Goal

Keep the architecture moving toward small, obvious boundaries so future humans and AI agents can place code correctly on the first try.

## Testing

### easysearch-engine integration tests

`crates/engine/tests/integration.rs` has two layers:

- **Memory layer** (no admin, fast): fake index + `DriveManager::apply` verify search,
  filter, and USN delta (create / delete / rename) logic. Runs everywhere.
- **Real MFT layer** (`#[ignore]`, needs Administrator + real NTFS): starts the real
  engine, indexes the live C: drive (~2M records), creates/deletes/renames real files,
  waits for USN polling, and verifies search results.

Run the memory layer:

```bash
cargo test -p easysearch-engine --test integration
```

Run the real MFT layer (Administrator shell required):

```bash
# IMPORTANT: single-threaded. Each test does a full-drive MFT rebuild + USN poll loop.
# Running them in parallel saturates disk/CPU and makes the USN timing windows flaky
# (create/delete/rename events can miss the wait window and cause false failures).
cargo test -p easysearch-engine --test integration -- --ignored --test-threads=1
```

Each real-MFT test calls `engine.shutdown()` at the end so its background USN poller
thread exits and does not contend with the next test. Do not remove those calls.
