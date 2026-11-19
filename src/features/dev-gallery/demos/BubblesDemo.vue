<template>
  <div class="overflow-hidden rounded-[1.75rem] border border-base-300 bg-base-200/70 p-4 shadow-inner">
    <div class="flex flex-col gap-4 rounded-[1.35rem] border border-base-300 bg-base-100/30 px-3 py-4">
      <ChatBubbleShell
        v-for="message in bubbleDemoDisplayMessages"
        :key="message.id"
        :tone="message.tone"
        :name="message.name"
        :meta="message.meta"
        :avatar-url="message.avatarUrl"
        :streaming="!!message.streaming"
        :streaming-text="message.streamingText"
        :separated="message.separated"
      >
        <template v-if="message.reasoning?.length || message.tools?.length" #activity>
          <div class="flex flex-col opacity-90">
            <div class="flex min-w-0 flex-wrap items-center gap-1.5 py-0.5 text-sm font-normal text-base-content/42">
              <span class="shrink-0">思考与工具</span>
              <span v-if="message.tools?.length" class="inline-flex h-3 items-center text-base-content/30">·</span>
              <span v-if="message.tools?.length" class="min-w-0 truncate text-base-content/42">
                {{ message.tools.length }} 个工具
              </span>
            </div>
            <div class="flex flex-col pt-1 text-xs text-base-content/70">
              <div
                v-if="message.reasoning?.length"
                class="flex gap-1.5 border-l border-base-content/15 py-1 pr-1 pl-2"
              >
                <span class="inline-flex w-3 shrink-0 items-center justify-center font-mono text-xs leading-none text-info">•</span>
                <ExpandableText
                  class="min-w-0 flex-1"
                  :text="message.reasoning.join('\n')"
                  text-class="text-base-content/70"
                />
              </div>
              <details
                v-for="(tool, toolIndex) in message.tools || []"
                :key="`${tool.name}-${toolIndex}`"
                class="collapse rounded-none border-l border-base-content/15 pl-2"
                :open="bubbleDemoActivityItemOpen(message.id, bubbleDemoToolItemKey(tool, toolIndex))"
                @toggle="onBubbleDemoActivityItemToggle(message.id, bubbleDemoToolItemKey(tool, toolIndex), $event)"
              >
                <summary class="collapse-title flex min-h-0 items-center gap-1.5 px-1 py-1 text-xs hover:bg-base-200">
                  <span class="inline-flex w-3 shrink-0 items-center justify-center leading-none text-success">
                    <AppIcon name="file-text" v-if="tool.icon === 'file'" class="size-3" aria-hidden="true" />
                    <AppIcon name="square-terminal" v-else-if="tool.icon === 'terminal'" class="size-3" aria-hidden="true" />
                    <AppIcon name="wrench" v-else class="size-3" aria-hidden="true" />
                  </span>
                  <span class="min-w-0 flex-1 truncate text-base-content/75">
                    {{ tool.name }}
                    <span class="ml-1 text-success">✓</span>
                  </span>
                </summary>
                <div v-if="bubbleDemoActivityItemOpen(message.id, bubbleDemoToolItemKey(tool, toolIndex))" class="collapse-content px-1 pb-2 pt-1">
                  <div class="whitespace-pre-wrap wrap-break-word text-xs leading-relaxed text-base-content/70">
                    {{ tool.detail }}
                  </div>
                </div>
              </details>
            </div>
          </div>
        </template>
        <div class="space-y-2 text-sm leading-relaxed">
          <p v-for="line in message.lines" :key="line" class="m-0 whitespace-pre-wrap break-words">
            {{ line }}
          </p>
          <div
            v-if="message.chips?.length"
            :class="[
              'flex flex-wrap gap-1.5 pt-1',
              message.tone === 'user' ? 'justify-end' : 'justify-start',
            ]"
          >
            <span
              v-for="chip in message.chips"
              :key="chip"
              class="rounded-lg bg-base-100/35 px-2 py-1 text-xs text-base-content/60 backdrop-blur-sm"
            >
              {{ chip }}
            </span>
          </div>
          <ChatAttachmentList
            v-if="message.attachments?.length"
            :attachments="message.attachments"
            :align="message.tone === 'user' ? 'end' : 'start'"
            :interactive-kinds="['audio']"
            :playing-id="playingBubbleDemoAudioId"
            @activate="handleBubbleDemoAttachmentActivate"
          />
        </div>
        <template #footer>
          <span v-if="message.footer">{{ message.footer }}</span>
          <span v-if="!message.streaming" class="inline-flex items-center gap-1">
            <AppIcon name="copy" class="size-3" aria-hidden="true" />
            复制
          </span>
          <span v-if="!message.streaming && message.canRecall" class="inline-flex items-center gap-1">
            <AppIcon name="undo" class="size-3" aria-hidden="true" />
            撤回
          </span>
        </template>
      </ChatBubbleShell>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import ChatBubbleShell from "../../chat/components/ChatBubbleShell.vue";
