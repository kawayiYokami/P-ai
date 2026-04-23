import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith("calendar-"),
        },
      },
    }),
  ],
  clearScreen: false,
  build: {
    rollupOptions: {
      input: {
        config: resolve(__dirname, "index.html"),
        chat: resolve(__dirname, "chat.html"),
        archives: resolve(__dirname, "archives.html"),
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        "**/.git/**",
        "**/node_modules/**",
        "**/dist/**",
        "**/src-tauri/target/**",
        "**/src-tauri/memory/**",
        "**/src-tauri/gen/**",
        "**/src-tauri/icons/**",
      ],
    },
  },
});
