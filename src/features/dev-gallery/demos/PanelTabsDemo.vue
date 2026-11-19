<template>
  <div class="space-y-4">
    <div class="flex flex-wrap items-center gap-3">
      <span class="text-xs text-base-content/70">容器宽度调节：</span>
      <input
        v-model.number="containerWidth"
        type="range"
        min="200"
        max="700"
        step="10"
        class="range range-xs range-primary w-64"
      />
      <span class="font-mono text-xs">{{ containerWidth }}px</span>
      <button type="button" class="btn btn-ghost btn-xs" @click="containerWidth = 360">默认 (360px)</button>
      <button type="button" class="btn btn-ghost btn-xs" @click="containerWidth = 240">窄视口 (240px)</button>
      <button type="button" class="btn btn-ghost btn-xs" @click="containerWidth = 560">宽视口 (560px)</button>
      <button type="button" class="btn btn-ghost btn-xs text-primary" @click="resetTabs">重置标签列表</button>
    </div>

    <div class="space-y-4">
      <div>
        <div class="mb-1.5 flex items-center justify-between text-xs font-medium text-base-content/70">
          <span>会话风格 (variant="default")</span>
          <span class="text-micro text-base-content/50">当前激活：{{ activeKey }}</span>
        </div>
        <div
          class="overflow-hidden rounded-box border border-base-300 bg-base-200"
          :style="{ width: `${containerWidth}px`, maxWidth: '100%' }"
        >
          <PanelTabStrip
            :tabs="tabs"
            :active-key="activeKey"
            aria-label="Demo Tabs"
            close-title="关闭"
            @select-tab="(k) => (activeKey = k)"
            @close-tab="onCloseTab"
          />
        </div>
      </div>

      <div>
        <div class="mb-1.5 flex items-center justify-between text-xs font-medium text-base-content/70">
          <span>文件风格 (variant="file")</span>
          <span class="text-micro text-base-content/50">当前激活：{{ fileActiveKey }}</span>
        </div>
        <div
          class="overflow-hidden rounded-box border border-base-300 bg-base-200"
          :style="{ width: `${containerWidth}px`, maxWidth: '100%' }"
        >
          <PanelTabStrip
            variant="file"
            :tabs="fileTabs"
            :active-key="fileActiveKey"
            aria-label="Demo File Tabs"
            close-title="关闭"
            @select-tab="(k) => (fileActiveKey = k)"
            @close-tab="onCloseFileTab"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import PanelTabStrip from "../../shared/components/PanelTabStrip.vue";
import type { PanelTabStripItem } from "../../shared/components/panel-tab-strip.types";

function svgDataUrl(svg: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const avatarMap: Record<string, string> = {
  leader: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><g fill="#FFFFFF" transform="translate(10, 10)"><path d="M10 2.5a1.5 1.5 0 0 0-1.5 1.5v3.2A7.5 7.5 0 0 0 4 10a7.5 7.5 0 0 0 7.5 7.5 7.5 7.5 0 0 0 6-3v1.5a1.5 1.5 0 0 0 3 0V4a1.5 1.5 0 0 0-1.5-1.5H10z" opacity="0.95"/><circle cx="10" cy="10" r="3" fill="#FDE68A"/></g></svg>`),
  小吉祥草王: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#10B981"/><path d="M20 9c-4 5-3 10 0 14 3-4 4-9 0-14zM14 17c5-2 9 0 11 3-5 2-9 0-11-3zM26 17c-5-2-9 0-11 3 5 2 9 0 11-3z" fill="#ECFDF5"/><circle cx="20" cy="18" r="3" fill="#A7F3D0"/></svg>`),
  莫娜: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#6366F1"/><path d="M20 8l2.5 8.5L31 19l-8.5 2.5L20 30l-2.5-8.5L9 19l8.5-2.5z" fill="#EEF2FF"/><circle cx="20" cy="19" r="2.5" fill="#C7D2FE"/></svg>`),
  桑多涅: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#EC4899"/><circle cx="20" cy="20" r="6" fill="#FCE7F3"/><path d="M14 16c3 2 3 6 0 8 3-1 6-1 8-3-2-2-5-2-8-5zM26 16c-3 2-3 6 0 8-3-1-6-1-8-3 2-2 5-2 8-5z" fill="#FFF1F2"/></svg>`),
  钟离: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><rect x="13" y="13" width="14" height="14" rx="2" fill="none" stroke="#FEF3C7" stroke-width="2.5"/><rect x="17" y="17" width="6" height="6" fill="#FEF3C7"/></svg>`),
  小黑塔: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#8B5CF6"/><circle cx="20" cy="15" r="4.5" fill="#EDE9FE"/><path d="M13 27c0-4 3.5-6 7-6s7 2 7 6z" fill="#EDE9FE"/></svg>`),
};

const containerWidth = ref(360);
const activeKey = ref("herta");
const fileActiveKey = ref("readme");

const initialTabs: PanelTabStripItem[] = [
  {
    key: "herta",
    label: "小黑塔",
    iconSrc: avatarMap["小黑塔"],
    statusDotClass: "size-2 shrink-0 rounded-full bg-success border border-base-100",
    statusTitle: "在线执行中",
    closeable: true,
  },
  {
    key: "alhaitham",
    label: "书记官",
    iconSrc: avatarMap["leader"],
    statusDotClass: "size-2 shrink-0 rounded-full bg-base-content/30 border border-base-100",
    statusTitle: "空闲就绪",
    closeable: true,
  },
  { key: "yae", label: "八重神子", iconSrc: avatarMap["莫娜"], closeable: true },
  { key: "nahida", label: "小吉祥草王", iconSrc: avatarMap["小吉祥草王"], closeable: true },
  { key: "sandrone", label: "桑多涅", iconSrc: avatarMap["桑多涅"], closeable: true },
  { key: "zhongli", label: "钟离", iconSrc: avatarMap["钟离"], busy: true, closeable: true },
];

const tabs = ref<PanelTabStripItem[]>([...initialTabs]);

const initialFileTabs: PanelTabStripItem[] = [
  { key: "readme", label: "README.md", icon: "file-text", closeable: true },
  { key: "tabstrip", label: "PanelTabStrip.vue", icon: "file-text", closeable: true },
  { key: "overflow", label: "use-panel-tab-overflow.ts", icon: "file-text", closeable: true },
  { key: "package", label: "package.json", icon: "file-text", closeable: true },
  { key: "cargo", label: "Cargo.toml", icon: "file-text", closeable: true },
  { key: "vite", label: "vite.config.ts", icon: "file-text", closeable: true },
];

const fileTabs = ref<PanelTabStripItem[]>([...initialFileTabs]);

function resetTabs() {
  tabs.value = [...initialTabs];
  activeKey.value = "herta";
  fileTabs.value = [...initialFileTabs];
  fileActiveKey.value = "readme";
}

function onCloseTab(key: string) {
  tabs.value = tabs.value.filter((t) => t.key !== key);
  if (activeKey.value === key && tabs.value.length > 0) {
    activeKey.value = tabs.value[0].key;
  }
}

function onCloseFileTab(key: string) {
  fileTabs.value = fileTabs.value.filter((t) => t.key !== key);
  if (fileActiveKey.value === key && fileTabs.value.length > 0) {
    fileActiveKey.value = fileTabs.value[0].key;
  }
}
</script>
