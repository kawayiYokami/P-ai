<template>
  <div class="flex justify-center">
    <div class="w-full max-w-2xl">
      <div class="mb-2 flex flex-wrap items-center gap-2">
        <span class="text-xs text-base-content/60">容器：</span>
        <button type="button" class="btn btn-xs" :class="isFilled ? 'btn-primary' : 'btn-ghost'" @click="isFilled = !isFilled">{{ isFilled ? '灌满' : '悬浮' }}</button>
        <button type="button" class="btn btn-xs btn-primary" @click="cycleDemoContainerBg">{{ demoContainerBgLabel }}</button>
        <span class="text-xs text-base-content/40">模拟窗口矩形容器 · 底色 {{ demoContainerBgLabel }}（100→200→300 循环，底盘固定 base200）</span>
      </div>
      <div class="mb-2 flex flex-wrap items-center gap-2">
        <span class="text-xs text-base-content/60">队列：</span>
        <button type="button" class="btn btn-xs" :class="showQueue ? 'btn-primary' : 'btn-ghost'" @click="showQueue ? (showQueue = false) : (resetDemoQueue(), showQueue = true)">显隐</button>
        <button type="button" class="btn btn-xs btn-ghost" @click="addDemoQueueMessage">加入消息</button>
        <span class="text-xs text-base-content/60">输入卡：</span>
        <button type="button" class="btn btn-xs" :class="longText ? 'btn-primary' : 'btn-ghost'" @click="toggleLongText">长文本</button>
        <button type="button" class="btn btn-xs" :class="showAttachments ? 'btn-primary' : 'btn-ghost'" @click="showAttachments ? (showAttachments = false) : (resetDemoAttachments(), showAttachments = true)">附件显隐</button>
        <button type="button" class="btn btn-xs btn-ghost" @click="addDemoAttachment">加入附件</button>
        <button type="button" class="btn btn-xs" :class="showBridges ? 'btn-primary' : 'btn-ghost'" @click="showBridges ? (showBridges = false) : (resetDemoBridges(), showBridges = true)">桥显隐</button>
        <button type="button" class="btn btn-xs btn-ghost" @click="addDemoBridge">加入桥</button>
        <button type="button" class="btn btn-xs" :class="demoBusy ? 'btn-primary' : 'btn-ghost'" @click="demoBusy = !demoBusy">忙碌态</button>
      </div>
      <div class="mb-2 flex flex-wrap items-center gap-2">
        <span class="text-xs text-base-content/60">功能：</span>
        <button type="button" class="btn btn-xs" :class="demoMentions.length > 0 ? 'btn-primary' : 'btn-ghost'" @click="demoMentions.length > 0 ? (demoMentions = []) : resetDemoMentions()">提及</button>
        <button type="button" class="btn btn-xs" :class="goalActive ? 'btn-primary' : 'btn-ghost'" @click="goalActive = !goalActive">目标</button>
        <button type="button" class="btn btn-xs" :class="planModeEnabled ? 'btn-primary' : 'btn-ghost'" @click="planModeEnabled = !planModeEnabled">计划模式</button>
      </div>

      <InputPanelCanvas :bg="demoContainerBg" :is-rounded="false">
        <!-- 演示与生产共用同一块生产面板，只换假数据源 -->
        <ChatComposerPanel
          :selection-mode-enabled="false"
          :selected-message-count="0"
          :chat-input="inputText"
          :instruction-presets="demoInstructionPresets"
          :mention-entries="demoMentionEntries"
          :selected-mentions="demoMentions"
          :clipboard-images="demoClipboardImages"
          :queued-attachment-notices="demoQueuedNotices"
          link-open-error-text=""
          conversation-call-primary-api-config-id="demo-model"
          :preferred-chat-model-id="demoPreferredModelId"
          :chat-model-options="demoChatModelOptions"
          :workspace-access="demoWorkspaceAccess"
          :plan-mode-enabled="planModeEnabled"
          :chatting="false"
          frontend-round-phase="idle"
          :busy="demoBusy"
          :stop-chat-disabled="false"
          :frozen="false"
          :goal-active="goalActive"
          goal-title="演示目标"
          :goal-disabled="false"
          :system-notification-mode="false"
          :remote-contact-mode="false"
          :show-side-conversation-list="false"
          active-conversation-id="demo"
          :unarchived-conversation-items="[]"
          :remote-im-contact-conversations="[]"
          user-alias="红豆"
          user-avatar-url=""
          persona-name="纳西妲"
          :persona-name-map="demoPersonaNameMap"
          :persona-avatar-url-map="{}"
          :create-conversation-department-options="[]"
          default-create-conversation-department-id=""
          :ide-context-groups="demoIdeGroups"
          :attached-ide-context-references="demoAttachedRefs"
          :show-conversation-actions="true"
          :chat-usage-percent="0"
          active-agent-id="a1"
          :is-rounded="!isFilled"
          :queue-events-override="demoQueueEvents"
          :queue-visible="showQueue"
          @update:chat-input="inputText = $event"
          @add-mention="handleDemoAddMention"
          @remove-mention="handleDemoRemoveMention"
          @remove-clipboard-image="demoImages.splice($event, 1)"
          @remove-queued-attachment-notice="demoFiles.splice($event, 1)"
          @pick-attachments="addDemoAttachment"
          @update:conversation-preferred-api-config-id="demoPreferredModelId = $event"
          @update:workspace-access="demoWorkspaceAccess = $event"
          @update:plan-mode-enabled="planModeEnabled = $event"
          @attach-ide-context-reference="attachDemoBridge($event.id)"
          @remove-ide-context-reference="detachDemoBridge($event)"
          @send-chat="handleDemoSend"
          @stop-chat="demoBusy = false"
          @open-goal-task="goalActive = !goalActive"
          @queue-recall="handleDemoRecall"
          @queue-mark-guided="handleDemoMarkGuided"
        />
      </InputPanelCanvas>
      <p class="mt-2 text-xs text-base-content/50">生产与演示共用同一块面板，只差数据源。@提及 / 指令 / 计划 / 目标都在输入卡内直接交互。</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import ChatComposerPanel from "./ChatComposerPanel.vue";
