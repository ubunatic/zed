# Rebase Tooling and Rerere Cache Setup

**Date:** 2026-04-27

---

## What Happened

Set up durable rebase infrastructure for the long-lived `develop` fork of `zed-industries/zed`: rebased onto main, resolved a merge conflict, seeded a shared rerere cache, and wrote helper scripts for future automated and manual rebases.

## Key Learnings

1. **`git rerere` cache can be pre-seeded and committed** — By re-triggering a known conflict in a throw-away worktree with rerere enabled, recording the resolution, and committing `rerere-cache/` to the repo, the resolution becomes available to any future checkout. `script/sync-rerere load/save` bridges the committed cache and `.git/rr-cache/`. This is the right pattern for long-lived forks that rebase frequently.

2. **GPG signing is purely the harness's responsibility** — Scripts and guides must never reference GPG, `commit.gpgsign`, or keychain access. The calling environment (local git config, CI harness) owns signing. Any mention of it in scripts or docs creates confusion and portability problems.

3. **`cargo check --profile` is silently ignored** — `cargo check` always uses debug settings regardless of `--profile`. Only `cargo build` and `cargo test` respect profile flags. Use `cargo check -q` for fast type-checking, `cargo build --profile release-min` when you need to verify the actual release profile compiles.

4. **`[[]]` vs `test` in bash** — For portable, readable shell scripts prefer `if test ...\nthen` over `if [[ ... ]]; then`. The `test` form works in any POSIX shell and the newline-before-`then` style makes the branch structure visually clear.

## Agent Setup Observations

- `script/rebase-onto-main` now handles the full rebase flow including rerere load — the agent's startup block only needs `git config rerere.enabled true` before calling the script.
- The `ROUTINE.yaml` pattern (identity + task + startup + workflow rules) is a clean way to encode agent session configuration alongside the code it operates on.
- `script/bundle-mac-min` fills a gap: the full `bundle-mac` requires codesigning infrastructure; the minimal wrapper is enough for local icon-bearing development builds.

## Open Questions

- [x] Should `script/rebase-onto-main` automatically run `script/sync-rerere save` and commit after a clean rebase? Currently the agent must do this manually after any conflict is resolved. -> save but not commit
- [ ] `ROUTINE.yaml` uses a custom `if: REBASE is true` syntax — depends on the harness interpreting this correctly; worth validating once the first scheduled run fires.
