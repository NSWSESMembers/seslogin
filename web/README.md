# seslogin web UI

> **First-time setup is in [DEVELOPMENT.md](../DEVELOPMENT.md)** — AWS access, secrets, and
> running the full stack with `make dev`. This file covers web-specific commands.

Requires Node.js >= 22.

- Install dependencies: `npm i`
- Start Relay compiler in watch mode: `npm run relay -- --watch`
- Run the dev server with hot module reloading: `npm run dev` (or `npm run start`)
- Build production assets: `npm run build`
- Run all web unit tests: `npm run test:unit`
- Run a single unit test file: `npm run test:unit -- src/admin/components/useSelectedLocation.test.tsx`

## Contributing

- Vscode users please -
  - install ESlint, GraphQL, Relay GraphQL extensions.
  - Enable format on save (search settings for 'format on save')
