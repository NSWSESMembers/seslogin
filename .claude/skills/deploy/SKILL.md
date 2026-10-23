---
name: deploy
description: Summarise the commits on main that haven't reached the test, preprod or prod deployment branches, then offer to push main to one of them (and to workers after a prod push). Use when asked what's undeployed, what's pending release, or to deploy/ship/release main.
---

# Deploy main

Pushing to a deployment branch triggers its GitHub Actions deploy workflow, and **every
deployed environment runs against the live production database** (see CLAUDE.md, "Lambda
Deployment"). Never push without the user's explicit choice in this run.

| Branch | Deploys |
|--------|---------|
| `test` | Test API Lambda + web (`test.seslogin.com`) |
| `preprod` | Preprod API Lambda + web (`preprod.seslogin.com`) |
| `prod` | Production API Lambda + web (`new.seslogin.com`) |
| `workers` | Sync/dispatcher/checker/nitc-export/healthcheck/activity-summary/sync-locations Lambdas |

## 1. Fetch

```bash
git fetch origin main test preprod prod workers
```

A branch that doesn't exist on the remote is fine — report it as "not present" and carry on.

## 2. Summarise

For each of `test`, `preprod`, `prod`:

```bash
git log --oneline --no-merges origin/<branch>..origin/main   # on main, not yet deployed
git log --oneline --no-merges origin/main..origin/<branch>   # on the branch, not on main
```

Present a summary the user can scan quickly:

- One section per branch, with the count of commits main is ahead and a one-line summary of
  each (group related commits, e.g. a stack of MCP PRs, rather than parroting every subject
  if the list is long). Mention the PR number where the subject carries one.
- Say plainly when a branch is already up to date with main.
- **Flag any branch that has commits not on main** (the second `git log`). Deployment
  branches are force-pushed, especially `test`, so this is common — but pushing main there
  will discard those commits from the deployed environment, and the user should know which
  ones before choosing.
- If `origin/main..origin/<branch>` is empty, pushing main there is a plain fast-forward.

Also note `workers`' status (ahead count against main) in one line, since it's the follow-up
to a prod push.

## 3. Offer the push

Ask (AskUserQuestion) which branch to push main to: `test`, `preprod`, `prod`, or none.
Note in each option whether it's a fast-forward or would overwrite commits.

Then push exactly `origin/main` — not the local checkout, which may be stale or dirty:

```bash
# fast-forward
git push origin origin/main:refs/heads/<branch>
# branch has commits not on main — only after the user confirmed overwriting them
git push --force-with-lease=<branch>:<sha of origin/branch you showed> origin origin/main:refs/heads/<branch>
```

Use `--force-with-lease` pinned to the SHA from step 1, so a push someone else made in the
meantime is rejected rather than silently destroyed. If the push is refused by
permissions (e.g. this session can only push its own branch), say so and give the user the
command to run themselves.

## 4. After a prod push, offer workers

If the push went to `prod`, ask whether to push the same commit to `workers` too, showing
the commits it would add (`git log --oneline origin/workers..origin/main`) and flagging any
commits on `workers` that aren't on main, as above. Push the same way.

## 5. Report

State which branches were pushed and to which SHA, and that the deploy workflow
(`deploy-<branch>.yml`) is now running on GitHub Actions. Don't wait for or poll the
workflow unless the user asks.
