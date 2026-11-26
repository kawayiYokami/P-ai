<template>
  <div class="flex justify-center">
    <div class="w-full max-w-2xl">
      <div class="mb-2 space-y-2">
      <GalleryControls>
        <span class="text-xs text-base-content/60">容器：</span>
        <button type="button" class="btn btn-xs" :class="isFilled ? 'btn-primary' : 'btn-ghost'" @click="isFilled = !isFilled">{{ isFilled ? '灌满' : '悬浮' }}</button>
        <button type="button" class="btn btn-xs btn-primary" @click="cycleDemoContainerBg">{{ demoContainerBgLabel }}</button>
        <span class="text-xs text-base-content/40">模拟窗口矩形容器 · 底色 {{ demoContainerBgLabel }}（100→200→300 循环，底盘固定 base200）</span>
      </GalleryControls>
      <GalleryControls>
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
      </GalleryControls>
      <GalleryControls>
        <span class="text-xs text-base-content/60">提问卡：</span>
        <button id="demo-question-off" type="button" class="btn btn-xs" :class="demoQuestionMode === 'off' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionMode = 'off'">无提问</button>
        <button id="demo-question-expanded" type="button" class="btn btn-xs" :class="demoQuestionMode === 'expanded' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionMode = 'expanded'">展开</button>
        <button id="demo-question-collapsed" type="button" class="btn btn-xs" :class="demoQuestionMode === 'collapsed' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionMode = 'collapsed'">收起</button>
        <span class="text-xs text-base-content/40">提问卡与普通输入在同一张输入卡里切换，右下角问题图标切换</span>
      </GalleryControls>
      <GalleryControls>
        <span class="text-xs text-base-content/60">功能：</span>
        <button type="button" class="btn btn-xs" :class="demoMentions.length > 0 ? 'btn-primary' : 'btn-ghost'" @click="demoMentions.length > 0 ? (demoMentions = []) : resetDemoMentions()">提及</button>
        <button type="button" class="btn btn-xs" :class="goalActive ? 'btn-primary' : 'btn-ghost'" @click="goalActive = !goalActive">目标</button>
        <button type="button" class="btn btn-xs" :class="planModeEnabled ? 'btn-primary' : 'btn-ghost'" @click="planModeEnabled = !planModeEnabled">计划模式</button>
        <button type="button" class="btn btn-xs" :class="demoWheelchairSuggestionKind !== 'off' ? 'btn-primary' : 'btn-ghost'" @click="cycleDemoWheelchairSuggestion">建议：{{ demoWheelchairSuggestionLabel }}</button>
      </GalleryControls>
      </div>

      <!-- 假消息：模拟 ChatView 里输入面板上方那段会话 -->
      <div class="w-full overflow-hidden border border-base-300">
      <div class="w-full space-y-3 p-4">
          <div class="chat chat-start">
            <div class="chat-image avatar placeholder">
              <div class="w-8 rounded-full bg-base-300"></div>
            </div>
            <div class="chat-bubble text-sm">帮我看一下这个输入面板的间距对不对。</div>
          </div>
          <div class="chat chat-end">
            <div class="chat-bubble chat-bubble-primary text-sm">间距没问题，露头和面卡的层次也正常。</div>
          </div>
          <div class="chat chat-start">
            <div class="chat-image avatar placeholder">
              <div class="w-8 rounded-full bg-base-300"></div>
            </div>
            <div class="chat-bubble text-sm">那我把附件和指令都挂上去试试。</div>
          </div>
          <div class="chat chat-end">
            <div class="chat-bubble chat-bubble-primary text-sm">好，挂上之后再看一眼底卡颜色。</div>
          </div>
        </div>
        <div class="shrink-0 bg-base-200 p-0">
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
          :wheelchair-mode-enabled="demoWheelchairModeEnabled"
          :wheelchair-suggestion="demoWheelchairSuggestion"
          :chatting="false"
          frontend-round-phase="idle"
          :busy="demoBusy"
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
          :create-conversation-agent-options="[]"
          default-create-conversation-agent-id=""
          :ide-context-groups="demoIdeGroups"
          :attached-ide-context-references="demoAttachedRefs"
          :kanban-issues="demoKanbanIssues"
          :attached-issue-references="kanbanIssueReferences"
          :show-conversation-actions="true"
          :chat-usage-percent="0"
          active-agent-id="a1"
          :is-rounded="!isFilled"
          :queue-events-override="demoQueueEvents"
          :queue-visible="showQueue"
          :question-active="demoQuestionMode !== 'off'"
          :question-expanded="demoQuestionMode === 'expanded'"
          @toggle-question="demoQuestionMode = demoQuestionMode === 'expanded' ? 'collapsed' : 'expanded'"
          @update:chat-input="inputText = $event"
          @add-mention="handleDemoAddMention"
          @remove-mention="handleDemoRemoveMention"
          @remove-clipboard-image="demoImages.splice($event, 1)"
          @remove-queued-attachment-notice="demoFiles.splice($event, 1)"
          @pick-attachments="addDemoAttachment"
          @update:conversation-preferred-api-config-id="demoPreferredModelId = $event"
          @update:workspace-access="demoWorkspaceAccess = $event"
          @update:plan-mode-enabled="planModeEnabled = $event"
          @update:wheelchair-mode-enabled="demoWheelchairModeEnabled = $event"
          @dismiss-wheelchair-suggestion="demoWheelchairSuggestionKind = 'off'"
          @attach-ide-context-reference="attachDemoBridge($event.id)"
          @remove-ide-context-reference="detachDemoBridge($event)"
          @attach-kanban-issue-reference="attachKanbanIssueReference"
          @remove-kanban-issue-reference="removeKanbanIssueReference"
          @send-chat="handleDemoSend"
          @stop-chat="demoBusy = false"
          @open-goal-task="goalActive = !goalActive"
          @queue-recall="handleDemoRecall"
          @queue-mark-guided="handleDemoMarkGuided"
        >
          <template #question>
            <ChatQuestionPanel
              :items="demoQuestionItems"
              v-model="demoQuestionAnswers"
              submit-mode="manual"
              @submit="demoQuestionAnswers = {}"
              @reject="demoQuestionAnswers = {}"
              @collapse="demoQuestionMode = 'collapsed'"
            />
          </template>
        </ChatComposerPanel>
      </InputPanelCanvas>
        </div>
      </div>
      <p class="mt-2 text-xs text-base-content/50">生产与演示共用同一块面板，只差数据源。@提及 / 指令 / 计划 / 目标都在输入卡内直接交互。</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import ChatComposerPanel from "./ChatComposerPanel.vue";
