<template>
  <!--
    「快速模型不可用」卡片的 demo：喂真实的失败文案与模型清单，两个宿主长同一个样子。
    卡片本身是 dialog 浮层，会盖在下面的宿主替身上，看起来就是真机上弹出时的观感。
  -->
  <div
    class="relative flex h-[720px] flex-col overflow-hidden rounded-box border border-base-300 bg-base-100"
    :style="{ width: `min(${demoWidth}px, 100%)` }"
  >
    <div class="flex flex-wrap items-center gap-3 border-b border-base-300 px-3 py-2 text-xs">
      <label class="flex items-center gap-1">
        宽度
        <select v-model.number="demoWidth" class="select select-bordered select-xs">
          <option :value="420">420</option>
          <option :value="560">560</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        宿主
        <select v-model="demoHost" class="select select-bordered select-xs">
          <option value="chat">聊天窗口（招募人格）</option>
          <option value="config">配置窗口（保存人格）</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        错误档位
        <select v-model="demoCase" class="select select-bordered select-xs">
          <option value="server">502 服务端错误</option>
          <option value="auth">401 鉴权失败</option>
          <option value="timeout">超时</option>
          <option value="unknown">未知长文本</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        当前快速模型
        <select v-model="demoModelId" class="select select-bordered select-xs">
          <option value="">不配置</option>
          <option value="demo-fast">gpt-6-astra</option>
        </select>
      </label>
    </div>

    <div class="min-h-0 flex-1 space-y-3 overflow-hidden p-4 text-sm text-base-content/60">
      <p>{{ demoHostText }}</p>
      <p>卡片本身是 dialog 浮层，落在哪个窗口由后端事件的 origin 决定，两处用的是同一份组件。</p>
    </div>

    <p class="border-t border-base-300 px-3 py-2 text-xs text-base-content/50">
      交互说明：下拉里选到「不配置」不会触发任何动作；选一个具体模型会保存快速模型并重试一次头像生成——demo 里两个回调都只打印、不写库。
    </p>

    <QuickModelUnavailableCard
      :open="true"
      :error="demoError"
      :api-configs="demoApiConfigs"
      :model-value="demoModelId"
      @select="onDemoSelect"
      @close="onDemoClose"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { ApiConfigItem } from "../../../types/app";
import QuickModelUnavailableCard from "./QuickModelUnavailableCard.vue";

const demoWidth = ref(560);
// 截图时用 ?host=config 指定宿主，手动点的话下拉也能切。
const demoHost = ref<"chat" | "config">(
  new URLSearchParams(window.location.search).get("host") === "config" ? "config" : "chat",
);
const demoCase = ref<"server" | "auth" | "timeout" | "unknown">("server");
const demoModelId = ref("demo-fast");

const demoHostText = computed(() =>
  demoHost.value === "chat"
    ? "这里是聊天窗口的替身：招募新人格时，头像生成失败就在这个窗口弹卡片。"
    : "这里是配置窗口的替身：保存人格时，头像生成失败就在这个窗口弹卡片。",
);

/** 真实错误原文：卡片要做的就是把它压成一句人话。 */
const DEMO_ERRORS: Record<typeof demoCase.value, string> = {
  server:
    `GenAI 流式收集失败：WebStream { model_iden: ModelIden { adapter_kind: OpenAIResp, model_name: ModelName(Shared("gpt-6-astra")) }, cause: "HTTP error.\\nStatus: 502 Bad Gateway\\nBody: upstream connect error" }`,
  auth: `GenAI 流式收集失败：WebStream { cause: "HTTP error.\\nStatus: 401 Unauthorized\\nBody: {\\"error\\":\\"invalid api key\\"}" }`,
  timeout: `GenAI 流式收集失败：request timed out after 60s while streaming chat completion`,
  unknown:
    `GenAI 流式收集失败：WebStream { cause: "unexpected handshake failure at 0x7ffd3a2c, adapter=OpenAIResp, retries=3, last_state=ConnectingToUpstream, detail=流式连接在握手阶段被对端关闭且未返回任何响应头" }`,
};

const demoError = computed(() => DEMO_ERRORS[demoCase.value]);

const demoApiConfigs: ApiConfigItem[] = [
  {
    id: "demo-fast",
    name: "快速模型",
    requestFormat: "openai_responses",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: false,
    tools: [],
    baseUrl: "https://api.example.com/v1",
    apiKey: "",
    model: "gpt-6-astra",
    displayName: "gpt-6-astra",
    temperature: 0.7,
    contextWindowTokens: 128000,
  },
  {
    id: "demo-fast-2",
    name: "备用快速模型",
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: false,
    tools: [],
    baseUrl: "https://api2.example.com/v1",
    apiKey: "",
    model: "deepseek-v4.1-flash",
    displayName: "deepseek-v4.1-flash",
    temperature: 0.7,
    contextWindowTokens: 64000,
  },
];

function onDemoSelect(modelId: string) {
  // demo 不写库、不触发重试，只把选择结果停在界面上。
  demoModelId.value = modelId;
  console.info("[demo] 选中快速模型：", modelId);
}

function onDemoClose() {
  console.info("[demo] 关闭卡片");
}
</script>
