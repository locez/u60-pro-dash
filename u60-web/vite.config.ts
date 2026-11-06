import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  plugins: [
    {
      name: "shared-router-models",
      enforce: "pre",
      transform(code, id) {
        if (
          /\/zte-agent\/ui\/(cellular|clients-model|battery-model|routing-model|data-usage-model|dns-model|wireguard-model|actions|advanced)\.js$/.test(
            id,
          )
        ) {
          // These shared parsers are UMD for the legacy browser and Node tests.
          // Convert them before Vite's CommonJS pass: otherwise production
          // captures module.exports and the browser globals stay undefined.
          const exported =
            /if\s*\(typeof module\s*===\s*'object'\s*&&\s*module\.exports\)\s*module\.exports\s*=\s*api;\s*else\s*globalThis\.(\w+)\s*=\s*api;/;
          if (!exported.test(code))
            throw new Error(`Unsupported shared model export: ${id}`);
          return (
            code
              .replace(exported, "globalThis.$1 = api;")
              .replace(
                /const tr = [^\n]+;/,
                "const tr = (...args) => globalThis.U60Translate(...args);",
              ) + "\nexport {};\n"
          );
        }
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