import InputPanelCanvas from "./input-panel/InputPanelCanvas.vue";
import type {
  ApiConfigItem,
  ChatMentionEntry,
  ChatMentionTarget,
  IdeContextReferenceItem,
  IdeContextWorkspaceGroup,
} from "../../../types/app";
import type { ChatQueueEvent } from "../composables/use-chat-queue";

const inputText = ref("");
const isFilled = ref(true);
const DEMO_CONTAINER_BG_CYCLE = ["base-100", "base-200", "base-300"] as const;
type DemoContainerBg = (typeof DEMO_CONTAINER_BG_CYCLE)[number];
const demoContainerBg = ref<DemoContainerBg>("base-200");
const demoContainerBgLabel = computed(() => {
  if (demoContainerBg.value === "base-100") return "base100";
  if (demoContainerBg.value === "base-300") return "base300";
  return "base200";
});
function cycleDemoContainerBg(): void {
  const idx = DEMO_CONTAINER_BG_CYCLE.indexOf(demoContainerBg.value);
  demoContainerBg.value = DEMO_CONTAINER_BG_CYCLE[(idx + 1) % DEMO_CONTAINER_BG_CYCLE.length];
}
const showQueue = ref(true);
const longText = ref(false);
const showAttachments = ref(true);
const showBridges = ref(true);
const demoBusy = ref(false);
const demoImages = ref(makeDemoImages());
const demoFiles = ref(makeDemoFiles());
const demoBridges = ref(makeDemoBridges());
const demoMentions = ref<ChatMentionTarget[]>(makeDemoMentions());
const goalActive = ref(false);
const planModeEnabled = ref(false);
const demoPreferredModelId = ref("demo-model");
const demoWorkspaceAccess = ref<"approval" | "full_access">("approval");
const demoPersonaNameMap = ref<Record<string, string>>({ "user-persona": "红豆" });
const demoInstructions = ref([
  { id: "p1", prompt: "帮我 review 这段 diff，列风险点" },
  { id: "p2", prompt: "把这段需求拆成可执行的 todo" },
  { id: "p3", prompt: "用中文总结这段 changelog" },
]);
const mentionCandidates = ref([
  { agentId: "a1", agentName: "纳西妲", departmentName: "智慧殿" },
  { agentId: "a2", agentName: "红豆", departmentName: "用户组" },
  { agentId: "a3", agentName: "派蒙", departmentName: "向导组" },
]);

const demoInstructionPresets = computed(() =>
  demoInstructions.value.map((item) => ({ id: item.id, name: item.prompt, prompt: item.prompt })),
);

