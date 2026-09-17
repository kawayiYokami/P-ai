<template>
  <div class="h-80 overflow-hidden rounded-box border border-base-300">
    <SettingsPageShell
      v-model:tab="activeTab"
      :tabs="demoTabs"
      :breadcrumb="demoBreadcrumb"
    >
      <template #actions>
        <button class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3" type="button">
          <RefreshCw class="h-4 w-4" />
          <span>刷新</span>
        </button>
      </template>

      <div class="grid gap-2">
        <div class="text-xs opacity-60">当前标签：{{ activeLabel }}（切换时整块按标签顺序左右进出）</div>
        <div
          v-for="row in 8"
          :key="row"
          class="rounded-box border border-base-300 bg-base-100 px-3 py-2 text-xs"
        >
          {{ activeLabel }} 第 {{ row }} 行内容
        </div>
      </div>
    </SettingsPageShell>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { RefreshCw } from "@lucide/vue";
import SettingsPageShell from "./SettingsPageShell.vue";
import type { UnderlineTabItem } from "./UnderlineTabs.vue";

const activeTab = ref("overview");

const demoTabs: UnderlineTabItem[] = [
  { key: "overview", label: "概览" },
  { key: "connector", label: "连接器", badge: 3 },
  { key: "skill", label: "技能" },
  { key: "remote", label: "联系人", badge: 12 },
];

const demoBreadcrumb = [{ label: "能力商店" }];

const activeLabel = computed(() => demoTabs.find((tab) => tab.key === activeTab.value)?.label || "");
</script>
