# Contributing

Contributions are welcome — bug fixes, improvements, or new features. Here's how to get involved.

## Getting started

1. Fork the repo and create a branch from `main`.
2. Follow the setup steps in the [README](README.md) to get the project running locally.
3. Make your changes.

> **Always branch from `main`.** It's the stable branch and its history is never rewritten. The `test`, `preprod`, and `prod` branches are deployment branches that push to their respective environments and **may be force-pushed / have their history rewritten** (`test` frequently). Don't base work on them. Note also that all three deployment environments usually share the same production database, so take care when deploying. See [Branches & deployments](README.md#branches--deployments) for details.

## Before submitting a PR

Run the full check suite and make sure everything passes:

```
make check
```

This runs Relay compilation, Prettier, ESLint, a production web build, `cargo fmt`, GraphQL schema diffing, and Clippy. CI will run the same checks, so it's worth catching issues locally first.

If you've changed the GraphQL API, regenerate the schema file before running `make check`:

```
cd api && cargo run --locked --bin export-schema > schema.graphql
```

## Opening a pull request

- Keep PRs focused — one logical change per PR makes review easier.
- Write a clear description of what the change does and why.
- If the change is non-trivial, include a short note on how you tested it.

There's no formal issue requirement for small fixes, but for larger changes it's worth opening an issue first to discuss the approach.

## Code style

- Rust: formatted with `cargo fmt`, linted with `cargo clippy`.
- TypeScript/JS: formatted with Prettier, linted with ESLint (`npm run lint`).

Both are enforced by `make check`.
