# Plan Appendix: Technical Nuances for Disabling "Collab"

This appendix provides additional context and technical details discovered during research to assist any agent or engineer implementing the [main plan](./PLAN.md).

## 1. The Dual Notification Systems
A critical distinction must be maintained between two similarly named systems:
- **`crates/notifications`**: This is the "Collab Notification Store". It handles server-side state like contact requests and channel invitations. It depends on `crates/channel`. This is what should be made optional.
- **`crates/workspace/src/notifications.rs`**: This is the general UI notification system (toasts, error prompts, LSP messages). It is **not** collaboration-specific and must remain mandatory for basic editor functionality.

**Note:** `crates/auto_update_ui` uses `StatusToast` from `crates/notifications`. Therefore, the `notifications` crate itself must remain a dependency, but its `NotificationStore` (which pulls in `channel`) should be gated.

## 2. Deep Integration in `OpenRequest`
In `crates/zed/src/zed/open_listener.rs`, the `OpenRequest` struct contains collaboration-specific fields that are not encapsulated within the `OpenRequestKind` enum. These must be gated:
- `open_channel_notes: Vec<(u64, Option<String>)>`
- `join_channel: Option<u64>`

Associated logic in `handle_open_request` and `open_listener.rs` that processes these fields will also require `#[cfg(feature = "collab")]` gating.

## 3. `Project` and `Worktree` Shared State
The `Project` entity in `crates/project` and `Worktree` in `crates/worktree` have extensive "shared" logic (e.g., `Project::shared`, `Worktree::shared`).
- To achieve the primary goal of reducing binary weight and UI clutter, gating the `zed` crate is the priority.
- However, if a complete removal of collaboration code is desired, these low-level crates would eventually need their own internal feature flags. For now, focus on the UI/Application layer.

## 4. The Role of the `client` Crate
The `client` crate handles the connection to Zed's infrastructure. It is used for both:
- **Collaboration**: Rooms, channels, calls.
- **General Services**: Auto-updates, telemetry, LLM token refreshing.

**Recommendation:** Do not attempt to disable the `client` crate. Focus exclusively on the service layers (`call`, `channel`, `collab_ui`) built on top of it.

## 5. Implementation Sequence for `main.rs`
When gating initializations in `main.rs`, ensure that the following calls are wrapped:
- `channel::init`
- `call::init`
- `collab_ui::init`
- `notifications::init` (This specific `init` belongs to the collab store)

## 6. Testing Strategy
Zed's integration tests (especially in `crates/collab`) rely heavily on `TestServer` and multi-client orchestration.
- **Validation**: Use `cargo check -p zed` as the primary sanity check.
- **Gating Tests**: Many tests in `crates/zed` and `crates/workspace` that assume collaboration is present will need to be marked with `#[cfg(feature = "collab")]` or adjusted to handle the absence of these features gracefully.

## 7. Known Risks & Overlooked Issues
- **Global Panics**: Code calling `ActiveCall::global(cx)` in a non-collab build will panic if `call::init` isn't called. Always prefer `ActiveCall::try_global(cx)` if it exists.
- **Title Bar Rendering**: Ensure `crates/title_bar` doesn't attempt to render call status if the `collab` feature is disabled.
- **`git status`**: 
```
On branch worktree-disable-features
Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
        modified:   crates/notifications/Cargo.toml
        modified:   crates/notifications/src/notification_store.rs
        modified:   crates/notifications/src/notifications.rs
        modified:   crates/zed/Cargo.toml
        modified:   crates/zed/src/main.rs
        modified:   crates/zed/src/zed.rs
        modified:   crates/zed/src/zed/app_menus.rs
        modified:   crates/zed/src/zed/open_listener.rs

Untracked files:
  (use "git add <file>..." to include in what will be committed)
        PLAN.appendix.md
        PLAN.md
```