const demoMentionEntries = computed<ChatMentionEntry[]>(() =>
  mentionCandidates.value.map((item) => ({
    agentId: item.agentId,
    agentName: item.agentName,
    departmentId: "d-demo",
    departmentName: item.departmentName,
    departmentNames: [item.departmentName],
    isFrontSpeaking: false,
    hasBackgroundTask: false,
    mentionable: true,
  })),
);

const demoClipboardImages = computed(() => {
  if (!showAttachments.value) return [];
  return demoImages.value.map((img) => ({
    mime: img.mime,
    bytesBase64: "",
    previewDataUrl: img.previewDataUrl,
  }));
});

const demoQueuedNotices = computed(() => {
  if (!showAttachments.value) return [];
  return demoFiles.value.map((file) => ({
    id: file.id,
    fileName: file.fileName,
    path: file.fileName,
    mime: "text/markdown",
  }));
});

function parseDemoStartLine(lineSuffix: string): number {
  const match = String(lineSuffix || "").match(/:(\d+)/);
  return match ? Number(match[1]) : 0;
}

function toDemoIdeRef(item: { id: string; fileName: string; lineSuffix: string; title: string }): IdeContextReferenceItem {
  const startLine = parseDemoStartLine(item.lineSuffix);
  return {
    id: item.id,
    workspacePath: "demo",
    workspaceName: "demo",
    filePath: item.fileName,
    fileName: item.fileName,
    relativePath: item.fileName,
    startLine,
    endLine: startLine,
    displayLabel: item.title,
    content: "",
    languageId: "vue",
    source: "demo",
    capturedAt: new Date().toISOString(),
    textBlock: "",
  };
}

const demoIdeGroups = computed<IdeContextWorkspaceGroup[]>(() => {
  if (!showBridges.value) return [];
  return [{ workspacePath: "demo", workspaceName: "demo", references: demoBridges.value.map(toDemoIdeRef) }];
});

const demoAttachedRefs = computed<IdeContextReferenceItem[]>(() => {
  if (!showBridges.value) return [];
  return demoBridges.value.filter((item) => item.attached).map(toDemoIdeRef);
});

const demoChatModelOptions = computed<ApiConfigItem[]>(() => [
  {
    id: "demo-model",
    name: "demo-model",
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: false,
    tools: [],
    baseUrl: "",
    apiKey: "",
    model: "demo-model",
    temperature: 0.7,
    contextWindowTokens: 8000,
  },
]);

function makeDemoMentions(): ChatMentionTarget[] {
  return [{ agentId: "a1", agentName: "纳西妲", departmentId: "d1", departmentName: "智慧殿" }];
}

function resetDemoMentions(): void {
  demoMentions.value = makeDemoMentions();
}

function handleDemoAddMention(item: ChatMentionTarget): void {
  const agentId = String(item?.agentId || "").trim();
  if (!agentId) return;
  const departmentId = String(item?.departmentId || "d-demo").trim() || "d-demo";
  if (demoMentions.value.some((m) => m.agentId === agentId && m.departmentId === departmentId)) return;
  demoMentions.value = [
    ...demoMentions.value,
    {
      agentId,
      agentName: String(item?.agentName || agentId),
      departmentId,
      departmentName: String(item?.departmentName || ""),
    },
  ];
}

function handleDemoRemoveMention(value: string | { agentId: string; departmentId?: string }): void {
  if (typeof value === "string") {
    demoMentions.value = demoMentions.value.filter((m) => m.agentId !== value);
    return;
  }
  const agentId = String(value?.agentId || "").trim();
  const departmentId = String(value?.departmentId || "").trim();
  demoMentions.value = demoMentions.value.filter((m) =>
    departmentId ? !(m.agentId === agentId && m.departmentId === departmentId) : m.agentId !== agentId,
  );
}

function makeDemoImages(): Array<{ mime: string; label: string; previewDataUrl?: string }> {
  return [
    {
      mime: "image/png",
      label: "screenshot.png",
      previewDataUrl:
        "data:image/svg+xml;charset=utf-8," +
        encodeURIComponent(
          `<svg xmlns="http://www.w3.org/2000/svg" width="152" height="60"><rect width="152" height="60" rx="8" fill="#3b82f6"/><text x="12" y="36" font-size="13" fill="white">screenshot.png</text></svg>`,
        ),
    },
    { mime: "application/pdf", label: "PDF 1" },
  ];
}

