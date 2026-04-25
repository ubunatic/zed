# Plan to Disable "Collab" Feature

**Status: Partially Implemented (Initial Gating Complete)**

This plan outlines the steps to make collaboration features (calls, channels, collaboration panel) optional in the Zed editor and disable them by default.

## Objective
Introduce a `collab` feature flag in the `zed` crate and associated crates to allow building Zed without collaboration-related dependencies and features. By default, this feature will be disabled.

## Progress
- [x] Initial feature flags and optional dependency setup (zed, notifications).
- [x] Basic code gating in `main.rs`, `zed.rs`, and `open_listener.rs`.
- [x] Initial compilation verification (successfully builds without `collab` feature).
- [ ] Comprehensive audit and gating of runtime access points (global panics, title bar status rendering).
- [ ] Full test suite validation with/without `collab` feature.
- [ ] Audit of `Project` and `Worktree` shared state.

## Proposed Changes

### 1. `notifications` Crate
The `notifications` crate currently mandatorily depends on the `channel` crate, but it also contains the generic `StatusToast` component used by non-collaboration features like `auto_update_ui`.

- **`crates/notifications/Cargo.toml`**:
    - Make `channel` an optional dependency.
    - Add a `collab` feature: `collab = ["dep:channel"]`.
- **`crates/notifications/src/notification_store.rs`**:
    - Wrap `NotificationStore`, its implementation, and the `init` function with `#[cfg(feature = "collab")]`.
- **`crates/notifications/src/notifications.rs`**:
    - Wrap `mod notification_store;` and `pub use notification_store::*;` with `#[cfg(feature = "collab")]`.

### 2. `zed` Crate
- **`crates/zed/Cargo.toml`**:
    - Make `call`, `channel`, and `collab_ui` optional dependencies.
    - Add a `collab` feature: `collab = ["dep:call", "dep:channel", "dep:collab_ui", "notifications/collab"]`.
    - Do NOT add `collab` to the `default` features list.
- **`crates/zed/src/main.rs`**:
    - Wrap collaboration-related imports and initialization calls (`call::init`, `channel::init`, `collab_ui::init`, `notifications::init`) with `#[cfg(feature = "collab")]`.
    - Wrap collaboration-related variants in `OpenRequestKind` and their handling with `#[cfg(feature = "collab")]`.
- **`crates/zed/src/zed.rs`**:
    - Wrap collaboration-related logic (e.g., `CollabPanel` loading and action registration) with `#[cfg(feature = "collab")]`.
- **`crates/zed/src/zed/app_menus.rs`**:
    - Wrap the "Collab Panel" menu item with `#[cfg(feature = "collab")]`.

### 3. Other Affected Areas
- **`crates/auto_update_ui`**:
    - Since `notifications` is still a mandatory dependency but its collab-related parts are now optional, `auto_update_ui` should continue to work using `StatusToast` without requiring the `channel` crate.

## Verification Plan

### Automated Tests
1. **Build without `collab` feature**:
   Run `cargo check -p zed` (ensure no `collab` feature is active). It should compile successfully.
2. **Build with `collab` feature**:
   Run `cargo check -p zed --features collab`. It should compile successfully.
3. **Run existing tests**:
   Ensure that tests that don't rely on collaboration still pass. Tests that DO rely on collaboration may need to be wrapped or run with the feature enabled.

### Manual Verification
1. Launch Zed built without the `collab` feature.
2. Verify that the Collaboration Panel is not present in the UI.
3. Verify that collaboration-related menu items are missing.
4. Verify that non-collaboration features (e.g., auto-update, settings) still work as expected.

---

For additional technical nuances and implementation details, see the [PLAN.appendix.md](./PLAN.appendix.md).