import ChatQuestionPanel from "./ChatQuestionPanel.vue";
import type {
  QuestionAnswer,
  QuestionItem,
} from "./chat-question-panel-types";
import InputPanelCanvas from "./input-panel/InputPanelCanvas.vue";
import GalleryControls from "../../dev-gallery/GalleryControls.vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import { useKanbanIssueReferences } from "../composables/use-kanban-issue-references";
import type { KanbanIssue } from "../composables/use-kanban-board";
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
/** 提问卡演示：展开/收起只是输入卡内部内容形态的切换，外框不动。 */
const demoQuestionMode = ref<"off" | "expanded" | "collapsed">("expanded");
const demoQuestionAnswers = ref<Record<string, QuestionAnswer>>({});
const demoQuestionItems: QuestionItem[] = [
  {
    id: "1",
    title: "发布前要不要先补一条回归测试？",
    description: "改动已经全绿，但这次动的是**提问卡容器**内部的内容形态。",
    options: [
      { id: "yes", label: "先补测试", kind: "direct" },
      { id: "no", label: "直接发", kind: "direct" },
    ],
  },
  {
    id: "2",
    title: "用哪个分支发布？",
    options: [
      { id: "main", label: "main", kind: "direct" },
      { id: "release", label: "release", kind: "direct" },
    ],
  },
];
/** 议题引用演示：磁贴与输入框共用这一份 store，和生产里一样只走一个入口。 */
const demoKanbanIssues = ref<KanbanIssue[]>([
  {
    number: 3,
    title: "看板磁贴尺寸定稿",
    status: "in_review",
    assignee: "纳西妲",
    note: "红豆要看两种宽度",
  },
  {
    number: 7,
    title: "输入框 # 引用的过滤规则",
    status: "todo",
    assignee: "哥伦比娅",
    note: "",
  },
  {
    number: 12,
    title: "引用块在消息里的展示",
    status: "in_progress",
    assignee: "桑多涅",
    note: "",
  },
]);
const {
  references: kanbanIssueReferences,
  attachKanbanIssueReference,
  removeKanbanIssueReference,
} = useKanbanIssueReferences();
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
const WHEELCHAIR_SUGGESTION_CYCLE = ["off", "suggest", "options"] as const;
type DemoWheelchairSuggestionKind = (typeof WHEELCHAIR_SUGGESTION_CYCLE)[number];
const demoWheelchairSuggestionKind = ref<DemoWheelchairSuggestionKind>("off");
const demoWheelchairModeEnabled = ref(false);
const demoWheelchairSuggestionLabel = computed(() => {
  if (demoWheelchairSuggestionKind.value === "suggest") return "单条";
  if (demoWheelchairSuggestionKind.value === "options") return "多选";
  return "关闭";
});
const demoWheelchairSuggestion = computed<
  { type: "suggest" | "options"; text: string; options: string[] } | null
