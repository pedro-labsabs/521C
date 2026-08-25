import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

// Standalone vitest config: protocol/state tests run in plain Node and must not
// load the TanStack Start / Tailwind build plugins from vite.config.ts.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
});
