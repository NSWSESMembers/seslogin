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

**→ See [DEVELOPMENT.md](DEVELOPMENT.md) for the full setup guide.** It covers getting AWS
access, obtaining secrets, installing the toolchain, and running the stack.

The short version, once you have AWS credentials and a `.env.secret`:

```bash
cp web/.env.local.example web/.env.local
cd web && npm i
make dev        # API :8000 + Relay watch + web dev server :5173
```

Prerequisites are Rust (via [rustup](https://rustup.rs)), Node.js >= 22, and AWS
credentials for DynamoDB — there is no offline mode.

> Local dev defaults to the **dev** database tables (`DB_PREFIX=seslogin_test`), which hold
> an out-of-date partial snapshot of production. See
> [Which database am I using?](DEVELOPMENT.md#3-which-database-am-i-using) before pointing
> anything at prod.

For deployed builds, `VITE_BEACON_URL` comes from the `BEACON_URL` repo variable in GitHub
Actions Settings → Variables.

---

## Project structure

```
api/    Rust GraphQL backend + all Lambda binaries
web/    React/Relay frontend
infra/  Terraform for AWS infrastructure
```

> **Note:** `infra/` is published as a reference for the canonical seslogin.com deployment — it hardcodes the production AWS account, DNS zone, and ACM records, so `terraform apply` from a fork will not work without adapting bucket names, domains, and IAM resources. Use it as a worked example, not a turnkey deploy.

See [DEVELOPMENT.md](DEVELOPMENT.md) for local setup, [SCHEMA.md](SCHEMA.md) for the data model, and [MANUAL.md](MANUAL.md) for operator documentation.

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