import ChatAttachmentList from "../../chat/components/ChatAttachmentList.vue";
import ExpandableText from "../../shared/components/ExpandableText.vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ChatAttachmentView } from "../../chat/utils/chat-attachment-display";

type BubbleDemoMessage = {
  id: string;
  tone: "assistant" | "user" | "system";
  name: string;
  meta: string;
  reasoning?: string[];
  tools?: Array<{
    name: string;
    detail: string;
    icon: "file" | "terminal" | "tool";
  }>;
  lines: string[];
  chips?: string[];
  attachments?: ChatAttachmentView[];
  footer?: string;
  canRecall?: boolean;
  streaming?: boolean;
  streamingText?: string;
};

type BubbleDemoTool = NonNullable<BubbleDemoMessage["tools"]>[number];

type BubbleDemoDisplayMessage = BubbleDemoMessage & {
  avatarUrl: string;
  separated: boolean;
};

const bubbleDemoActivityItemOpenKey = ref("");
const playingBubbleDemoAudioId = ref("");

const bubbleDemoAttachmentImage = `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" width="520" height="300" viewBox="0 0 520 300"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop stop-color="#dbeafe"/><stop offset="1" stop-color="#c4b5fd"/></linearGradient></defs><rect width="520" height="300" rx="24" fill="url(#g)"/><rect x="38" y="38" width="444" height="224" rx="16" fill="#111827" fill-opacity=".82"/><path d="M72 92h156M72 126h260M72 160h208M72 194h300" stroke="#bfdbfe" stroke-linecap="round" stroke-width="12"/><circle cx="418" cy="104" r="28" fill="#a7f3d0" fill-opacity=".9"/><text x="72" y="238" fill="#e0e7ff" font-family="Segoe UI, sans-serif" font-size="18">screenshot.png</text></svg>',
 )}`;

