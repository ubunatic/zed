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

**CRITICAL:** `title_bar::init(cx)` must **NOT** be wrapped. It was previously nested in `collab_ui::init`, which caused the title bar to vanish when the `collab` feature was disabled. It has been moved to a mandatory initialization path.

## 6. Testing Strategy
Zed's integration tests (especially in `crates/collab`) rely heavily on `TestServer` and multi-client orchestration.
- **Validation**: Use `cargo check -p zed` as the primary sanity check.
- **Gating Tests**: Many tests in `crates/zed` and `crates/workspace` that assume collaboration is present will need to be marked with `#[cfg(feature = "collab")]` or adjusted to handle the absence of these features gracefully.

## 7. Known Risks & Overlooked Issues
- **Global Panics**: Code calling `ActiveCall::global(cx)` in a non-collab build will panic if `call::init` isn't called. Always prefer `ActiveCall::try_global(cx)` if it exists.
- **Title Bar Initialization**: Decoupling `title_bar::init` from `collab_ui` is essential for features to be disabled safely.

## 8. Preventing Keymap Panics
When a feature like `collab` is disabled, actions registered by that feature (e.g., `collab_panel::ToggleFocus`) will be missing from the `Action` registry. Default keymaps that reference these actions would normally cause a panic on startup if `unwrap()` is used on the result of `KeymapFile::load_asset`.
- **Solution:** Use `KeymapFile::load_asset_allow_partial_failure(path, cx)` instead of `load_asset`. This method allows the keymap to load even if some actions are unknown, preventing a fatal crash while allowing the rest of the keymap to function.

## 9. `visual_test_runner` Compatibility
The `zed_visual_test_runner` binary (in `crates/zed/src/visual_test_runner.rs`) performs a full subsystem initialization. Collab-specific initializers like `call::init` must be gated here as well:
```rust
#[cfg(feature = "collab")]
call::init(app_state.client.clone(), app_state.user_store.clone(), cx);
```
Failure to do so will result in compilation errors when building visual tests with the `collab` feature disabled.

## 10. Handling `ActiveCall` in TitleBar
When disabling collaboration features, the `TitleBar` will panic if it attempts to access the global `ActiveCall` which is initialized only when collaboration features are enabled.
- **Solution:** Replace all calls to `ActiveCall::global(cx)` with `ActiveCall::try_global(cx)` within the `TitleBar` and any related collaboration UI code. 
- **Graceful Handling:** Always wrap the result of `try_global` in a check or an `if let Some(active_call)` block to safely handle cases where the global active call is absent. Do not assume it is always available.
