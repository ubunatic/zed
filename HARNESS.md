# Agentic Harness Report

This file tracks session-level observations about token efficiency, tooling friction, and workflow
improvements. Each session appends a dated entry. The goal is to make future sessions faster and
cheaper.

ROUTINE.yaml should instruct agents to append a new entry here as part of the `cleanup` step.

---

## Session: 2026-04-27 — Keymap partial-load warning (PR #6)

### Task completed
Added `log::warn!` to `load_asset_allow_partial_failure` in `crates/settings/src/keymap_file.rs`
so unresolvable keybindings in built-in asset keymaps are visible in logs. PLAN items 2.1, 2.2,
and 3.1 marked done after codebase audit.

---

### Token burn analysis

| Area | Rough cost | Notes |
|------|-----------|-------|
| ToolSearch (deferred tools) | Medium | 6 `ToolSearch` calls needed before MCP/TodoWrite tools were usable. Each round-trip adds latency and tokens. |
| Explore sub-agent (audit) | Medium-high | Valuable parallelization — cheaper than doing 3 separate grep sequences in main thread. Justified. |
| Cargo check — two attempts | Medium | First background job wrote to an empty output file; a second `cargo check` was launched. The Monitor `until` loop then timed out because the file-readiness check raced with the job. Two full compilations ran instead of one. |
| Branch switch confusion | Low-medium | Switching from feature branch back to `develop` caused Git to revert PLAN.next.md and REVIEW.md in the working tree. System reminders flagged the "modification", requiring re-application of the doc changes via `git checkout <branch> -- <files>`. This added ~4 extra tool calls. |
| Sequential grep exploration | Low | After the Explore agent returned, several follow-up `grep` + `Read` calls were made in the main thread to verify specific details (title_bar Cargo.toml, keymap_file.rs load function). Some of these could have been bundled into the initial Explore prompt. |

**Biggest avoidable cost:** the double cargo check. The background task output file was empty on
first read because the job had not started writing yet. The retry triggered a second full
compilation. Total wasted compile time: ~60s.

---

### What worked well

- **Explore sub-agent** for the initial audit was token-efficient: one agent call replaced ~6
  sequential grep/read sequences in the main thread.
- **Parallel tool calls** (e.g. reading PLAN.next.md and REVIEW.md simultaneously) kept latency
  low.
- **Cargo check scoped to one crate** (`-p settings`) was fast (~15s). Using the full workspace
  would have been much more expensive.
- **Doc-only commit directly to develop** avoided a separate PR just for markdown updates.

---

### Ideas for improvement

#### 1. Pre-warm deferred tools in startup ✅ Done
ROUTINE.yaml currently has no mention of deferred MCP tools. The first 3–4 tool calls of every
session are `ToolSearch` lookups for `TodoWrite`, `Monitor`, `mcp__github__*`, etc. A startup
note listing the commonly needed tool names would let the agent batch-load them in one call.

**Suggestion:** add a `tools:` section to ROUTINE.yaml listing tool names that are always needed,
so the agent loads them upfront in a single `ToolSearch select:A,B,C` call.

#### 2. Avoid double cargo check via a sentinel file ✅ Done
Background tasks write output to a temp file, but the file may not exist yet when the Monitor
`until` loop starts. Use a wrapper that creates the file before running the command:

```sh
# script/bg-check (proposed)
set -e
out=$(mktemp)
echo "RUNNING" > "$out"
cargo check -p "$1" -q 2>&1 | tee "$out"
echo "DONE:$?" >> "$out"
```

Then the `until` loop can reliably detect both "still running" and "finished" states.

#### 3. Commit docs and code in the same feature branch commit ✅ Done
The current cleanup pattern commits PLAN/REVIEW doc updates directly to `develop`. When the
feature branch also modifies those files, Git's branch switch reverts them, requiring re-applying
via `git checkout <branch> -- <files>`. This is fragile.

**Suggestion:** keep doc and code changes in the same feature branch commit. On merge, the PR
lands everything together. The `develop` branch stays at the merge commit rather than receiving a
separate doc-only push. Update the `cleanup` step in ROUTINE.yaml accordingly.

#### 4. Encode the "audit first" pattern more explicitly ✅ Done (via abort_criteria + plan-audit script)
The PLAN.next.md audit step (check if unchecked items are already done before picking a task) is
mentioned in ROUTINE.yaml but not emphasized enough. Agents sometimes skip it and pick a task
without checking. The audit in this session revealed that 2.1, 2.2, and 3.1 were largely done,
saving a wasted implementation attempt.

