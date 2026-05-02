# Upstreaming `editor: Paste image into Markdown file as asset link`

What needs to happen before this PR can be sent to `zed-industries/zed`.

The current PR (#14 on `ubunatic/zed`) is a development branch on a fork. It mixes the feature with a large amount of unrelated fork-only tooling. To upstream it, the feature needs to be extracted onto a clean branch and brought up to Zed's contribution standards.

---

## 1. Process gates (CONTRIBUTING.md)

- [ ] **Sign the CLA** — https://zed.dev/cla. Required before any PR can be merged.
- [ ] **Confirm the feature is wanted.** Per CONTRIBUTING.md, features should be
      confirmed before sending a PR: *"If there isn't already a GitHub issue for
      your feature with staff confirmation that we want it, start with a GitHub
      discussion rather than a PR."* This is a new feature (not a bugfix or doc
      change), so open a Discussion first describing:
      - The problem: pasting screenshots / images into Markdown notes is awkward today.
      - The proposed UX: clipboard image → save under `assets/` next to the doc → insert `![name](assets/file)`.
      - Scope: Markdown buffers backed by a saved local file only.
      - Open questions: alt text default, where `assets/` lives (next to file? workspace root?), behavior for non-Markdown buffers, behavior when an image with the same name already exists.
- [ ] Wait for staff signal before opening the PR. Reference the Discussion / Issue from the PR body.

## 2. Branch hygiene — extract just the feature

The current branch has 43 files changed and ~15k additions. **Only three files belong to this feature** (plus `Cargo.lock`):

**Keep:**
- `crates/editor/Cargo.toml` — adds the `image` dependency
- `crates/editor/src/editor.rs` — the paste implementation
- `crates/editor/src/editor_tests.rs` — five tests
- `Cargo.lock` — locked transitive deps for `image`
- `Cargo.toml` (workspace) — only if `image` was added to the workspace deps table

**Drop everything else.** Examples of what must NOT go upstream:
- Fork-only tooling: `HARNESS.md`, `PLAN.editor.md`, `PLAN.next.md`, `REBASE.md`, `REVIEW.md`, `ROUTINE.yaml`, `UPSTREAM.md` (this file)
- Build/CI tweaks: `.cargo/config.toml` aliases (`build-quiet`, `build-lean`), `.github/workflows/lean_build_check.yml`, `.github/workflows/assign-reviewers.yml` rewrite, `.gitignore` changes
- Unrelated crate changes: `agent_ui`, `collab`, `collab_ui`, `notifications`, `search`, `settings/keymap_file.rs`, `title_bar`, `vim/test`, `zed/*` — these are all from the "make collab optional" / fork-management work
- Helper scripts: `script/bg-check`, `script/bundle-mac-min`, `script/lean-check`, `script/plan-audit`, `script/rebase-onto-main`, `script/sync-rerere`
- Generated artifacts: anything under `*postimage*` / `*preimage*`, plan-audit outputs

**How to do it cleanly:**
```
git fetch upstream
git switch -c upstream-image-paste upstream/main
git checkout editor-copy-paste-image -- crates/editor/Cargo.toml crates/editor/src/editor.rs crates/editor/src/editor_tests.rs
# review, then handle Cargo.lock / workspace Cargo.toml manually
```
Squash to a single well-described commit, or two at most (feature + tests) if the diff is large.

## 3. Code issues to fix (from review on PR #14)

- [ ] **Move blocking I/O off the foreground thread.** `load_external_image_from_path` calls `std::fs::read` synchronously inside `paste`, which runs on the UI thread. Read the file inside the existing `cx.background_spawn` block.
- [ ] **Fix multi-image paste positioning.** When `ExternalPaths` contains several images, every async `paste_image` task resolves at the *same* cursor position, so links stack on top of each other rather than appearing in sequence. Either serialize the writes in a single background task and emit one combined `do_paste`, or restrict to a single image per paste and document that.
- [ ] **Bound the filename retry loop.** `next_available_image_filename` loops without an upper limit. Cap it (e.g. 9999) and return an error from the background task on overflow — per CLAUDE.md, prefer `?` over potentially-unbounded operations.
- [ ] **Remove what-comments in `paste`.** `// Priority 1: string entry`, `// Priority 2: image entry`, `// Priority 3: external file paths`, and `// Fallback: paste whatever text is available` describe what the code already says. Project rule: comments only for non-obvious *why*.
- [ ] **Reconsider alt text.** External-path alt text is currently the full filename including extension (`![photo.png](assets/photo.png)`). Drop the extension so it reads `![photo](assets/photo.png)`. Update the test accordingly.
- [ ] **Verify `image` crate version policy.** Check whether `image` is already a workspace dep elsewhere (it likely is, given the project size). If so, depend on it via `workspace = true` in `crates/editor/Cargo.toml` instead of pinning a fresh version.

