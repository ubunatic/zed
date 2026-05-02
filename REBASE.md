# Rebasing develop onto main

This branch is a long-lived fork tracking `zed-industries/zed:main`. It may not
merge for months. This document explains how to keep it rebased.

## Quick start

```bash
# First time on a new checkout — enable rerere and load shared resolutions:
git config --global rerere.enabled true
script/sync-rerere load

# Every subsequent rebase:
script/rebase-onto-main

# If conflicts remain after rerere, resolve them manually, then continue:
git add <resolved-files>
git rebase --continue

# After resolving a new conflict, save the resolution so others benefit:
script/sync-rerere save
git add rerere-cache/
git commit -m "rerere: Record resolution for <describe the conflict>"

# Push when done:
git push --force-with-lease origin develop
```

## How the rerere cache works

`rerere` ("reuse recorded resolution") makes Git replay known conflict
resolutions automatically. When a conflict occurs during rebase:

1. Git records the conflict preimage in `.git/rr-cache/<hash>/preimage`
2. You resolve the conflict and run `git add`
3. Git records the resolved postimage in `.git/rr-cache/<hash>/postimage`
4. Next time the same conflict appears, Git applies the postimage automatically

`.git/rr-cache/` is local and not pushed. This repo tracks resolutions in
`rerere-cache/` instead. `script/sync-rerere load` copies them into
`.git/rr-cache/` so rerere can use them. `script/rebase-onto-main` does this
automatically before every rebase.

Enable rerere globally (once, on each machine):

```bash
git config --global rerere.enabled true
```

Check what resolutions are cached: `git rerere status`

## Branch overview

18 commits ahead of `main` as of 2026-04-27. Substantive commits most likely to
conflict during future rebases:

| Commit | File(s) touched | Conflict risk |
|--------|----------------|---------------|
| `Allow partial keymap loading in zed` | `crates/settings/src/keymap_file.rs`, `crates/zed/src/zed.rs` | Resolved 2026-04-27 — rerere cached |
| `make collab optional phase 1: hide UI` | `crates/collab_ui/`, `crates/title_bar/` | Medium — collab_ui is active upstream |
| `fix panics with disabled collab` | scattered | Medium — depends on upstream collab changes |
| `zed: Enable collab feature by default` | `Cargo.toml`, `.gitignore` | Low |
| `zed: Show toast when collab link opened without collab feature` | `crates/zed/src/zed.rs` | Low |

CI / docs / cargo commits are unlikely to conflict.

## Known conflict: `keymap_file.rs` / `zed.rs` (resolved 2026-04-27)

**What happened:** Upstream added a `source: Option<KeybindSource>` parameter to
`KeymapFile::load_asset`. Our branch added `load_asset_allow_partial_failure`
without that parameter. The two changes touched the same call sites.

**Resolution:** Added `source: Option<KeybindSource>` to
`load_asset_allow_partial_failure` and updated all callers to pass `None` (test
helpers that set source manually) or the appropriate `KeybindSource` variant
(`Default`, `Base`) in `load_default_keymap`. This resolution is stored in
`rerere-cache/` and will be applied automatically on future rebases.

## Rebase cadence suggestion

Rebase at least once per two weeks. Upstream moves fast; smaller deltas mean
simpler conflicts. A monthly cadence is the minimum to keep this tractable.
