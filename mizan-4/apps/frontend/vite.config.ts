import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;
const apiTarget =
  process.env.VITE_API_TARGET || process.env.WF_API_TARGET || "http://127.0.0.1:8080";
const enableProxy = process.env.WF_ENABLE_VITE_PROXY === "true";
const serverProxy = enableProxy
  ? {
      "/api": {
        target: apiTarget,
        changeOrigin: true,
      },
      "/docs": {
        target: apiTarget,
        changeOrigin: true,
      },
    }
  : undefined;

// Determine build target: "tauri" for desktop, "web" for browser
// Default to "tauri" for local development - use BUILD_TARGET=web for web builds
// TAURI_DEV_HOST is only set for mobile/network dev, so we can't rely on it
const buildTarget = process.env.BUILD_TARGET || "tauri";

// https://vitejs.dev/config/
export default defineConfig({
  envDir: "../..",
  plugins: [react(), tailwindcss()],
  publicDir: "public",
  optimizeDeps: {
    // `recharts` was previously eagerly bundled which forced the
    // ~200 KiB-gzipped chart engine into the initial paint even
    // though it's only used on the History / Performance / Detail
    // pages. Moved into a dedicated lazy chunk via `manualChunks`
    // (see `build.rollupOptions`) so the dashboard ships without it.
    // `lucide-react` stays here for dev-mode pre-bundling speed —
    // it's tree-shaken on production builds by esbuild.
    include: ["lucide-react", "lodash"],
  },
  define: {
    __BUILD_TARGET__: JSON.stringify(buildTarget),
  },
  resolve: {
    alias: {
      "@mizan/addon-sdk": path.resolve(__dirname, "../../packages/addon-sdk/src"),
      "@mizan/ui": path.resolve(__dirname, "../../packages/ui/src"),
      // Conditional adapter alias based on build target
      "@/adapters": path.resolve(
        __dirname,
        buildTarget === "tauri" ? "./src/adapters/tauri" : "./src/adapters/web",
      ),
      // Platform-specific core module for shared adapters
      "#platform": path.resolve(
        __dirname,
        buildTarget === "tauri" ? "./src/adapters/tauri/core" : "./src/adapters/web/core",
      ),
      "@": path.resolve(__dirname, "./src"),
    },
    extensions: [".js", ".ts", ".jsx", ".tsx", ".json"],
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host ? "0.0.0.0" : false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    proxy: serverProxy,
    watch: {
      // 3. tell vite to ignore watching `apps/desktop`
      ignored: ["**/apps/tauri/**"],
    },
  },
  // 3. to make use of `TAURI_DEBUG` and other env variables
  // https://tauri.app/v1/api/config#buildconfig.beforedevcommand
  envPrefix: ["VITE_", "TAURI_", "CONNECT_"],
  build: {
    // Output to project root's dist folder (for Tauri)
    outDir: "../../dist",
    // Tauri uses Chromium on Windows and WebKit on macOS and Linux
    // Keep target unset to use modern defaults for desktop WebView engines.
    // don't minify for debug builds
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    // produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      output: {
        // Code-split heavyweight chart + animation libs into their own
        // chunks so the dashboard's first paint doesn't drag ~250 KB
        // of unused JS along with it. Vite/Rollup walks the import
        // graph and emits a separate file the browser only fetches
        // when one of these modules is touched — i.e. when the user
        // navigates to a page that actually charts something.
        //
        // The pattern matches at MODULE-RESOLUTION time, so any
        // `import "recharts"` deep in a feature triggers the lazy
        // chunk automatically without per-file code changes.
        manualChunks: (id) => {
          if (id.includes("node_modules/recharts/")) return "vendor-charts";
          if (id.includes("node_modules/d3-")) return "vendor-charts";
          if (id.includes("node_modules/framer-motion/")) return "vendor-motion";
          if (id.includes("node_modules/react-pdf/")) return "vendor-pdf";
          return undefined;
        },
      },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    include: ["**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}"],
  },
} as unknown as import("vitest/config").UserConfigExport);
