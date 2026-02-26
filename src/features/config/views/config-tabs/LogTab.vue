<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-sm font-medium">调用日志（内存）</div>
      <div class="join">
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 join-item" :disabled="loading" @click="reload">刷新</button>
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 join-item" :disabled="loading || logs.length === 0" @click="clearAll">清空</button>
      </div>
    </div>

    <div class="text-sm opacity-60">仅保留最近 10 轮发送-接收日志，进程退出后自动清空。</div>

    <div v-if="loading" class="text-sm opacity-70">加载中...</div>
    <div v-else-if="logs.length === 0" class="text-sm opacity-50">暂无日志</div>

    <div v-else class="space-y-2">
      <div
        v-for="entry in logs"
        :key="entry.id"
        class="card bg-base-100 border border-base-300"
      >
        <div class="card-body p-3 space-y-2">
          <div class="flex items-center justify-between gap-2">
            <div class="text-sm opacity-70 break-all">
              {{ entry.createdAt }} | {{ entry.scene }} | {{ entry.provider }} | {{ entry.requestFormat }} | {{ entry.model }}
            </div>
            <div class="badge badge-sm" :class="entry.success ? 'badge-success' : 'badge-error'">
              {{ entry.success ? "成功" : "失败" }}
            </div>
          </div>

          <div class="text-sm opacity-70">
            耗时 {{ entry.elapsedMs }}ms | {{ entry.baseUrl || "-" }}
          </div>

          <details class="collapse collapse-arrow bg-base-200 border border-base-300">
            <summary class="collapse-title text-sm py-2 min-h-0">Headers</summary>
            <div class="collapse-content text-sm">
              <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.headers) }}</pre>
            </div>
          </details>

          <details class="collapse collapse-arrow bg-base-200 border border-base-300">
            <summary class="collapse-title text-sm py-2 min-h-0">Tools</summary>
            <div class="collapse-content text-sm">
              <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.tools ?? null) }}</pre>
            </div>
          </details>

          <details class="collapse collapse-arrow bg-base-200 border border-base-300">
            <summary class="collapse-title text-sm py-2 min-h-0">Request</summary>
            <div class="collapse-content text-sm">
              <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.request) }}</pre>
            </div>
          </details>

          <details class="collapse collapse-arrow bg-base-200 border border-base-300">
            <summary class="collapse-title text-sm py-2 min-h-0">Response</summary>
            <div class="collapse-content text-sm">
              <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.response ?? null) }}</pre>
            </div>
          </details>

          <div v-if="entry.error" class="text-sm text-error break-all">
            {{ entry.error }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invokeTauri } from "../../../../services/tauri-api";
import type { LlmRoundLogEntry } from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";

const loading = ref(false);
const logs = ref<LlmRoundLogEntry[]>([]);

function toPretty(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input ?? "");
  }
}

async function reload() {
  loading.value = true;
  try {
    const list = await invokeTauri<LlmRoundLogEntry[]>("list_recent_llm_round_logs");
    logs.value = [...list].reverse();
  } catch (error) {
    logs.value = [
      {
        id: "error",
        createdAt: new Date().toISOString(),
        scene: "ui",
        requestFormat: "-",
        provider: "-",
        model: "-",
        baseUrl: "",
        headers: [],
        request: null,
        tools: null,
        response: null,
        error: toErrorMessage(error),
        elapsedMs: 0,
        success: false,
      },
    ];
  } finally {
    loading.value = false;
  }
}

async function clearAll() {
  loading.value = true;
  try {
    await invokeTauri<boolean>("clear_recent_llm_round_logs");
    logs.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void reload();
});
</script>
