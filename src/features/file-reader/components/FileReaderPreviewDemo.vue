<template>
  <div class="grid gap-3">
    <div class="file-reader-preview-demo-switch flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">视图：</span>
      <button
        type="button"
        class="btn btn-xs"
        :class="view === 'html' ? 'btn-primary' : 'btn-ghost'"
        @click="view = 'html'"
      >HTML 沙箱渲染</button>
      <button
        type="button"
        class="btn btn-xs"
        :class="view === 'source' ? 'btn-primary' : 'btn-ghost'"
        @click="view = 'source'"
      >同文件源码</button>
      <button
        type="button"
        class="btn btn-xs"
        :class="view === 'image' ? 'btn-primary' : 'btn-ghost'"
        @click="view = 'image'"
      >聊天图片</button>
    </div>

    <div class="file-reader-preview-stage h-[520px] overflow-hidden rounded-lg border border-base-content/10 bg-base-100">
      <FileReaderHtmlPreview
        v-if="view === 'html'"
        :code="htmlSource"
        :is-dark="isDark"
        title-text="demo.html"
        loading-text="正在准备沙箱"
        reload-title="重新运行"
      />
      <FileReaderRawPreview
        v-else-if="view === 'source'"
        :content="htmlSource"
        wrap
      />
      <FileReaderMediaPreview
        v-else
        kind="image"
        :src="chatImageDataUrl"
        title="聊天图片"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import FileReaderHtmlPreview from "./FileReaderHtmlPreview.vue";
import FileReaderMediaPreview from "./FileReaderMediaPreview.vue";
import FileReaderRawPreview from "./FileReaderRawPreview.vue";

/**
 * 右栏 reader 的三类内容视图 demo。
 *
 * 这里挂的就是面板里用的那三个组件，不是另写的复刻；
 * html 源里带 script 与 style，能直接看出脚本在沙箱里真跑起来了。
 */
const props = withDefaults(defineProps<{ isDark?: boolean }>(), { isDark: undefined });

const view = ref<"html" | "source" | "image">("html");

/** 没显式传深色时按宿主主题猜一次，保证沙箱内的主题变量与面板一致 */
const isDark = computed(() => {
  if (typeof props.isDark === "boolean") return props.isDark;
  if (typeof document === "undefined") return false;
  const root = document.documentElement;
  const themeId = `${root.getAttribute("data-theme") || ""} ${root.className || ""}`;
  return /dark/i.test(themeId);
});

const htmlSource = `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <title>沙箱里跑起来的页面</title>
  <style>
    body { margin: 0; font-family: system-ui, sans-serif; background: #0f172a; color: #e2e8f0; }
    .wrap { padding: 24px; display: grid; gap: 16px; }
    .title { font-size: 18px; font-weight: 600; }
    .row { display: flex; align-items: center; gap: 12px; }
    button { border: 0; border-radius: 10px; padding: 8px 14px; font-size: 14px;
             background: #4f46e5; color: #fff; cursor: pointer; }
    button:active { transform: translateY(1px); }
    .badge { display: inline-block; padding: 4px 10px; border-radius: 999px;
             background: #1e293b; color: #93c5fd; font-variant-numeric: tabular-nums; }
    .gradient { height: 72px; border-radius: 12px;
                background: linear-gradient(120deg, #6366f1, #06b6d4 55%, #22c55e); }
  </style>
</head>
<body>
  <div class="wrap">
    <div class="title">这个页面由沙箱内的脚本渲染</div>
    <div class="gradient"></div>
    <div class="row">
      <button id="hit" type="button">点我</button>
      <span class="badge" id="count">计数 0</span>
    </div>
    <div class="badge" id="probe">脚本尚未执行</div>
  </div>
  <script>
    var hits = 0;
    var count = document.getElementById('count');
    document.getElementById('hit').addEventListener('click', function () {
      hits += 1;
      count.textContent = '计数 ' + hits;
    });
    document.getElementById('probe').textContent =
      '脚本已执行，样式表规则数 ' + document.styleSheets[0].cssRules.length;
  <\/script>
</body>
</html>`;

/** 聊天图片走的是内联 dataUrl，与远程图/本地图在右栏是同一条路 */
const chatImageDataUrl = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400" viewBox="0 0 640 400">
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#6366f1"/><stop offset="1" stop-color="#22c55e"/>
      </linearGradient>
    </defs>
    <rect width="640" height="400" fill="url(#g)"/>
    <circle cx="180" cy="200" r="86" fill="rgba(255,255,255,0.35)"/>
    <circle cx="420" cy="150" r="56" fill="rgba(255,255,255,0.22)"/>
    <text x="320" y="352" font-family="system-ui" font-size="24" fill="#ffffff"
          text-anchor="middle">聊天里收到的图片</text>
  </svg>`,
)}`;
</script>
