import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { describe, expect, it } from "vitest";

const sourceRoot = resolve(process.cwd(), "src");
const adapterPath = resolve(sourceRoot, "services", "tauri-api.ts");
const sidebarRoot = resolve(sourceRoot, "features", "sidebar");
const sidebarEntryPath = resolve(sidebarRoot, "main-sidebar.ts");
const sidebarExtensionRoot = resolve(sidebarRoot, "extension");
const webDispatcherPath = resolve(
  process.cwd(),
  "src-tauri",
  "src",
  "features",
  "system",
  "commands",
  "ide_context",
  "jsonrpc_dispatch.rs",
);
const hostExtensionPath = resolve(sourceRoot, "features", "sidebar", "extension", "extension.js");
const capabilityConsumerPaths = new Set([
  resolve(sourceRoot, "ConfigWindowApp.vue"),
  resolve(sourceRoot, "features", "chat", "components", "dialogs", "ChatImagePreviewDialog.vue"),
  resolve(sourceRoot, "features", "config", "views", "config-tabs", "ImageGenerationTab.vue"),
  resolve(sourceRoot, "features", "config", "views", "config-tabs", "ChatSettingsTab.vue"),
  resolve(sourceRoot, "features", "config", "views", "config-tabs", "McpTab.vue"),
  resolve(sourceRoot, "features", "config", "views", "config-tabs", "SkillTab.vue"),
  resolve(sourceRoot, "features", "config", "views", "config-tabs", "StorageTab.vue"),
  resolve(sourceRoot, "features", "file-reader", "components", "FileReaderPanel.vue"),
  resolve(sourceRoot, "features", "shared", "components", "FileLinkContextMenu.vue"),
  resolve(sourceRoot, "features", "shell", "components", "AppWindowHeader.vue"),
]);

const portableChatNativeCommand = new RegExp(`\\b(?:${[
  "submit_chat_message", "stop_chat_message", "archive_conversation", "compact_conversation",
  "get_foreground_conversation_light_snapshot", "get_foreground_conversation_freshness_snapshot",
  "get_conversation_runtime_snapshot", "get_unarchived_conversation_message_by_id",
  "get_active_conversation_messages_before", "request_conversation_messages_after_async",
  "mark_conversation_read", "set_active_unarchived_conversation", "set_conversation_preferred_model",
  "delete_unarchived_conversation", "rename_unarchived_conversation", "toggle_unarchived_conversation_pin",
  "set_conversation_auto_push_remote_contact", "rebind_unarchived_conversation_recipient",
  "branch_unarchived_conversation_from_selection", "create_conversation_branch_from_message",
  "forward_unarchived_conversation_selection", "forward_selection_to_remote_im_contact",
  "list_unarchived_conversations", "list_unarchived_conversations_changed_since",
  "get_unarchived_conversation_block_page", "list_delegate_conversations",
  "get_delegate_conversation_block_page", "delete_delegate_conversation", "submit_user_async_delegate",
  "remote_im_list_contact_conversations", "remote_im_get_contact_conversation_block_page",
  "remote_im_clear_contact_conversation", "goal_get_current", "goal_create_goal", "goal_cancel_goal",
  "get_prompt_preview", "get_system_prompt_preview", "get_conversation_section_orders",
  "save_conversation_section_order", "list_archives", "get_archive_block_page", "get_archive_summary",
  "delete_archive", "unarchive_archive", "batch_archive_conversations",
  "check_git_workspace_root", "get_chat_shell_workspace", "update_chat_shell_workspace_layout",
].join("|")})\\b`);

function adapterNativeOnlyCommands(): string[] {
  const adapterSource = readFileSync(adapterPath, "utf8");
  const block = adapterSource.match(/const WEB_BRIDGE_NATIVE_ONLY_COMMANDS = new Set\(\[([\s\S]*?)\]\);/);
  if (!block?.[1]) throw new Error("无法读取统一传输适配器的 native-only 命令清单");
  return Array.from(block[1].matchAll(/["']([^"']+)["']/g), (match) => match[1]);
}

const nativeOnlyCommands = adapterNativeOnlyCommands();

function dispatcherNativeOnlyCommands(): string[] {
  const dispatcherSource = readFileSync(webDispatcherPath, "utf8");
  const block = dispatcherSource.match(
    /fn ide_chat_web_native_only_method\(method: &str\) -> bool \{[\s\S]*?matches!\([\s\S]*?method,([\s\S]*?)\n\s*\)\n\}/,
  );
  if (!block?.[1]) throw new Error("无法读取 Web dispatcher 的 native-only 命令清单");
  return Array.from(block[1].matchAll(/"([^"]+)"/g), (match) => match[1]);
}

function sourceFiles(directory: string): string[] {
  const result: string[] = [];
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      if (["node_modules", "dist", "coverage"].includes(entry)) continue;
      result.push(...sourceFiles(path));
      continue;
    }
    if (/\.(ts|vue|js)$/.test(entry) && !/\.spec\.(ts|js)$/.test(entry)) result.push(path);
  }
  return result;
}