## 4. Convention compliance (CLAUDE.md + CONTRIBUTING.md)

- [x] PR title format: `editor: Paste image into Markdown file as asset link` — imperative, crate-prefixed, no conventional-commit prefix, no trailing punctuation.
- [x] `Release Notes:` section present with a single user-facing bullet.
- [x] Tests included — five `#[gpui::test]`s covering the documented happy paths and no-op cases.
- [ ] Run `./script/clippy` and ensure it's clean on the extracted branch.
- [ ] Run the editor crate tests (`cargo test -p editor -j 4`) on the extracted branch.
- [ ] No `unwrap()` / silently-ignored `Result` introduced in production code paths. (Tests are fine.)
- [ ] Confirm no new `mod.rs` files; no `lib.rs` if a new crate is added (n/a here).

## 5. PR description

When you open the upstream PR, the body should contain:

- **Motivation** — one short paragraph describing the user pain point.
- **Behavior** — what's pasted, where it's saved, when it's a no-op (non-Markdown, unsaved buffer, no image in clipboard).
- **Demo** — a short screen recording or GIF showing: copy screenshot → paste into a `.md` file → asset appears under `assets/` and link is inserted. CONTRIBUTING.md explicitly asks for this for UI changes.
- **Test plan** — keep the existing checklist, but mark every box (the second one is already covered by `test_paste_image_path_into_markdown_inserts_link`).
- **Link to the Discussion** from step 1.
- **Release Notes:** unchanged from current PR.

## 6. Things that may still get pushed back

CONTRIBUTING.md lists what they tend not to merge. Anticipate the following questions:

- *"Why isn't this an extension?"* — Image paste touches `Editor::paste` directly and needs clipboard + buffer + filesystem access at the right moment; it's not feasible as an extension today. Be ready to say so.
- *"Markdown-only feels narrow."* — Justify by pointing to the natural Markdown image-link syntax and the fact that other languages don't have an equivalent. Or extend to AsciiDoc/Org if the maintainers want it broader.
- *"Where do `assets/` go in a multi-root workspace?"* — The current design uses the document's own directory. Confirm that's acceptable, or make it configurable via a setting (`editor.image_paste.assets_dir`).
- *"Do existing files get overwritten?"* — No, the filename loop ensures unique names. Make this explicit in the PR description.

## 7. Is it worth it?

Honestly — probably not, in its current form.

**Against:**
- CONTRIBUTING.md says they merge ~50% of PRs, and explicitly lists *"features where the extra complexity isn't worth it for the number of people who will benefit"* as something they don't merge. Image-paste-to-Markdown is a niche workflow: people who take notes in Markdown inside Zed *and* paste screenshots regularly.
- Markdown-only scope is narrow and somewhat arbitrary — they'll likely ask "why not all buffers?" or "why not configurable?", which expands scope.
- The `assets/` directory placement is a design call that needs maintainer buy-in (next to file? workspace root? configurable setting?). Without that conversation up front, the PR risks rewrites.
- Real cost ahead: clean rebase, fix the foreground-I/O and multi-image bugs, write a Discussion, record a demo, sign the CLA, then likely iterate on review feedback. Realistically a few more focused hours.

**For:**
- It's a genuinely useful feature (Obsidian, Typora, VS Code's Markdown extension all do this — there's prior art).
- The implementation is small and self-contained in `Editor::paste`.
- You already have it working and tested for your own use.

**Recommendation:** keep it on the fork as a personal patch. If you want to upstream it, **don't start with a PR — start with a Discussion** ("would Zed accept image-paste-to-Markdown, and if so what's the right scope?"). That's a 10-minute investment that tells you whether the rest is worth doing. If they say yes with a clear scope, the work is well-defined; if they say no or "make it an extension," you've saved a day.

## 8. Final pre-submit checklist

- [ ] CLA signed.
- [ ] Discussion opened and staff has indicated interest.
- [ ] Clean branch off `upstream/main` containing only the feature.
- [ ] All four code issues from §3 fixed.
- [ ] `./script/clippy` clean.
- [ ] `cargo test -p editor -j 4` green.
- [ ] Manual test on a real Markdown file (clipboard image *and* file path).
- [ ] Demo recording attached.
- [ ] PR body links the Discussion and contains the Release Notes section.
