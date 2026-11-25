<template>
  <div class="space-y-4">
    <GalleryControls>
      <span class="text-xs text-base-content/70">当前表达式</span>
      <code class="rounded bg-base-200 px-2 py-1 font-mono text-xs">{{ value }}</code>
      <button
        v-for="preset in presets"
        :key="preset.expression"
        type="button"
        class="btn btn-ghost btn-xs"
        @click="value = preset.expression"
      >
        {{ preset.label }}
      </button>
    </GalleryControls>

    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-box border border-base-300 bg-base-100 p-4">
        <div class="mb-3 text-xs font-medium text-base-content/70">点选结果会实时出现在上方与下方的表达式里</div>
        <CronPicker v-model="value" />
      </div>

      <div class="space-y-3">
        <div class="rounded-box border border-base-300 bg-base-100 p-4">
          <div class="mb-2 text-xs font-medium text-base-content/70">宿主拿到的值</div>
          <code class="block break-all rounded bg-base-200 px-2 py-1.5 font-mono text-xs">{{ value }}</code>
          <div class="mt-2 text-xs text-base-content/55">
            组件只吐字符串，不额外维护第二套状态；存进库、传给后端都是这一个值。
          </div>
        </div>

        <div class="rounded-box border border-base-300 bg-base-100 p-4">
          <div class="mb-2 text-xs font-medium text-base-content/70">禁用态</div>
          <CronPicker :model-value="value" disabled />
        </div>

        <div class="rounded-box border border-base-300 bg-base-100 p-4">
          <div class="mb-2 text-xs font-medium text-base-content/70">窄容器（窄屏下会不会挤出可点区域）</div>
          <div class="w-[300px] max-w-full rounded-box border border-base-300 p-3">
            <CronPicker v-model="value" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import CronPicker from "./CronPicker.vue";
import GalleryControls from "../../dev-gallery/GalleryControls.vue";

const value = ref("0 9 * * *");

const presets = [
  { label: "每天 9 点", expression: "0 9 * * *" },
  { label: "每 5 分钟", expression: "*/5 * * * *" },
  { label: "工作日 9 点", expression: "0 9 * * 1,2,3,4,5" },
  { label: "每周一 8 点半", expression: "30 8 * * 1" },
  { label: "每月 1 日 0 点", expression: "0 0 1 * *" },
];
</script>
