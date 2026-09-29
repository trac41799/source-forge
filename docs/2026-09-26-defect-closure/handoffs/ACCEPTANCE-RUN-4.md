# Acceptance Run 4 — agents' work is now actually delivered

**Date:** 2026-09-29 · branch `hardening/production`

## The gap this closes

Before this change the pipeline ran agents in worktrees and then verified the
**base project**, which contained none of their work. The run could report
`succeeded` while every agent's output stayed stranded in a throwaway worktree —
the acceptance test had to assert `agent_wrote_greeting_in_worktree=true` because
`greeting_in_base_project` was false.

Now `wave_executor::merge_completed_agents` runs between finalize and verify:

1. commits each completed agent's worktree (excluding `.acc/` bookkeeping),
2. merges its branch into the base branch (`--no-ff`),
3. removes the worktree — only after its work is merged,
4. on conflict: aborts the merge, records it, and the run pauses as
   `awaiting_user` with the conflicting agents named (deploy is skipped too).

A production-mode flag (`PipelineAdapters::deliver`) enables it, alongside
`install_deps`, so hermetic tests stay offline and deterministic.

## Result (real agents, 2 concurrent)

```
status : succeeded
  finalize_and_verify done  verification passed=true
  deploy              done  deployed via mock → … (+ Dockerfile artifact)
delivery: greeting_in_base_project=true farewell_in_base_project=true
merges  : {"conflicts":[],"merged":[{"agent_ref":"1-1.1",…},{"agent_ref":"1-1.2",…}]}
192 lib + 42 integration, 341 web, tsc/lint clean — 69.75s
```

Both agents' files are in the base project; every merge names its commit; no
worktrees are left behind.

## Defect found by this step (and fixed)

The first delivery attempt failed with
`CONFLICT (add/add): Merge conflict in .acc/GUIDELINE.md` — each agent's worktree
contained its **own** copy of the generated guideline, and every merge after the
first collided on it. Per-agent bookkeeping is not deliverable, so the commit now
excludes `.acc/` via a pathspec (`:(exclude).acc`), which needs no change to the
user's git config. The fail-closed behaviour worked: the run reported
`awaiting_user` and skipped deploy instead of shipping a half-merged project.
Regression test: the merge tests give every agent a distinct
`.acc/GUIDELINE.md` and still expect a clean 2/2 merge.

## Still not proven

- **Real build/test gate end-to-end.** `install_deps` + `VerifyMode::RequireBuild`
  are implemented and unit-tested (`test_require_build_turns_tooling_skips_into_failures`),
  but the acceptance run keeps `install_deps=false` because the synthetic fixture
  has no real toolchain (its `build` script references `vite`, which is not
  installed). The next milestone is a **from-empty** run: `scaffold_project`
  produces a real app → `npm install` → real `build`/`typecheck`/runtime checks
  execute and can fail — the point where "verification passed" finally means
  "the app was built and tested".
- **GitHub delivery** (private repo, issues, PRs) is still not implemented; it
  remains documented in `EXTERNAL-VERIFICATION.md`.
