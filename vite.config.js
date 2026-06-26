import { defineConfig } from "vite";

// https://vitejs.dev/config/
export default defineConfig({
  // Prevent Vite from obscuring Rust errors in development
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Watch for changes in the Tauri source too
      ignored: ["**/src-tauri/**"],
    },
  },
  // Build everything into a single offline-capable bundle.
  // No CDN references anywhere — all assets are inlined or local.
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: true,
    // Tauri uses sync File APIs, so this is safe
    rollupOptions: {
      output: {
        // Single chunk for editor — keeps offline load fast
        manualChunks: undefined,
      },
    },
  },
  // Vite processes these as assets; Tauri bundles them into the binary
  assetsInclude: ["**/*.html"],
});
