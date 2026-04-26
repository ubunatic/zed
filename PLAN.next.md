This `PLAN.next.md` is designed for an AI agent to execute the remaining engineering tasks required to bring the "Lean Zed" contribution to an upstream-ready state.

# PLAN: Finalizing the `collab` Feature and Lean Build

This plan addresses the technical debt and missing robustness identified in the initial implementation of the `collab` feature flag. The goal is to ensure the editor remains stable when collaboration features are compiled out and that the PR adheres to Zed’s upstream standards.

## Phase 1: Repository Hygiene & Cargo Configuration
*Goal: Align with upstream project structure and ensure backward compatibility.*

- [ ] **1.1 Revert `.gitignore` changes:** Remove `.gemini/worktrees` and any other non-standard local environment paths.
- [ ] **1.2 Clean up documentation:** Delete `PLAN.md`, `PLAN.appendix.md`, and `PLAN.issue.md`. (These should be moved to the PR description/GitHub issue).
- [ ] **1.3 Default Feature Alignment:**
    - Update `crates/zed/Cargo.toml` to include `collab` in the `default` features array.
    - Ensure feature propagation: `collab = ["collab_ui/collab", "notifications/collab"]` (verify crate-level dependencies).
- [ ] **1.4 Profile Consolidation:** Review the `release-lean` profile in `.cargo/config.toml`. If it strictly replicates `release` with fewer features, consider recommending a `CARGO_FLAGS` approach for CI instead of adding a new named profile to the core config.

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