const bubbleDemoMessages: BubbleDemoMessage[] = [
  {
    id: "assistant-opening",
    tone: "assistant",
    name: "Pai",
    meta: "刚刚",
    reasoning: [
      "**Considering layout migration** The user wants to replace DaisyUI chat with a lighter custom message shell.",
      "Need preserve current activity presentation while separating assistant content from user bubbles.",
      "Only the user side should keep a translucent rounded rectangle bubble.",
    ],
    tools: [
      { name: "exec", detail: 'rg -n "chat-bubble|activityPanel|thinkingAndTools" src/features/chat', icon: "terminal" },
      { name: "exec", detail: 'rg -n "ChatMessageItem|DemoTab|ChatBubbleShell" src/features', icon: "terminal" },
    ],
    lines: [
      "我把名字、气泡和底部操作都放在同一列里，头像永远只占头像列。",
      "这样左侧消息不会被头像压到下面，右侧消息也能自然贴齐内容列。",
    ],
    chips: ["半透明背景", "无气泡尾巴", "名称对齐"],
    canRecall: true,
  },
  {
    id: "reviewer-assistant",
    tone: "assistant",
    name: "代码审查员",
    meta: "刚刚",
    reasoning: [
      "**Reviewing assistant identity** Multiple assistant speakers should stay visually distinct without adding bubbles.",
      "The content column must align name, activity, final answer, and footer under the same speaker identity.",
    ],
    tools: [
      { name: "exec", detail: 'rg -n "speakerAgentId|personaNameMap|activityItems" src/features/chat', icon: "terminal" },
      { name: "exec", detail: 'Get-Content src/features/chat/components/ChatMessageItem.vue | Select-Object -First 220', icon: "terminal" },
    ],
    lines: [
      "这里是另一个助理身份。左侧消息没有气泡底，只保留头像、名称、内容和工具状态。",
      "如果多个助理连续发言，视觉上应该靠头像和名称区分，而不是靠不同气泡颜色。",
    ],
    chips: ["review", "layout"],
    canRecall: true,
  },
  {
    id: "user-reply",
    tone: "user",
    name: "我",
    meta: "1 分钟前",
    lines: [
      "右侧也保持同一套结构：头像在右，名称和气泡右对齐，不再依赖 DaisyUI chat 的网格。",
    ],
    attachments: [
      { kind: "file", label: "screenshot.png" },
      { kind: "file", label: "design-notes.md" },
    ],
    canRecall: true,
  },
  {
    id: "user-all-attachments",
    tone: "user",
    name: "我",
    meta: "刚刚",
    lines: ["这是一条全附件气泡，用来观察不同附件内容放在一起时的层级。"],
    attachments: [
      { kind: "image", label: "screenshot.png", src: bubbleDemoAttachmentImage },
      { id: "demo-meeting-note", kind: "audio", label: "meeting-note.m4a", detail: "0:18" },
      { kind: "file", label: "design-notes.md" },
      { kind: "context", label: "ChatMessageItem.vue" },
      { kind: "text", label: "保持附件标签轻量，避免重复说明。" },
    ],
    canRecall: true,
  },
  {
    id: "memory-assistant",
    tone: "assistant",
    name: "记忆管家",
    meta: "1 分钟前",
    tools: [
      { name: "exec", detail: 'rg -n "ChatBubbleShell|message footer" src/features/chat', icon: "terminal" },
    ],
    lines: [
      "我补一条更短的助理消息，观察无气泡状态下短文本是否过于飘。",
      "如果太轻，可以后续给助理内容区加极淡的左侧引导线，而不是恢复气泡。",
    ],
    chips: ["memory: UI preference", "命中 3"],
    canRecall: true,
  },
  {
    id: "assistant-long",
    tone: "system",
    name: "系统人格",
    meta: "2 分钟前",
    reasoning: [
      "**Preparing demo patch** Keep the current activity visual language, but make sample tool content match real runtime traces.",
      "The final answer below should remain independent from activity, so future rendering can lazy-load details.",
    ],
    tools: [
      { name: "apply_patch", detail: "更新 DemoTab.vue 的气泡样张内容", icon: "tool" },
      { name: "exec", detail: "pnpm typecheck", icon: "terminal" },
    ],
    lines: [
      "长内容会限制在内容列最大宽度内，背景是圆角矩形半透明层，适合后续接 Markdown、工具卡片和附件。",
      "这个 demo 先只验证外壳：左右、头像、名称、气泡、footer。确认方向后，再迁移到正式 ChatMessageItem。",
    ],
    chips: ["未来兼容: assistant-attachment.md"],
    streaming: true,
    streamingText: "正在执行工具并等待供应商返回结果",
  },
];

// 样张不传 personas 与头像表，按默认值出图：名称取样本自身，头像留空。
const bubbleDemoDisplayMessages = computed<BubbleDemoDisplayMessage[]>(() =>
  bubbleDemoMessages.map((message, index) => {
    const previous = bubbleDemoMessages[index - 1];
    return {
      ...message,
      avatarUrl: "",
      separated: !!previous && message.tone !== "user" && previous.tone !== "user",
    };
  }),
);

function detailsOpenFromEvent(event: Event): boolean {
  const target = event.target;
  return target instanceof HTMLDetailsElement ? target.open : false;
}

function bubbleDemoActivityItemKey(messageId: string, itemKey: string): string {
  return `${messageId}:${itemKey}`;
}

function bubbleDemoActivityItemOpen(messageId: string, itemKey: string): boolean {
  return bubbleDemoActivityItemOpenKey.value === bubbleDemoActivityItemKey(messageId, itemKey);
}

function bubbleDemoToolItemKey(tool: BubbleDemoTool, index: number): string {
  return `tool:${index}:${String(tool.name || "").trim()}`;
}

function toggleBubbleDemoAudio(attachmentId: string): void {
  playingBubbleDemoAudioId.value = playingBubbleDemoAudioId.value === attachmentId ? "" : attachmentId;
}

function handleBubbleDemoAttachmentActivate(payload: { attachment: ChatAttachmentView }): void {
  if (payload.attachment.kind !== "audio" || !payload.attachment.id) return;
  toggleBubbleDemoAudio(payload.attachment.id);
}

function onBubbleDemoActivityItemToggle(messageId: string, itemKey: string, event: Event): void {
  const nextKey = bubbleDemoActivityItemKey(messageId, itemKey);
  bubbleDemoActivityItemOpenKey.value = detailsOpenFromEvent(event) ? nextKey : "";
}
</script>
