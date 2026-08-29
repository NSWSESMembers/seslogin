/// <reference types="vitest/config" />
import { defineConfig, type Plugin } from "vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
import relay from "vite-plugin-relay";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
const __dirname = dirname(fileURLToPath(import.meta.url));

process.env.VITE_CLIENT_VERSION ??= "dev";

// Keep in sync with getGraphQLEndpoint() in src/lib/api.ts, which points the dev
// browser straight at the API rather than through vite.
const DEV_API_ORIGIN = "http://localhost:8000/";
const API_PROBE_TIMEOUT_MS = 2_000;
const API_PROBE_INTERVAL_MS = 500;
const API_WAIT_TIMEOUT_MS = 120_000;

/**
 * Hold the HTML document until the API answers.
 *
 * `make dev` starts vite and the API together, but vite serves in about a second
 * while `cargo run` still has to compile the API — so a page opened in that window
 * loads a web app whose every GraphQL call fails against a dead :8000. Delaying only
 * the document (assets, HMR and everything else still flow) turns that into a tab
 * that waits and then loads a working app.
 *
 * Dev server only; `apply: "serve"` keeps it out of builds.
 */
function waitForApi(): Plugin {
  let apiReady = false;
  let waiting: Promise<void> | null = null;

  const probe = async () => {
    try {
      await fetch(DEV_API_ORIGIN, {
        signal: AbortSignal.timeout(API_PROBE_TIMEOUT_MS),
      });
      return true;
    } catch {
      return false;
    }
  };

  return {
    name: "seslogin:wait-for-api",
    apply: "serve",
    configureServer(server) {
      const { logger } = server.config;

      // One shared wait, however many documents are requested while it runs.
      const waitForReady = () =>
        (waiting ??= (async () => {
          const deadline = Date.now() + API_WAIT_TIMEOUT_MS;
          let announced = false;
          while (!(await probe())) {
            if (Date.now() > deadline) {
              logger.warn(
                `[wait-for-api] ${DEV_API_ORIGIN} still unreachable after ` +
                  `${API_WAIT_TIMEOUT_MS / 1000}s; loading the page anyway.`,
              );
              waiting = null;
              return;
            }
            if (!announced) {
              announced = true;
              logger.info(
                `[wait-for-api] holding page loads until ${DEV_API_ORIGIN} is up...`,
              );
            }
            await new Promise((resolve) =>
              setTimeout(resolve, API_PROBE_INTERVAL_MS),
            );
          }
          if (announced) logger.info("[wait-for-api] API is up.");
          apiReady = true;
        })());

      server.middlewares.use((req, _res, next) => {
        if (apiReady || !req.headers.accept?.includes("text/html")) {
          next();
          return;
        }
        void waitForReady().then(() => next());
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  build: {
    rolldownOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
      },
    },
  },
  optimizeDeps: {
    include: ["exceljs"],
  },
  plugins: [
    relay,
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
    waitForApi(),
  ],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./setupTests.ts"],
  },
  server: {
    forwardConsole: true,
  },
});
