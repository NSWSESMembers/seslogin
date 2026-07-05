# SES Activity

SES Activity is a web app for tracking volunteer attendance with as little friction as possible. Members check in and out at a kiosk, sessions are recorded with activity categories, and reports are available for coordinators.

**Features:**
- Fast check-in/out — designed for kiosk use
- Activity categories (training, rescue, combat roles, etc.)
- Detailed reporting
- Automatic member import from the SES headquarters API
- Runs on AWS Lambda + DynamoDB — scales to zero when idle

**Stack:** Rust (GraphQL API) · React + Relay (frontend) · AWS (Lambda, DynamoDB, SQS, CloudFront)

---

## Getting started

### Prerequisites

- Rust (stable, via [rustup](https://rustup.rs))
- Node.js >= 22
- AWS credentials configured (for DynamoDB). The deployment/Terraform account (`641079927221`) is the `seslogin` profile — an IAM Identity Center (SSO) profile; run `aws sso login --profile seslogin`.

### 1. Configure secrets

Non-secret config lives in `.env` at the repo root (checked in). Create `.env.secret` for secrets (gitignored):

```
JWT_SECRET=...
SES_API_BASE_URL=...
SES_API_KEY=...
```

### 2. Configure the frontend

Copy the example env file and fill it in:

```
cp web/.env.local.example web/.env.local
```

| Variable | Description |
| --- | --- |
| `VITE_BEACON_URL` | Base URL for the Beacon system, used to link NITC event IDs. For deployed builds, set the `BEACON_URL` repo variable in GitHub Actions Settings → Variables. |

### 3. Run locally

```
make dev
```

This starts the API server, Relay compiler (watch mode), and the Vite dev server together. Or run them individually:

```bash
cd api && RUST_LOG=info cargo run --bin poem -- --enable-mutations  # API on port 8000
cd web && npm run relay -- --watch                                   # Relay compiler
cd web && npm run dev                                                 # Web dev server
```

---

## Project structure

```
api/    Rust GraphQL backend + all Lambda binaries
web/    React/Relay frontend
infra/  Terraform for AWS infrastructure
```

> **Note:** `infra/` is published as a reference for the canonical seslogin.com deployment — it hardcodes the production AWS account, DNS zone, and ACM records, so `terraform apply` from a fork will not work without adapting bucket names, domains, and IAM resources. Use it as a worked example, not a turnkey deploy.

See [SCHEMA.md](SCHEMA.md) for the data model and [MANUAL.md](MANUAL.md) for operator documentation.

---

## Branches & deployments

| Branch | Environment |
| --- | --- |
| `test` | [test.seslogin.com](https://test.seslogin.com) — experimental; frequently rewritten |
| `preprod` | [preprod.seslogin.com](https://preprod.seslogin.com) — production-like staging |
| `prod` | [new.seslogin.com](https://new.seslogin.com) — production |

`test`, `preprod`, and `prod` are deployment branches: pushing to one deploys to its environment. They **may have their history rewritten / force-pushed** — `test` especially, as experimental work lands there often.

> ⚠️ All three environments are usually configured to use the **same production database**, so a push to any of them (including `test`) can affect live data. Take care.

`main` is the stable branch and is never force-pushed. It may be ahead of or behind the deployment branches. **Fork PR branches from `main`.**

---

## Contributing

Contributions are welcome — bug fixes, improvements, or new features. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to submit a PR and run the checks.

---

## License

[MIT](LICENSE)
