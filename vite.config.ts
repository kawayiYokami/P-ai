/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";
import { readdir, rename, rm } from "node:fs/promises";

const entryDir = resolve(__dirname, "src/entries");
// 隔离工作树位于 .pai/.worktree/* 下；若 ignored 里再排除 .pai，会把工作树自身
// 的所有文件一并排除，watcher 收不到变更、HMR 完全失效。
const isIsolatedWorktree = __dirname.replace(/\\/g, "/").includes("/.pai/.worktree/");
const htmlEntryAliases = new Set([
  "/",
  "/index.html",
  "/chat.html",
  "/archives.html",
  "/file-reader.html",
  "/runtime-logs.html",
  "/sidebar.html",
  "/settings.html",
  "/demo.html",
]);

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith("calendar-"),
        },
      },
    }),
    {
      name: "pai-html-entry-rewrite",
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const requestUrl = req.url ?? "/";
          const [pathname, search = ""] = requestUrl.split("?", 2);
          if (!htmlEntryAliases.has(pathname)) {
            next();
            return;
          }

          const normalizedPath = pathname === "/" ? "/index.html" : pathname;
          req.url = `/src/entries${normalizedPath}${search ? `?${search}` : ""}`;
          next();
        });
      },
      async closeBundle() {
        const builtEntryDir = resolve(__dirname, "dist/src/entries");
        let builtEntries: string[] = [];
        try {
          builtEntries = await readdir(builtEntryDir);
        } catch {
          return;
        }

        await Promise.all(
          builtEntries
            .filter((fileName) => fileName.endsWith(".html"))
            .map((fileName) =>
              rename(
                resolve(builtEntryDir, fileName),
                resolve(__dirname, "dist", fileName),
              ),
            ),
        );

        await rm(resolve(__dirname, "dist/src"), { recursive: true, force: true });
      },
    },
  ],
  clearScreen: false,
  build: {
    rollupOptions: {
      input: {
        config: resolve(entryDir, "index.html"),
        chat: resolve(entryDir, "chat.html"),
        archives: resolve(entryDir, "archives.html"),
        fileReader: resolve(entryDir, "file-reader.html"),
        runtimeLogs: resolve(entryDir, "runtime-logs.html"),
        sidebar: resolve(entryDir, "sidebar.html"),
        settings: resolve(entryDir, "settings.html"),
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
        // 在工作树里不排除 .pai（它自身就在 .pai 下）；主仓 dev 仍忽略 .pai 下的工作树与参考项目。
        ...(isIsolatedWorktree ? [] : ["**/.pai/**"]),
        "**/.debug/**",
        "**/.qoder/**",
        "**/temp/**",
        "**/relay_tool_probe/**",
      ],
    },
  },
  test: {
    // .pai/ 下的参考项目快照（第三方组件，独立依赖树）与隔离工作树（旧版本
    // 副本，各自有 pnpm test）不属于主仓测试范围；vitest 默认 include 会全仓
    // 扫描把它们的测试卷进来，导致缺依赖/快照过期类的无关失败。
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/cypress/**",
      "**/.{idea,git,cache,output,temp}/**",
      "**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build,eslint,prettier}.config.*",
      "**/.pai/**",
    ],
  },
});
