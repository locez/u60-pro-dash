import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [
    {
      name: "shared-router-models",
      transform(code, id) {
        if (
          /\/zte-agent\/ui\/(cellular|clients-model|battery-model|routing-model|data-usage-model|dns-model|wireguard-model|actions|advanced)\.js$/.test(
            id,
          )
        )
          return code.replace(
            /const tr = [^\n]+;/,
            "const tr = (...args) => globalThis.U60Translate(...args);",
          );
      },
    },
    react(),
    tailwindcss(),
  ],
  base: "/console/",
  server: {
    host: "127.0.0.1",
    port: 5177,
    strictPort: true,
    proxy: {
      "/api": { target: process.env.U60_AGENT_URL || "http://127.0.0.1:19090" },
      "/console/proxy": {
        target: process.env.U60_AGENT_URL || "http://127.0.0.1:19090",
      },
    },
  },
  build: { sourcemap: false },
});