>(() => {
  if (demoWheelchairSuggestionKind.value === "suggest") {
    return {
      type: "suggest",
      text: "下次可以说是「随便调个工具，别跟上次一样」，这样能直接命中观测点",
      options: [],
    };
  }
  if (demoWheelchairSuggestionKind.value === "options") {
    return {
      type: "options",
      text: "",
      options: ["继续处理下一步", "换个思路试试", "先看一下当前状态", "把这段拆成 todo"],
    };
  }
  return null;
});

function cycleDemoWheelchairSuggestion(): void {
  const idx = WHEELCHAIR_SUGGESTION_CYCLE.indexOf(demoWheelchairSuggestionKind.value);
  const next = WHEELCHAIR_SUGGESTION_CYCLE[(idx + 1) % WHEELCHAIR_SUGGESTION_CYCLE.length];
  demoWheelchairSuggestionKind.value = next;
  demoWheelchairModeEnabled.value = next !== "off";
}
const demoPreferredModelId = ref("demo-model");
const demoWorkspaceAccess = ref<"approval" | "full_access">("approval");
const demoPersonaNameMap = ref<Record<string, string>>({ "user-persona": "红豆" });
const demoInstructions = ref([
  { id: "p1", prompt: "帮我 review 这段 diff，列风险点" },
  { id: "p2", prompt: "把这段需求拆成可执行的 todo" },
  { id: "p3", prompt: "用中文总结这段 changelog" },
]);
const mentionCandidates = ref([
  { agentId: "a1", agentName: "纳西妲" },
  { agentId: "a2", agentName: "红豆" },
  { agentId: "a3", agentName: "派蒙" },
]);

const demoInstructionPresets = computed(() =>
  demoInstructions.value.map((item) => ({ id: item.id, name: item.prompt, prompt: item.prompt })),
);

const demoMentionEntries = computed<ChatMentionEntry[]>(() =>
  mentionCandidates.value.map((item) => ({
    agentId: item.agentId,
    agentName: item.agentName,
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
  return [{ agentId: "a1", agentName: "纳西妲" }];
}

function resetDemoMentions(): void {
  demoMentions.value = makeDemoMentions();
}

function handleDemoAddMention(item: ChatMentionTarget): void {
  const agentId = String(item?.agentId || "").trim();
  if (!agentId) return;
  if (demoMentions.value.some((m) => m.agentId === agentId)) return;
  demoMentions.value = [
    ...demoMentions.value,
    {
      agentId,
      agentName: String(item?.agentName || agentId),
    },
  ];
}

function handleDemoRemoveMention(value: string | { agentId: string }): void {
  if (typeof value === "string") {
    demoMentions.value = demoMentions.value.filter((m) => m.agentId !== value);
    return;
  }
  const agentId = String(value?.agentId || "").trim();
  demoMentions.value = demoMentions.value.filter((m) => m.agentId !== agentId);
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
