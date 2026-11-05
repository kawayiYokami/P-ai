<script setup lang="ts">
import { computed, ref } from "vue";
import type { CatalogEntry } from "../../../types/app";
import CatalogEntryCard from "../views/config-tabs/CatalogEntryCard.vue";

// 喂的是商店里真实存在的条目形态：有图标的、没图标的、已装的、装不了的。
const entries = computed<CatalogEntry[]>(() => [
  {
    id: "magpie-community/plugins/setup",
    name: "magpie-setup",
    description:
      "Set magpie up as you ask, through its own commands: add keys and subscriptions, point it at a library, and keep everything else untouched.",
    kind: "skill",
    source: "clawhub",
    categories: [],
    popularity: 12400,
    author: "magpie-community/plugins",
    homepage: "",
    icon: "",
    transport: "",
    definitionJson: "",
    requiredEnv: [],
    tools: [],
    detailUrl: "",
    installReady: true,
    installed: false,
    enabled: false,
    localId: "",
  },
  {
    id: "obra/superpowers/using-git-worktrees",
    name: "using-git-worktrees",
    description: "用工作树把并行的改动隔开，各自一个目录，互不覆盖。",
    kind: "skill",
    source: "modelscope-skill",
    categories: [],
    popularity: 872,
    author: "obra/superpowers",
    homepage: "",
    icon: "",
    transport: "",
    definitionJson: "",
    requiredEnv: [],
    tools: [],
    detailUrl: "",
    installReady: true,
    installed: true,
    enabled: true,
    localId: "using-git-worktrees",
  },
  {
    id: "ai.modelcontextprotocol.registry/amap",
    name: "amap-maps",
    description: "",
    kind: "mcp",
    source: "official-registry",
    categories: [],
    popularity: 0,
    author: "AMap-Web",
    homepage: "",
    icon: "",
    transport: "stdio",
    definitionJson: "",
    requiredEnv: ["AMAP_KEY"],
    tools: [],
    detailUrl: "",
    installReady: false,
    installed: false,
    enabled: false,
    localId: "",
  },
]);

const cardWidth = ref(360);
const events = ref<string[]>([]);

function record(text: string) {
  events.value = [`${new Date().toLocaleTimeString()} ${text}`, ...events.value].slice(0, 6);
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap items-center gap-3 text-xs">
      <label class="flex items-center gap-1.5">
        <span class="whitespace-nowrap opacity-60">卡宽</span>
        <select v-model.number="cardWidth" class="select select-bordered select-xs">
          <option :value="300">300</option>
          <option :value="360">360</option>
          <option :value="440">440</option>
        </select>
      </label>
      <span class="opacity-50">交互回调只记录，不落库</span>
    </div>

    <div
      class="grid gap-3"
      :style="{ gridTemplateColumns: `repeat(auto-fill, minmax(min(100%, ${cardWidth}px), 1fr))` }"
    >
      <div v-for="entry in entries" :key="entry.id">
        <CatalogEntryCard
          :entry="entry"
          icon-url=""
          @open="record(`打开详情：${entry.name}`)"
          @install="record(`安装：${entry.name}`)"
          @toggle="record(`切换启用：${entry.name}`)"
          @icon-error="record(`图标加载失败：${entry.name}`)"
        />
      </div>
    </div>

    <div v-if="events.length" class="rounded-box border border-base-300 bg-base-200/40 p-3 text-caption">
      <div v-for="line in events" :key="line" class="font-mono opacity-70">{{ line }}</div>
    </div>

    <p class="text-caption opacity-60">
      喂了三条真实形态的条目：普通可装、已安装带开关、不可装且无说明。图标统一留空，走首字母兜底。
    </p>
  </div>
</template>
