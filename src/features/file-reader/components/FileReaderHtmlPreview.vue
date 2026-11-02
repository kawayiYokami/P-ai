<template>
  <div class="flex h-full min-h-0 w-full flex-col overflow-auto px-4 py-4">
    <!-- 复用聊天里的同一份沙箱：CSP、iframe sandbox、主题注入都由 buildSandboxSrcdoc 负责 -->
    <InteractiveHtmlSandbox
      :code="code"
      :is-dark="isDark"
      :title-text="titleText"
      :loading-text="loadingText"
      :reload-title="reloadTitle"
      class="min-w-0"
    />
  </div>
</template>

<script setup lang="ts">
import InteractiveHtmlSandbox from "../../chat/components/InteractiveHtmlSandbox.vue";

/**
 * 文件阅读器里的 HTML 按沙箱渲染。
 *
 * 不要在这里另写 iframe 或自己拼 CSP：一律走 InteractiveHtmlSandbox，
 * 与聊天里的交互组件用同一套注入逻辑，沙箱属性也只有那一处可改。
 */
withDefaults(defineProps<{
  code: string;
  isDark?: boolean;
  titleText?: string;
  loadingText?: string;
  reloadTitle?: string;
}>(), {
  isDark: false,
  titleText: "",
  loadingText: "",
  reloadTitle: "",
});
</script>
