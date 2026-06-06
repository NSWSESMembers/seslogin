# GraphQL API server for seslogin

1. Install rust via [rustup](https://rustup.rs)
2. Setup AWS credentials such as: ~/.aws/credentials

```
[default]
aws_access_key_id = ...
aws_secret_access_key = ...
region = ap-southeast-2
```

3. `RUST_LOG=info cargo run --bin poem`

## Run tests

```
cargo test
```

## Run dev API server

```
RUST_LOG=info cargo run --bin poem
```

Listens on port 8000.

Add `--enable-mutations` to allow writes.

## SES member sync

Set `ses_api_headquarters_id` per location via the admin UI before syncing.

Run sync in dry-run mode (prints planned changes only):

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members -- --dry-run
```

Run sync in apply mode:

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members --
```

SQS Lambda binary (invoked per location by the dispatcher; reads config from env vars):

```bash
cargo run --bin sync-members-sqs-lambda
```

Standalone Lambda binary (reads config from env vars and executes one sync run directly):

```bash
cargo run --bin sync-members-lambda
```

Optional flags:

- `--location-id L10 --location-id L22` limits syncing to specific locations.
- `--page-limit 100` overrides SES API page size.
- `--max-retries 3` controls retries for transient SES API failures.
- `--max-mutations 500` aborts apply mode if planned writes exceed threshold.

Behavior notes:

- Imported SES members are tagged with `members.ses_api_person_id`.
- Updates/deletes only apply to local rows whose `ses_api_person_id` matches SES `person.id`.
- If a row exists with matching `serialnumber` but no `ses_api_person_id`, sync prints a skip message and does not modify that row.