function makeDemoFiles(): Array<{ id: string; fileName: string }> {
  return [
    { id: "f1", fileName: "design-notes.md" },
    { id: "f2", fileName: "meeting-note.m4a" },
  ];
}

function makeDemoBridges(): Array<{ id: string; fileName: string; lineSuffix: string; title: string; attached: boolean }> {
  return [
    { id: "b1", fileName: "ChatComposerPanel.vue", lineSuffix: ":143", title: "ChatComposerPanel.vue:143", attached: true },
    { id: "b2", fileName: "ChatView.vue", lineSuffix: ":343", title: "ChatView.vue:343", attached: false },
  ];
}

function resetDemoAttachments(): void {
  demoImages.value = makeDemoImages();
  demoFiles.value = makeDemoFiles();
}

function resetDemoBridges(): void {
  demoBridges.value = makeDemoBridges();
}

function attachDemoBridge(id: string): void {
  demoBridges.value = demoBridges.value.map((item) =>
    item.id === id ? { ...item, attached: true } : item,
  );
}

function detachDemoBridge(id: string): void {
  demoBridges.value = demoBridges.value.map((item) =>
    item.id === id ? { ...item, attached: false } : item,
  );
}

const demoQueueEvents = ref<ChatQueueEvent[]>(makeDemoQueueEvents());

function makeDemoQueueEvents(): ChatQueueEvent[] {
  const now = new Date().toISOString();
  return [
    {
      id: "demo-q1",
      source: "user",
      queueMode: "normal",
      createdAt: now,
      messagePreview: "先帮我整理 changelog 未发布条目",
      conversationId: "demo",
    },
    {
      id: "demo-q2",
      source: "user",
      queueMode: "guided",
      createdAt: now,
      messagePreview: "补齐后台任务终止的回归测试",
      conversationId: "demo",
    },
    {
      id: "demo-q3",
      source: "task",
      queueMode: "normal",
      createdAt: now,
      messagePreview: "每周整理一次 changelog 未发布条目",
      conversationId: "demo",
    },
  ];
}

function resetDemoQueue(): void {
  demoQueueEvents.value = makeDemoQueueEvents();
}

let demoQueueSeq = 0;

function addDemoQueueMessage(): void {
  demoQueueSeq += 1;
  demoQueueEvents.value = [
    ...demoQueueEvents.value,
    {
      id: `demo-q-added-${demoQueueSeq}`,
      source: "user",
      queueMode: "normal",
      createdAt: new Date().toISOString(),
      messagePreview: `新排队消息 ${demoQueueSeq}`,
      conversationId: "demo",
    },
  ];
  showQueue.value = true;
}

let demoAttachmentSeq = 0;

function addDemoAttachment(): void {
  demoAttachmentSeq += 1;
  demoFiles.value = [
    ...demoFiles.value,
    { id: `f-added-${demoAttachmentSeq}`, fileName: `new-file-${demoAttachmentSeq}.md` },
  ];
  showAttachments.value = true;
}

let demoBridgeSeq = 0;

function addDemoBridge(): void {
  demoBridgeSeq += 1;
  demoBridges.value = [
    ...demoBridges.value,
    {
      id: `b-added-${demoBridgeSeq}`,
      fileName: `NewFile${demoBridgeSeq}.vue`,
      lineSuffix: `:${demoBridgeSeq}`,
      title: `NewFile${demoBridgeSeq}.vue:${demoBridgeSeq}`,
      attached: false,
    },
  ];
  showBridges.value = true;
}

function handleDemoRecall(event: ChatQueueEvent): void {
  demoQueueEvents.value = demoQueueEvents.value.filter((item) => item.id !== event.id);
  const text = String((event as { messageText?: string }).messageText || event.messagePreview || "").trim();
  if (text) inputText.value = text;
}

function handleDemoMarkGuided(eventId: string): void {
  demoQueueEvents.value = demoQueueEvents.value.map((item) =>
    item.id === eventId ? { ...item, queueMode: "guided" } : item,
  );
}

const SHORT_TEXT = "";
const LONG_TEXT = Array.from({ length: 8 }, (_, i) => `第 ${i + 1} 行演示文本，用于撑高上层卡并观察底层卡动画。`).join("\n");

function toggleLongText(): void {
  longText.value = !longText.value;
  inputText.value = longText.value ? LONG_TEXT : SHORT_TEXT;
}

function handleDemoSend(): void {
  if (!inputText.value.trim() || demoBusy.value) return;
  demoBusy.value = true;
  inputText.value = "";
}
</script>
