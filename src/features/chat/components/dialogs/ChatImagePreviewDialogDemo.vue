<template>
  <div class="flex flex-col gap-2 p-2">
    <div class="flex flex-wrap items-center gap-2 text-xs">
      <button v-for="value in zoomSteps" :key="value" type="button" class="btn btn-xs" @click="setZoom(value)">
        {{ Math.round(value * 100) }}%
      </button>
      <button type="button" class="btn btn-xs btn-primary" @click="runAll">跑读数</button>
      <span class="text-base-content/60">{{ statusText }}</span>
    </div>

    <div class="relative h-[420px] overflow-hidden rounded-box border border-base-300 bg-base-200">
      <ChatImagePreviewDialog
        :open="true"
        :data-url="DATA_URL"
        :zoom="zoom"
        :min-zoom="0.25"
        :max-zoom="8"
        :offset-x="0"
        :offset-y="0"
        :rotation="0"
        :dragging="false"
        @close="noop('close')"
        @zoom-in="noop('zoomIn')"
        @zoom-out="noop('zoomOut')"
        @reset="handleReset"
        @rotate="noop('rotate')"
        @wheel="noop('wheel')"
        @pointer-down="noop('pointerDown')"
        @pointer-move="noop('pointerMove')"
        @pointer-up="noop('pointerUp')"
        @copy-image="noop('copyImage')"
        @save-image="noop('saveImage')"
        @open-in-right-panel="noop('openInRightPanel')"
      />
    </div>

    <pre class="demo-measure max-h-40 overflow-auto rounded bg-base-200 p-2 text-[11px] leading-4">{{ reportText }}</pre>
    <p class="text-xs text-base-content/60">
      <span class="font-medium text-base-content/80">验的是：</span>底部百分比读数是否与当前缩放一致——非整数倍按取整显示、
      读数 100% 时按钮必须禁用、点按钮是否复位到 100%。真实 ChatImagePreviewDialog，zoom 由外部喂（等同真实链路的
      缩放状态），量与图片无关，所以图片用内联 SVG，不发请求。
    </p>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import ChatImagePreviewDialog from "./ChatImagePreviewDialog.vue";

const DATA_URL =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160"><rect width="240" height="160" fill="#1f2937"/><circle cx="120" cy="80" r="52" fill="#60a5fa"/></svg>',
  );

const zoomSteps = [0.5, 1, 1.37, 1.5, 2];
const zoom = ref(1);
const statusText = ref("待读数");
const reportText = ref("");

function noop(name: string) {
  void name;
}

function setZoom(value: number) {
  zoom.value = value;
  statusText.value = `zoom=${value}`;
}

function handleReset() {
  zoom.value = 1;
  statusText.value = "已点击百分比按钮 → 复位 100%";
}

function percentButton(): HTMLButtonElement | null {
  return document.querySelector<HTMLButtonElement>(".ecall-image-preview-modal button[title*='当前']");
}

async function readButton() {
  await nextTick();
  const button = percentButton();
  return {
    zoom: Math.round(zoom.value * 100) / 100,
    label: button ? button.textContent?.trim() : null,
    disabled: button ? button.disabled : null,
    title: button ? button.getAttribute("title") : null,
  };
}

async function runAll() {
  const lines: string[] = [];
  for (const value of [0.5, 1, 1.37, 1.5, 2]) {
    zoom.value = value;
    const snapshot = await readButton();
    lines.push(`zoom=${value} → ${JSON.stringify(snapshot)}`);
  }
  zoom.value = 1.37;
  await nextTick();
  percentButton()?.click();
  await nextTick();
  lines.push(`点击百分比按钮后 → ${JSON.stringify(await readButton())}`);
  reportText.value = lines.join("\n");
  statusText.value = "读数完成";
}

onMounted(() => {
  (window as unknown as { __imageDemo?: unknown }).__imageDemo = {
    setZoom,
    readButton,
    runAll,
    clickPercent: () => percentButton()?.click(),
    report: () => reportText.value,
  };
});
</script>
