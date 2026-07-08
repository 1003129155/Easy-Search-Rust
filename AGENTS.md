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
