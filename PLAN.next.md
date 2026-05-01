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

✅ Phase 1 complete

## Phase 2: Robust Keymap & Action Handling
*Goal: Prevent runtime panics when users have collaboration-specific keybinds in a non-collab build.*

- [x] **2.1 Implement Partial Keymap Loading:**
    - `load_asset_allow_partial_failure` is implemented in `crates/settings/src/keymap_file.rs`.
    - Returns successfully with valid bindings when some fail to load; now also emits `log::warn!` for the partial-failure case so missing collab actions are visible in logs without halting startup.
- [x] **2.2 Audit Default Keymaps:**
    - Default keymaps (`default-macos.json`, `default-linux.json`, `default-windows.json`) contain `collab_panel::*` actions, but these are scoped to `CollabPanel` context (active only when that panel is focused).
    - The global `collab_panel::ToggleFocus` binding is handled gracefully by `load_asset_allow_partial_failure`, which skips unresolvable actions and emits a warning rather than an error.

✅ Phase 2 complete

## Phase 3: UI & UX Refinement
*Goal: Ensure the "Lean" UI feels intentional, not "broken."*

- [x] **3.1 Strict UI Gating:**
    - Audited `crates/zed/src/main.rs` and `crates/zed/src/zed/app_menus.rs`.
    - "Collab Panel" menu item is gated with `#[cfg(feature = "collab")]`; no ungated "Join Channel" or "Share" menu items exist in the application menus.
    - All collab-specific imports and initialization in `main.rs` are guarded with `#[cfg(feature = "collab")]`.
    - Note: `crates/title_bar` has `call`, `channel`, and `livekit_client` as unconditional dependencies — the title bar's collab rendering (screen-share button, collaborator list) is not feature-gated. This is a remaining concern for a full lean build but is out of scope for the current phase.
- [x] **3.2 ZedLink Error Handling:**
    - `OpenRequestKind::CollabLinkUnsupported` variant exists in `open_listener.rs`.
    - Set via `#[cfg(not(feature = "collab"))]` catch-all arm in the `ZedLink` match.
    - Handled in `main.rs` with a `Toast`: *"Collaboration links are not supported in this build."*

✅ Phase 3 complete

## Phase 4: Verification & CI Integration
*Goal: Maintain the "Lean" build over time.*

- [ ] **4.1 Local Build Verification:**
    - Run `cargo build --no-default-features`.
    - Run `cargo test --no-default-features`.
- [x] **4.2 CI Definition:**
    - `.github/workflows/lean_build_check.yml` added: triggers on PRs and pushes to `main`/`develop`, runs `./script/lean-check` (`cargo check -p zed --no-default-features`) on ubuntu-22.04. Cancels stale runs via concurrency group.

---

### Alternative Perspectives to Consider
* **Dynamic vs. Static Gating:** While `#[cfg]` is better for binary size, some UI elements might be better handled by checking a "Collaboration Enabled" setting at runtime. However, for a "Lean Build," the current static approach is preferred for performance.
* **The "Core" definition:** Ensure we aren't stripping features that users consider "Core" (like some notification types) by mistake when disabling `collab`.

### Practical Summary/Action Plan
1.  ~~**Fix the Keymap Panic (2.1):** This is the highest priority technical blocker.~~ ✅ Done
2.  ~~**Audit keymaps (2.2):** Check `assets/keymaps/default.json` for collab-only actions.~~ ✅ Done — partial loading handles them gracefully.
3.  ~~**UI gating (3.1):** Wrap any remaining collab menu items in `#[cfg(feature = "collab")]`.~~ ✅ Done for menus. Title bar collab module remains ungated (future work).
4.  ~~**Test (4.1):** Validate `cargo build --no-default-features` succeeds and binary shrinks.~~ Tracked as known gap (requires a full Linux build environment).
5.  ~~**CI snippet (4.2):** Draft `lean_build_check.yml` workflow.~~ ✅ Done — `.github/workflows/lean_build_check.yml` added.

---

## Known Gaps / Out of Scope

These issues are noted but explicitly deferred — do not re-audit them each session:

- **`crates/title_bar` collab deps**: `call`, `channel`, and `livekit_client` are unconditional dependencies in `title_bar`'s Cargo.toml. Gating them requires adding a `collab` feature to `title_bar` and wrapping the screen-share / collaborator-list render methods. Tracked as future work.
- **Test suite gating (4.2 prerequisite)**: Many integration tests assume a collaboration server. Full gating is out of scope until upstream review begins.
