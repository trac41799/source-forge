# AGENTS.md — repository working rules

These rules apply to **every agent harness** (opencode, Claude Code, Codex, or any other
automation) that makes changes to this repository. They are non-negotiable.

## 1. Never push to `main` directly

All changes land through a **branch + pull request**. `main` is protected by process, not
just by convention:

1. Create/switch to a dedicated branch off the latest `origin/main`.
2. Commit there; open a PR.
3. Merge only after review (below).

Do not commit, amend, or push to `main`. Do not `git push` any branch other than your own
feature branch. Do not force-push a shared branch; `git push --force-with-lease` is permitted
**only** on your own open PR branch, to update it after a rebase onto `origin/main`.

## 2. Use a git worktree per batch of work

Do not pile work onto the primary checkout. For each batch:

```bash
git fetch origin
git worktree add .worktrees/<name> -b <type>/<name> origin/main
```

- `.worktrees/` is gitignored; use it for all linked worktrees (see `.worktrees/wave-A`,
  `.worktrees/decision-r7`).
- The primary checkout stays on a clean, up-to-date `main`.
- A second worktree has no `node_modules`; create a junction to the primary checkout's
  (`New-Item -ItemType Junction -Path .worktrees/<name>/node_modules -Target <primary>/node_modules`).
- Remove the worktree once the PR is merged: `git worktree remove .worktrees/<name>`.

## 3. Required flow for every change

1. **Branch + worktree** (rule 2).
2. **Implement** with tests (TDD: failing test first, then make it pass).
3. **Spawn an independent adversarial reviewer** over the diff. Required.
   - Review `origin/main...HEAD` (all changed files), looking for real defects: correctness,
     concurrency, security, migration safety, test quality, and overstated/misleading claims.
   - The reviewer must cite `file:line` and a concrete trigger, and must return a verdict
     (`clean` / `clean-with-nits` / `findings`).
4. **Fix until factually clean** — every finding is either fixed or explicitly accepted with a
   written rationale. Do not merge with unaddressed findings.
5. **Run all gates and paste the results:**
   - Rust (MSVC): `$env:Path = "C:\Users\mrtra\tools\mingw64\bin;$env:Path"; cargo +stable-x86_64-pc-windows-msvc test`
   - Frontend: `npm test` (includes the IPC contract test), `npx tsc --noEmit`, `npm run lint`
   - Daemon: `python -m pytest local-daemon/tests/test_router.py`
6. **Push the branch and open/update the PR.**
7. **Merge** only when review is clean and all gates pass.

## 4. Honesty of documentation

Docs, commit messages, and PR descriptions must match the code. If a risk is only partially
mitigated, say so. Every "FIXED"/"RESOLVED" claim must be backed by code and a test.

## 5. Touch nothing outside the scope of the batch

Keep diffs focused; do not reformat or refactor unrelated code.
