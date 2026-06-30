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
- AWS credentials configured (for DynamoDB)

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

### 3. Run locally (Docker)

```bash
make dev
```

This starts the API server, Relay compiler (watch mode), and Vite dev server inside Docker Compose.

- Web app: http://localhost:5173
- API: http://localhost:8000

To stop the dev container:

```bash
make dev-down
```

### 4. Build in Docker (optional)

If you prefer not to install Rust and Node.js directly on your machine, you can use the containerized build environment:

```bash
make docker-image   # build the dev/build image (Rust stable + Node 22)
make docker-build   # run web + API builds inside the container
make docker-shell   # open an interactive shell in the container
```

The docker targets mount this repository into the container and use named volumes for Cargo and npm caches to speed up repeat builds.

For a persistent container workflow, use Docker Compose:

```bash
make docker-up        # start the dev container in the background
make docker-shell     # open a shell in the running dev container
make docker-workflow  # run the web + API build workflow inside that container
make docker-down      # stop and remove the compose stack
```

This workflow uses a single `dev` container as both the toolchain environment
and the running API/web stack.

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

## Contributing

Contributions are welcome — bug fixes, improvements, or new features. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to submit a PR and run the checks.

---

## License

[MIT](LICENSE)
