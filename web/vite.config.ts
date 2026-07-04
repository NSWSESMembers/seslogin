import { defineConfig } from "vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
import relay from "vite-plugin-relay";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
const __dirname = dirname(fileURLToPath(import.meta.url));

process.env.VITE_CLIENT_VERSION ??= "dev";

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
  ],
  server: {
    forwardConsole: true,
  },
});