**Suggestion:** add an `abort_if_all_done` check to ROUTINE.yaml that explicitly says: after
marking already-done items, if no simple task remains, do the doc-only commit and stop — do not
force a coding task.

#### 5. Explore agent prompt should bundle related questions ✅ Done (noted in ROUTINE.yaml agentic_workflow)
The initial Explore agent call asked three separate questions (2.1, 2.2, 3.1). This worked well,
but the follow-up verification calls in the main thread (title_bar Cargo.toml, keymap_file.rs
details) could have been part of the original prompt. When doing a pre-task audit, include all
files mentioned in the PLAN items in one agent call.

---

### Suggestions for MD file updates

#### ROUTINE.yaml
- ✅ Add a `tools:` section listing commonly deferred tools (TodoWrite, Monitor,
  `mcp__github__list_pull_requests`, `mcp__github__create_pull_request`, etc.) so the agent
  loads them in one batch at session start.
- ✅ Change the `cleanup` step to commit PLAN/REVIEW changes on the feature branch rather than
  directly to `develop`, so branch switching doesn't cause working-tree confusion.
- ✅ Add a step: "Append a dated entry to HARNESS.md summarizing token burn, friction points, and
  any new script ideas. Include this file in the same PR as the coding change."
- ✅ Clarify that `post_rebase.branch naming` should reuse the session's designated branch
  (`claude/sharp-shannon-PMlMo` in this case) rather than creating a new one, to avoid orphaned
  branches.

#### PLAN.next.md
- ✅ Add a "Known gaps / out of scope" section so future agents don't re-audit the same already-
  noted issues (e.g. `crates/title_bar` unconditional `call`/`channel` deps). This avoids
  duplicating audit work across sessions.
- ✅ After all items in a phase are done, add a phase-level `✅ Phase N complete` marker so the
  agent can skip re-reading that phase's items.

#### REVIEW.md
- ✅ Add a "Last audited" date field at the top so it's clear when the review was last updated and
  agents know whether to re-audit or trust it.

---

### Suggestions for new scripts

#### `script/lean-check` ✅ Done
Runs `cargo check --no-default-features` to verify the lean (no-collab) build compiles. Should
be idempotent and fast (~30s on warm cache).

```sh
#!/usr/bin/env bash
set -euo pipefail
echo "Checking lean build (--no-default-features)..."
cargo check -p zed --no-default-features -q
echo "Lean build OK."
```

#### `script/plan-audit` ✅ Done
Prints all unchecked `[ ]` items from PLAN.*.md files so agents get a quick summary without
reading every file. Useful as the first step of each session.

```sh
#!/usr/bin/env bash
set -euo pipefail
grep -rn '^\- \[ \]' PLAN.*.md 2>/dev/null || echo "No unchecked items found."
```

#### `script/bg-check` ✅ Done
Wraps `cargo check` for use as a background task with a reliable sentinel, avoiding the
"empty output file" race condition described above.

```sh
#!/usr/bin/env bash
# Usage: script/bg-check <crate> [extra cargo flags...]
set -euo pipefail
crate="${1:?Usage: bg-check <crate>}"
shift
out_file="${BG_CHECK_OUT:-/tmp/bg-check-$crate.out}"
echo "RUNNING" > "$out_file"
cargo check -p "$crate" "$@" >> "$out_file" 2>&1
echo "EXIT:$?" >> "$out_file"
```

Then in the Monitor loop: `until grep -q '^EXIT:' "$out_file"; do sleep 1; done`

---

### Notes for next session

- Remaining open PLAN items: **4.1** (local lean build verification) and **4.2** (CI snippet).
  4.1 can be done with `script/lean-check` above (or `cargo check --no-default-features`).
  4.2 is a doc/yaml authoring task with no compilation needed.
- The `crates/title_bar` unconditional dependency on `call`/`channel`/`livekit_client` is a
  known gap not yet addressed. Gating those would require adding a `collab` feature to
  `title_bar`'s Cargo.toml and wrapping the relevant render methods.

---

## Session: 2026-04-27 — Harness improvements (PR #8)

### Task completed
Implemented all improvement suggestions from the previous HARNESS.md session entry:
- Updated `ROUTINE.yaml`: added `tools:` preload section, fixed cleanup to commit on feature branch,
  added HARNESS.md update step, clarified branch reuse in `post_rebase`.
