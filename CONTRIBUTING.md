# Contributing

Contributions are welcome — bug fixes, improvements, or new features. Here's how to get involved.

## Getting started

1. Fork the repo and create a branch from `main`.
2. Follow [DEVELOPMENT.md](DEVELOPMENT.md) to get the project running locally — it covers AWS access, secrets, toolchain, and `make dev`.
3. Make your changes.

> **Always branch from `main`.** It's the stable branch and its history is never rewritten. The `test`, `preprod`, and `prod` branches are deployment branches that push to their respective environments and **may be force-pushed / have their history rewritten** (`test` frequently). Don't base work on them. Note also that all three deployment environments usually share the same production database, so take care when deploying. See [Branches & deployments](README.md#branches--deployments) for details.

## Before submitting a PR

Run the full check suite and make sure everything passes:

```
make check
```

This runs actionlint, Relay compilation, Prettier, ESLint, TypeScript typecheck, a production web build, `terraform fmt`, a Rust toolchain version check, `cargo fmt`, GraphQL schema diffing, and Clippy. CI will run the same checks, so it's worth catching issues locally first.

If it fails on formatting, fix it with:

```
make format
```

If you've changed the GraphQL API, regenerate the schema file before running `make check`:

```
cd api && cargo run --locked --bin export-schema > schema.graphql
cd web && npm run relay
```

> `make check` needs `actionlint` (`brew install actionlint`) and Terraform on your PATH. See [DEVELOPMENT.md](DEVELOPMENT.md#4-install-the-toolchain).

## Opening a pull request

- **One commit per PR.** Squash your branch down to a single commit before opening the PR (or before merge, if review feedback added fixup commits along the way).
- **Keep PRs small and focused** — one logical change per commit/PR makes review easier. For larger work, prefer a stack of small PRs that build on each other (each branched from the previous one, merged in order) over one large PR.
- **Every PR/commit must be safe to deploy on its own.** It must pass CI (`make check` && `make test`) and stand correctly by itself — don't leave a PR in a half-working state that only becomes correct once a later PR in the stack lands. If a change genuinely can't be split into independently-deployable steps, that's a signal to reconsider the approach before opening the PR.
- Write a clear commit message / PR description explaining what the change does and why.
- If the change is non-trivial, include a short note on how you tested it.
- **If it changes what admins see or do, update the [admin user guide](#user-guide) in the same PR.** The PR template asks.

There's no formal issue requirement for small fixes, but for larger changes it's worth opening an issue first to discuss the approach.

## User guide

SES Login has two user guides, published at `/guide` with no sign-in needed: the
**admin guide** for unit administrators and the **kiosk guide** for members. Readers
can search and print them. Their source is Markdown in [`web/guide/`](web/guide/)
(`admin/` and `kiosk/`), compiled into the web build — see
[web/guide/README.md](web/guide/README.md) for how to write a page.

**Every PR that changes the admin or kiosk UI, or how a feature behaves for
users, updates the matching guide in the same PR**: describe the new feature,
correct the steps that changed, rename what was renamed, and re-capture any
screenshot of a screen that changed (`make guide-screenshots`). A kiosk change
usually touches both guides — the kiosk guide for members, the admin guide's
kiosk settings for administrators. A feature isn't finished until the people
using it can find out how.

A UI change can skip the guides only when no user-visible behaviour changes — a
refactor, a styling fix, a test. Say so in the PR description ("Guide: not
needed — refactor only").

The build checks the guides: a broken link between pages, a missing screenshot,
or a page missing its title fails `npm run build` and so CI. Whether the *words*
are right is up to the author and the reviewer — reviewers, read the guide diff
alongside the UI diff. The guides are public, so they must never describe
anything only signed-in users should see.

## Code style

- Rust: formatted with `cargo fmt`, linted with `cargo clippy`.
- TypeScript/JS: formatted with Prettier, linted with ESLint (`npm run lint`).

Both are enforced by `make check`.
