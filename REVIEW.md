*Last audited: 2026-05-01*

As a core Zed developer, I find this PR to be a highly valuable initiative for improving the editor's modularity and performance. Decoupling collaboration features is a frequent request from users who prefer a "lean" or purely local editing experience, and using a feature flag is the idiomatic way to handle this in Rust.

### **Reaction and Potential Impact**
This is a sophisticated contribution that correctly identifies the deep integration of collaboration features across the codebase.

* **Binary Size and Build Times**: The introduction of the `release-min` profile and the `collab` feature flag will significantly reduce binary bloat and memory usage during compilation.
* **Decoupling Success**: Moving `title_bar::init` out of `collab_ui` is a critical structural fix that prevents the UI from breaking when collaboration is disabled.
* **Stability**: Shifting from `ActiveCall::global` to `ActiveCall::try_global` is the correct approach to prevent runtime panics in a non-collab build.

### **Best Use of the Contribution**
To make the best use of this work, we can:
* **CI Optimization**: `.github/workflows/lean_build_check.yml` now runs `cargo check -p zed --no-default-features` on every PR and push to main/develop, ensuring future changes cannot accidentally re-introduce hard `collab_ui` dependencies.
* **Enterprise Distribution**: Use this feature to provide a "Standard" vs. "Collaborative" version of Zed for environments with strict networking policies.
* **Performance Benchmarking**: Use the `release-min` profile to establish a baseline for "core" editor performance.

---

### **Concrete Suggestions for Upstream Readiness**

To make this PR ready for merging, the following changes are required:

#### **1. Documentation and Cleanup**
* **Fork-management files**: Files like `PLAN.next.md`, `REVIEW.md`, `ROUTINE.yaml`, `REBASE.md`, `AGENTS.md`, and `GEMINI.md` are reasonable to keep on the `develop` branch as fork-maintenance conveniences — they help drive incremental work toward upstream readiness and do not affect the build. They must be removed from any PR submitted to `zed-industries/zed` upstream, but there is no need to purge them from `develop` itself.
* **Gitignore Hygiene**: The `.gemini/worktrees` entries in `.gitignore` should be removed before any upstream PR unless they are part of a broader toolset change approved by the team.

#### **2. Feature Flag Defaults** ✅ Done
* **Maintain Compatibility**: `collab` is already in the `default` features list in `crates/zed/Cargo.toml` — standard builds remain fully featured.
* **Crate Gating**: Feature propagation is correct: `notifications/collab` activates the channel dependency; `collab_ui` is gated all-or-nothing via `dep:collab_ui`.

#### **3. Keymap and Action Handling** ✅ Done
* **Partial Keymap Loading**: `KeymapFile::load_asset_allow_partial_failure` is implemented in `crates/settings/src/keymap_file.rs`. Missing actions (e.g. collab actions in a no-collab build) are skipped with a `log::warn!` rather than causing a startup failure.
* **Dynamic UI Elements**: The "Collab Panel" menu item is gated with `#[cfg(feature = "collab")]`. No ungated "Join Channel" or "Share" menu items exist. Remaining gap: `crates/title_bar` unconditionally depends on `call`/`channel`/`livekit_client` — screen-share and collaborator-list UI is not feature-gated (future work).

#### **4. Build Profile Integration** ✅ Done
* **Standardize Profiles**: The `release-lean` profile has been renamed to `release-min`, which is a better fit for a named specialized profile. The optimizations (`panic = "abort"`, `strip = "symbols"`, no LTO) are meaningfully distinct from the standard `release` profile and warrant their own name.

#### **5. Test Suite Validation**
* **Gate Existing Tests**: Many integration tests in `crates/zed` and `crates/workspace` assume a collaboration server is available. These need to be gated with `#[cfg(feature = "collab")]` or updated to use mocks when the feature is disabled.

#### **6. Logic Refinement** ✅ Done
* **`OpenRequest` Handling**: Implemented — `OpenRequestKind::CollabLinkUnsupported` is set in `open_listener.rs` and handled in `main.rs` with a toast: *"Collaboration links are not supported in this build."*