- Updated `PLAN.next.md`: added `✅ Phase N complete` markers for phases 1–3, added "Known Gaps /
  Out of Scope" section to prevent re-auditing deferred issues each session.
- Updated `REVIEW.md`: added *Last audited* date field at top.
- Created three new scripts: `script/lean-check`, `script/plan-audit`, `script/bg-check`.

---

### Token burn analysis

| Area | Rough cost | Notes |
|------|-----------|-------|
| ToolSearch (deferred tools) | Low | Only 1 batch load needed (ExitPlanMode schema). Pre-warm section in ROUTINE.yaml should eliminate this for future sessions. |
| Explore sub-agent | Low | One agent call read ROUTINE.yaml, PLAN.next.md, REVIEW.md, and script/ listing in parallel. |
| Plan mode | Low | Plan file written cleanly in one pass; no iteration needed. |
| File edits | Low | 6 edits + 3 writes, all targeted. No compile step. |

**No significant waste this session.** Doc-only task with clear scope.

---

### What worked well

- **plan-audit script** confirms itself immediately: running `script/plan-audit` after editing
  PLAN.next.md showed exactly the two expected unchecked items (4.1, 4.2).
- **Parallel agent reads** in the Explore call retrieved all three source files at once.
- **No compilation** needed — doc+script task completed end-to-end without cargo.

---

### Ideas for improvement

None new this session. All previously identified improvements are now implemented.

---

### Notes for next session

- Remaining open PLAN items: **4.1** (lean build) and **4.2** (CI snippet). Use `script/lean-check`
  for 4.1. Both are now clearly described in PLAN.next.md Phase 4.
- `script/bg-check` is ready to use for any future background cargo checks — replace raw
  `cargo check ... &` calls in sub-agents with `script/bg-check <crate>` + Monitor until-loop.
- The `tools:` preload section in ROUTINE.yaml should be tested in the next session to confirm
  it actually reduces ToolSearch round-trips at startup.
- **`gh` default repo**: `gh pr create` targets `upstream` (zed-industries/zed) by default.
  Run `gh repo set-default ubunatic/zed` once per clone to point all `gh` commands at the fork.
  This is stored in `.git/config` and does not affect `git remote` or `git fetch upstream`.

---

## Session: 2026-05-01 — Lean build CI workflow (PLAN 4.2)

### Task completed
Created `.github/workflows/lean_build_check.yml`: a GitHub Actions workflow that runs
`./script/lean-check` (`cargo check -p zed --no-default-features`) on every PR and push to
`main`/`develop`. Marked PLAN item 4.2 done. Updated REVIEW.md last-audited date and CI section.

---

### Token burn analysis

| Area | Rough cost | Notes |
|------|-----------|-------|
| ToolSearch (deferred tools) | Low | 2 batch loads (TodoWrite, list_pull_requests). `tools:` preload section in ROUTINE.yaml worked — fewer round-trips than prior sessions. |
| GitHub MCP reads | Low | Parallel reads for PLAN.next.md, HARNESS.md, workflow listing, and run_tests.yml reference. All resolved in 2 round-trips. |
| File writes/edits | Low | 1 new file + 4 targeted edits. No compilation needed. |

**No significant waste.** Pure doc+YAML task with a clear scope and no cargo invocations.

---

### What worked well

- **`tools:` preload section** in ROUTINE.yaml reduced ToolSearch calls from ~6 to 2 this session.
- **Parallel MCP fetches** (PLAN.next.md + HARNESS.md + workflow listing simultaneously) kept latency low.
- **No open PRs check** upfront caught that the branch was clean before picking a task.
- **Doc + code in same branch commit** (per improved ROUTINE.yaml) avoided the branch-switch confusion documented in session 2026-04-27.

---

### Ideas for improvement

None new this session. All previously identified improvements are in place and working.

---

### Notes for next session

- Only remaining open PLAN item: **4.1** (local lean build verification — run `script/lean-check` in a full Linux build env). This requires system deps (`./script/linux`) and a warm cargo cache; best done on a machine with a full Zed build set up.
- Phase 4 is effectively complete once 4.1 is validated. At that point, all PLAN.next.md items will be done and the branch is ready for upstream PR prep.
- The `crates/title_bar` collab dep gap (screen-share/collaborator UI ungated) remains as noted in Known Gaps.