describe("统一传输边界", () => {
  it("Sidebar 只能复用统一聊天入口，不得保留第二套 Vue 前台", () => {
    const entrySource = readFileSync(sidebarEntryPath, "utf8");
    // 样式入口（sidebar-theme.css 按 data-host 分流宿主主题）不属于 Vue 前台，
    // 唯一前台必须是 main-chat，不得出现第二套聊天状态机/视图。
    const entryImports = Array.from(
      entrySource.matchAll(/^\s*import\s+["']([^"']+)["'];?\s*$/gm),
      (match) => match[1],
    ).filter((path) => !path.startsWith("./assets/") && !path.endsWith(".css"));
    expect(entryImports).toEqual(["../../main-chat"]);

    const independentSources = sourceFiles(sidebarRoot)
      .filter((path) => path !== sidebarEntryPath && !path.startsWith(`${sidebarExtensionRoot}${sep}`))
      .map((path) => relative(process.cwd(), path));
    expect(independentSources).toEqual([]);
  });

  it("前端适配器与 Web dispatcher 使用同一份本机能力边界", () => {
    expect([...nativeOnlyCommands].sort()).toEqual(dispatcherNativeOnlyCommands().sort());
  });

  it("导入导出不得在业务组件内选择原生命令或浏览器实现", () => {
    const memoryExportSources = [
      resolve(sourceRoot, "features", "config", "components", "MemoryExportCard.vue"),
      resolve(sourceRoot, "features", "memory", "composables", "use-memory-viewer.ts"),
    ].map((path) => readFileSync(path, "utf8"));
    for (const text of memoryExportSources) {
      expect(text).toContain("exportTransportMemories");
      expect(text).not.toMatch(/getTransportCapabilities|saveTransportFileDialog|exportTransportMemoriesToPath/);
      expect(text).not.toMatch(/["']export_memories["']/);
    }

    const storageSource = readFileSync(
      resolve(sourceRoot, "features", "config", "views", "config-tabs", "StorageTab.vue"),
      "utf8",
    );
    expect(storageSource).toContain("pickTransportConfigMigrationPackage");
    expect(storageSource).not.toMatch(/FileReader|downloadBase64File|packageBytesBase64|openTransportFileDialog/);
  });

  it("业务源码不得重新创建通信桥或探测运行平台", () => {
    const violations: string[] = [];
    for (const path of sourceFiles(sourceRoot)) {
      if (path === adapterPath) continue;
      const text = readFileSync(path, "utf8");
      const relativePath = relative(process.cwd(), path);
      const normalizedRelativePath = relativePath.replaceAll("\\", "/");
      const checks: Array<[RegExp, string]> = [
        [/@tauri-apps\//, "direct Tauri API or plugin"],
        [/new\s+WebSocket\s*\(/, "direct WebSocket"],
        [/__TAURI(?:_INTERNALS)?__/, "runtime probe"],
        [/acquireVsCodeApi\s*\(/, "host probe"],
        [/\bbridge(?:Request|Subscribe)\b/, "legacy bridge"],
        [/isTauriRuntimeAvailable\s*\(/, "platform branch"],
        [/\b(?:isPrimaryTransportChatView|isPrimaryChatView|isPrimaryChatWindow)\b/, "chat host/platform branch"],
        [/getCurrentTransportWindowLabel\s*\(/, "raw window-label branch"],
        [/\b(?:tauriWindowLabel|isChatTauriWindow)\b/, "raw Tauri window state"],
        [/\b(?:emitTauriEvent|emitTauriEventTo|uploadBrowserFileThroughTauri)\b/, "Tauri-specific transport API"],
        [/chat-message-tauri-adapter/, "Tauri-specific chat event adapter"],
        [/SidebarLightMarkdown|ecall-sidebar-light/, "sidebar-specific shared renderer"],
        [/\b(?:getTransportHostTheme|initializeTransportHostAppearance)\b/, "host-specific appearance"],
        [/data-host\s*=/, "host-specific styling branch"],
        [/\bsidebarMode\b|sidebar-mode\s*=/, "shared chat platform branch"],
        [/\bconversation\.open\b/, "web-only conversation projection"],
        [/conversation\.compactPreview/, "sidebar-only compaction path"],
        [/\b(?:preview_rewind_conversation_from_message|rewind_conversation_from_message)\b/, "native protocol command in business code"],
        [/\bonNativeFileDrop\b/, "native file-drop naming"],
        [/addEventListener\s*\(\s*["']message["']/, "direct host message listener"],
      ];
      for (const [pattern, label] of checks) {
        if (pattern.test(text)) violations.push(`${relativePath}: ${label}`);
      }
      if (
        (normalizedRelativePath.startsWith("src/features/chat/")
          || normalizedRelativePath.startsWith("src/features/sidebar/"))
        && portableChatNativeCommand.test(text)
      ) {
        violations.push(`${relativePath}: native chat protocol command outside adapter`);
      }
      if (path !== hostExtensionPath && /\.postMessage\s*\(/.test(text)) {
        violations.push(`${relativePath}: direct postMessage`);
      }
      if (path !== adapterPath && /getTransportCapabilities\s*\(/.test(text) && !capabilityConsumerPaths.has(path)) {
        violations.push(`${relativePath}: capability branch outside button/file capability consumers`);
      }
      for (const command of nativeOnlyCommands) {
        if (
          text.includes(`"${command}"`)
          || text.includes(`'${command}'`)
          || text.includes(`\`${command}\``)
        ) {
          violations.push(`${relativePath}: native-only command outside adapter (${command})`);
        }
      }
    }
    expect(violations).toEqual([]);
  });
});
