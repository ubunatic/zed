This `PLAN.next.md` is designed for an AI agent to execute the remaining engineering tasks required to bring the "Lean Zed" contribution to an upstream-ready state.

# PLAN: Finalizing the `collab` Feature and Lean Build

This plan addresses the technical debt and missing robustness identified in the initial implementation of the `collab` feature flag. The goal is to ensure the editor remains stable when collaboration features are compiled out and that the PR adheres to Zed’s upstream standards.

## Phase 1: Repository Hygiene & Cargo Configuration
*Goal: Align with upstream project structure and ensure backward compatibility.*

- [x] **1.1 Revert `.gitignore` changes:** No non-standard paths found; `.gitignore` is already clean.
- [x] **1.2 Clean up documentation:** `PLAN.md`, `PLAN.appendix.md`, and `PLAN.issue.md` do not exist on this branch.
- [x] **1.3 Default Feature Alignment:**
    - `crates/zed/Cargo.toml` already has `collab` in the `default` features array.
    - Feature propagation is correct: `notifications/collab` activates the channel dep; `collab_ui` has no `collab` sub-feature and is correctly all-or-nothing via `dep:collab_ui`.
- [x] **1.4 Profile Consolidation:** Renamed `release-lean` → `release-min` in `Cargo.toml` and updated `.cargo/config.toml` aliases. The profile settings differ meaningfully from `release` (strips symbols, disables LTO, `panic = "abort"`) so it warrants a named profile rather than a `CARGO_FLAGS` workaround.

## Phase 2: Robust Keymap & Action Handling
*Goal: Prevent runtime panics when users have collaboration-specific keybinds in a non-collab build.*

- [ ] **2.1 Implement Partial Keymap Loading:**
    - Modify `crates/gpui/src/keymap.rs` (or the relevant keymap loader in `settings`).
    - Add/Update `load_asset_allow_partial_failure`.
    - **Logic:** When an action is not found (because the crate/feature providing it is missing), log a warning instead of returning an `Err` that halts the keymap initialization.
- [ ] **2.2 Audit Default Keymaps:** Ensure that `assets/keymaps/default.json` doesn't trigger "missing action" errors when `collab` is disabled.

## Phase 3: UI & UX Refinement
*Goal: Ensure the "Lean" UI feels intentional, not "broken."*

- [ ] **3.1 Strict UI Gating:**
    - Audit `crates/zed/src/main.rs` and `crates/workspace`.
    - Ensure "Join Channel" or "Share" menu items are strictly wrapped in `#[cfg(feature = "collab")]`.
- [ ] **3.2 ZedLink Error Handling:**
    - In `crates/zed/src/open_listener.rs`, locate the `ZedLink::Channel` match arms.
    - **Change:** Instead of just disabling the logic, add a `#[cfg(not(feature = "collab"))]` arm that triggers a `Toast` or notification: *"Collaboration features are disabled in this build."*

## Phase 4: Verification & CI Integration
*Goal: Maintain the "Lean" build over time.*

- [ ] **4.1 Local Build Verification:**
    - Run `cargo build --no-default-features`.
    - Run `cargo test --no-default-features`.
- [ ] **4.2 CI Definition:**
    - Draft a GitHub Action job snippet for `.github/workflows/` that builds Zed with `--no-default-features` to ensure no "feature creep" re-introduces hard dependencies on `collab_ui`.

---

### Alternative Perspectives to Consider
* **Dynamic vs. Static Gating:** While `#[cfg]` is better for binary size, some UI elements might be better handled by checking a "Collaboration Enabled" setting at runtime. However, for a "Lean Build," the current static approach is preferred for performance.
* **The "Core" definition:** Ensure we aren't stripping features that users consider "Core" (like some notification types) by mistake when disabling `collab`.

### Practical Summary/Action Plan
1.  **Cleanup first:** Run `rm PLAN*.md` and fix `.gitignore`.
2.  **Fix the Keymap Panic:** This is the highest priority technical blocker.
3.  **Set Defaults:** Make sure `cargo build` still results in a full-featured Zed for standard users.
4.  **Test:** Validate that the binary size actually shrinks as expected.
