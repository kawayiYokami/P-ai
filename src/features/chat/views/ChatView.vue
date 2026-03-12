<template>
  <div class="flex flex-col h-full min-h-0 relative">
    <div
      v-if="mediaDragActive && !chatting && !frozen"
      class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
    >
      <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
        {{ t("chat.dropImageOrPdf") }}
      </div>
    </div>
    <div
      ref="scrollContainer"
      class="flex-1 min-h-0 overflow-y-auto p-3 flex flex-col scrollbar-gutter-stable"
      @scroll="onScroll"
      @wheel.passive="onWheel"
    >
      <!-- 历史对话 -->
      <template v-for="block in messageBlocks" :key="block.id">
        <div :class="['chat group/user-turn mt-3', isOwnMessage(block) ? 'chat-end' : 'chat-start']">
          <div class="chat-image avatar self-start">
            <div class="w-7 rounded-full">
              <img v-if="messageAvatarUrl(block)" :src="messageAvatarUrl(block)" :alt="messageName(block)" />
              <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-xs">
                {{ avatarInitial(messageName(block)) }}
              </div>
            </div>
          </div>
          <div class="chat-header mb-1 flex items-center gap-2">
            <button
              v-if="isOwnMessage(block) && !chatting && !frozen"
              type="button"
              class="inline-flex h-5 w-5 items-center justify-center rounded text-base-content/40 hover:text-base-content opacity-0 pointer-events-none transition-opacity group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto"
              :title="t('chat.recall')"
              @click="$emit('recallTurn', { turnId: block.id })"
            >
              <Undo2 class="h-3 w-3" />
            </button>
            <span class="text-xs text-base-content">{{ messageName(block) }}</span>
            <time v-if="formattedBlockTime(block.createdAt)" class="text-[10px] opacity-50">{{ formattedBlockTime(block.createdAt) }}</time>
          </div>
          <div :class="['chat-bubble max-w-[92%]', isOwnMessage(block) ? '' : 'bg-base-100 text-base-content border border-base-300/70 assistant-markdown']">
            <div v-if="block.taskTrigger" class="space-y-2">
              <div class="flex items-center gap-2">
                <span class="badge badge-sm badge-outline">{{ t("chat.taskTrigger.badge") }}</span>
              </div>
              <div class="text-base font-medium leading-6">{{ block.taskTrigger.title }}</div>
              <div v-if="block.taskTrigger.cause" class="space-y-0.5">
                <div class="text-[11px] opacity-55">{{ t("config.task.fields.cause") }}</div>
                <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.cause }}</div>
              </div>
              <div v-if="block.taskTrigger.goal" class="space-y-0.5">
                <div class="text-[11px] opacity-55">{{ t("config.task.fields.goal") }}</div>
                <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.goal }}</div>
              </div>
              <div v-if="block.taskTrigger.flow" class="space-y-0.5">
                <div class="text-[11px] opacity-55">{{ t("config.task.fields.flow") }}</div>
                <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.flow }}</div>
              </div>
              <div v-if="block.taskTrigger.statusSummary" class="space-y-0.5">
                <div class="text-[11px] opacity-55">{{ t("config.task.fields.statusSummary") }}</div>
                <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.statusSummary }}</div>
              </div>
              <div v-if="block.taskTrigger.runAt || block.taskTrigger.endAt || block.taskTrigger.everyMinutes" class="grid gap-1 text-sm leading-6">
                <div v-if="block.taskTrigger.runAt">
                  <span class="text-[11px] opacity-55">{{ t("config.task.fields.runAt") }}</span>
                  <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.runAt) }}</span>
                </div>
                <div v-if="block.taskTrigger.endAt">
                  <span class="text-[11px] opacity-55">{{ t("config.task.fields.endAt") }}</span>
                  <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.endAt) }}</span>
                </div>
                <div v-if="block.taskTrigger.everyMinutes">
                  <span class="text-[11px] opacity-55">{{ t("config.task.fields.everyMinutes") }}</span>
                  <span class="ml-2">{{ block.taskTrigger.everyMinutes }}</span>
                </div>
              </div>
              <div v-if="(block.taskTrigger?.todos ?? []).length > 0" class="space-y-1">
                <div class="text-[11px] opacity-55">{{ t("config.task.fields.todos") }}</div>
                <ul class="space-y-1 text-sm leading-6">
                  <li v-for="(todo, todoIdx) in block.taskTrigger?.todos ?? []" :key="`${block.id}-todo-${todoIdx}`" class="flex items-start gap-2">
                    <span class="mt-2 inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-current opacity-60"></span>
                    <span class="min-w-0 whitespace-pre-wrap">{{ todo }}</span>
                  </li>
                </ul>
              </div>
            </div>
            <details
              v-if="!isOwnMessage(block) && block.reasoningStandard"
              class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none"
            >
              <summary class="collapse-title py-0 px-0 min-h-0 text-sm italic flex items-center text-base-content/80">
                <span class="block min-w-0 flex-1 truncate">
                  {{ firstLinePreview(block.reasoningStandard) || "..." }}
                </span>
              </summary>
              <div class="collapse-content px-0 py-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70 italic">
                {{ block.reasoningStandard }}
              </div>
            </details>
            <details
              v-if="!isOwnMessage(block) && resolvedInlineReasoning(block)"
              class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none"
            >
              <summary class="collapse-title py-0 px-0 min-h-0 text-[11px] italic flex items-center text-base-content/60 cursor-pointer">
                <span class="block min-w-0 flex-1 truncate">
                  {{ firstLinePreview(resolvedInlineReasoning(block)) || "..." }}
                </span>
              </summary>
              <div class="collapse-content max-w-full px-0 py-2 whitespace-pre-wrap wrap-break-word text-[11px] leading-relaxed text-base-content/60 italic" style="overflow-wrap: anywhere;">
                {{ resolvedInlineReasoning(block) }}
              </div>
            </details>
            <div v-if="block.toolCalls.length > 0" class="mb-2 flex flex-col gap-1 text-[11px] opacity-90">
              <details
                v-for="(toolCall, idx) in block.toolCalls"
                :key="`${block.id}-tool-${idx}`"
                class="collapse bg-base-200 border-base-300 border"
              >
                <summary class="collapse-title py-2 px-3 min-h-0 text-[11px] font-semibold flex items-center gap-1.5">
                  <span class="inline-block h-2 w-2 rounded-full bg-success"></span>
                  <span>调用 {{ toolCall.name }}</span>
                </summary>
                <div class="collapse-content px-3 pb-2 pt-0 text-[10px] text-base-content/70">
                  <pre class="whitespace-pre-wrap break-all">{{ toolCall.argsText }}</pre>
                </div>
              </details>
            </div>
            <div
              v-if="block.text"
              :class="block.taskTrigger ? 'mt-3' : ''"
            >
              <div
                v-if="isOwnMessage(block)"
                class="whitespace-pre-wrap"
              >{{ block.text }}</div>
              <div
                v-else
                class="ecall-markdown-content prose prose-sm max-w-none"
                v-html="renderMarkdown(splitThinkText(block.text).visible)"
                @click="handleAssistantLinkClick"
              ></div>
            </div>
            <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
              <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
                <img
                  v-if="isImageMime(img.mime)"
                  :src="`data:${img.mime};base64,${img.bytesBase64}`"
                  loading="lazy"
                  decoding="async"
                  class="rounded max-h-28 object-contain bg-base-100/40"
                />
                <div v-else-if="isPdfMime(img.mime)" class="badge badge-outline gap-1 py-3 w-fit">
                  <FileText class="h-3.5 w-3.5" />
                  <span class="text-[11px]">PDF</span>
                </div>
              </template>
            </div>
            <div v-if="block.audios.length > 0" :class="block.taskTrigger || block.text || block.images.length > 0 ? 'mt-2 flex flex-col gap-1' : 'flex flex-col gap-1'">
              <button
                v-for="(aud, idx) in block.audios"
                :key="`${block.id}-aud-${idx}`"
                class="btn btn-sm bg-base-100/70 w-fit"
                @click="toggleAudioPlayback(`${block.id}-aud-${idx}`, aud)"
              >
                <Pause v-if="playingAudioId === `${block.id}-aud-${idx}`" class="h-3 w-3" />
                <Play v-else class="h-3 w-3" />
                <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
              </button>
            </div>
            <div v-if="!isOwnMessage(block) && !chatting && !frozen" class="mt-2 flex items-center gap-1.5">
              <button
                type="button"
                class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
                :title="t('chat.copy')"
                @click="copyMessage(block)"
              >
                <Copy class="h-3.5 w-3.5" />
              </button>
              <button
                v-if="block.role === 'assistant'"
                type="button"
                class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
                :title="t('chat.regenerate')"
                @click="$emit('regenerateTurn', { turnId: block.id })"
              >
                <RotateCcw class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        </div>
      </template>

      <template v-if="showAssistantStreamingPreview">
        <!-- 助手流式响应 -->
        <div class="chat chat-start mt-3">
          <div class="chat-image self-start">
            <div class="flex flex-col items-center gap-2">
              <div class="avatar">
                <div class="w-7 rounded-full">
                  <img v-if="assistantAvatarUrl" :src="assistantAvatarUrl" :alt="personaName || t('archives.roleAssistant')" />
                  <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-xs">
                    {{ avatarInitial(personaName || t("archives.roleAssistant")) }}
                  </div>
                </div>
              </div>
              <button
                v-if="chatting"
                type="button"
                class="btn btn-error btn-sm btn-circle relative"
                :title="`${t('chat.stop')} / ${t('chat.stopReplying')}`"
                @click="$emit('stopChat')"
              >
                <Square class="h-4 w-4 fill-current" />
                <span class="pointer-events-none absolute inset-0 flex items-center justify-center">
                  <span class="loading loading-spinner loading-xl"></span>
                </span>
              </button>
            </div>
          </div>
          <div class="chat-header mb-1 flex items-center gap-2">
            <span class="text-xs text-base-content">{{ personaName || t("archives.roleAssistant") }}</span>
          </div>
          <div class="chat-bubble max-w-[92%] bg-base-100 text-base-content assistant-markdown">
            <details
              v-if="latestReasoningStandardText"
              class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none"
            >
              <summary class="collapse-title py-0 px-0 min-h-0 text-sm italic flex items-center gap-1 text-base-content/80">
                <span class="block min-w-0 flex-1 truncate">{{ firstLinePreview(latestReasoningStandardText) || "..." }}</span>
                <span class="loading loading-dots loading-sm opacity-60"></span>
              </summary>
              <div class="collapse-content px-0 py-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70 italic">
                {{ latestReasoningStandardText }}
              </div>
            </details>
            <details
              v-if="latestInlineReasoningText"
              class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none"
            >
              <summary class="collapse-title py-0 px-0 min-h-0 text-[11px] italic flex items-center gap-1 text-base-content/60 cursor-pointer">
                <span class="block min-w-0 flex-1 truncate">{{ firstLinePreview(latestInlineReasoningText) || "..." }}</span>
                <span class="loading loading-dots loading-sm opacity-60"></span>
              </summary>
              <div class="collapse-content max-w-full px-0 py-2 whitespace-pre-wrap wrap-break-word text-[11px] leading-relaxed text-base-content/60 italic" style="overflow-wrap: anywhere;">
                {{ latestInlineReasoningText }}
              </div>
            </details>
            <div
              v-if="streamToolCalls.length > 0"
              class="mb-2 flex flex-col gap-1 text-[11px] opacity-90"
            >
              <details
                v-for="(toolCall, idx) in streamToolCalls"
                :key="`stream-tool-${idx}`"
                class="collapse bg-base-200 border-base-300 border"
              >
                <summary class="collapse-title py-2 px-3 min-h-0 text-[11px] font-semibold flex items-center gap-1.5">
                  <span class="inline-block h-2 w-2 rounded-full bg-success"></span>
                  <span>调用 {{ toolCall.name }}</span>
                </summary>
                <div class="collapse-content px-3 pb-2 pt-0 text-[10px] text-base-content/70">
                  <pre class="whitespace-pre-wrap break-all">{{ toolCall.argsText }}</pre>
                </div>
              </details>
            </div>
            <div
              v-if="latestAssistantText"
              class="ecall-markdown-content prose prose-sm max-w-none"
              v-html="renderedAssistantHtml"
              @click="handleAssistantLinkClick"
            ></div>
            <div class="mt-1">
              <span v-if="!latestAssistantText" class="loading loading-dots loading-sm"></span>
              <span v-else-if="chatting" class="inline-block w-1.5 h-4 bg-base-content animate-pulse"></span>
            </div>
            <div v-if="toolStatusText" class="mt-1 text-[11px] opacity-80 flex items-center gap-1">
              <span v-if="toolStatusState === 'running'" class="loading loading-spinner loading-sm"></span>
              <span
                v-else-if="toolStatusState === 'failed'"
                class="inline-block w-1.5 h-1.5 rounded-full bg-error"
              ></span>
              <span
                v-else-if="toolStatusState === 'done'"
                class="inline-block w-1.5 h-1.5 rounded-full bg-success"
              ></span>
              <span>{{ toolStatusText }}</span>
            </div>
          </div>
        </div>
      </template>

      <div class="flex-1 min-h-3"></div>

      <div class="pt-1 pb-2">
        <div class="rounded-box border border-base-300 bg-base-100/70 px-2 py-1.5 flex items-center gap-2 text-[11px]">
          <button
            class="btn btn-sm bg-base-100"
            :title="workspaceLocked ? '已锁定，点击还原到默认工作空间' : '未锁定'"
            :disabled="chatting || frozen || !workspaceLocked"
            @click="$emit('unlockWorkspace')"
          >
            <Lock v-if="workspaceLocked" class="h-3.5 w-3.5" />
            <LockOpen v-else class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm bg-base-100"
            :disabled="chatting || frozen"
            @click="$emit('lockWorkspace')"
          >
            {{ currentWorkspaceName }}{{ workspaceLocked ? " (临时)" : "" }}
          </button>
          <div class="ml-auto flex items-center gap-1.5 overflow-x-auto scrollbar-thin">
            <button
              v-for="persona in props.personaPresenceChips"
              :key="persona.id"
              type="button"
              class="btn btn-ghost btn-sm btn-circle p-0 shrink-0 border relative"
              :class="persona.isFrontSpeaking ? 'border-primary/60 bg-primary/10' : 'border-base-300/70 bg-base-100/70'"
              :title="`部门：${persona.departmentName}\n人格：${persona.name}`"
              @click.prevent
            >
              <div class="avatar">
                <div
                  class="w-7 rounded-full"
                  :class="persona.isFrontSpeaking ? 'ring-2 ring-primary ring-offset-2 ring-offset-base-100' : ''"
                >
                  <img v-if="persona.avatarUrl" :src="persona.avatarUrl" :alt="persona.name" />
                  <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-[10px]">
                    {{ avatarInitial(persona.name) }}
                  </div>
                </div>
              </div>
              <span
                v-if="persona.hasBackgroundTask"
                class="absolute right-0.5 top-0.5 inline-block h-2.5 w-2.5 rounded-full bg-error ring-2 ring-base-100"
              ></span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <div v-show="showJumpToBottom" class="pointer-events-none absolute inset-x-0 bottom-20 z-30 flex justify-center">
      <button
        class="btn btn-sm btn-circle btn-primary pointer-events-auto shadow-md"
        :title="t('chat.jumpToBottom')"
        @click="jumpToBottom"
      >
        <ArrowDown class="h-4 w-4" />
      </button>
    </div>

    <div class="shrink-0 border-t border-base-300 bg-base-100 p-2">
      <!-- 队列预览 -->
      <ChatQueuePreview
        :queue-events="queueEvents"
        :session-state="sessionState"
        @recall-to-input="handleRecallToInput"
        @remove-from-queue="removeFromQueue"
      />

      <div
        v-if="linkOpenErrorText"
        class="alert alert-warning mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-24 overflow-auto"
      >
        <span>{{ linkOpenErrorText }}</span>
      </div>
      <div
        v-if="chatErrorText"
        class="alert alert-error mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-28 overflow-auto"
      >
        <span>{{ chatErrorText }}</span>
      </div>
      <div v-if="clipboardImages.length > 0" class="flex flex-wrap gap-1 mb-2">
        <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-outline gap-1 py-3">
          <ImageIcon v-if="isImageMime(img.mime)" class="h-3.5 w-3.5" />
          <FileText v-else-if="isPdfMime(img.mime)" class="h-3.5 w-3.5" />
          <ImageIcon v-else class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ isPdfMime(img.mime) ? `PDF ${idx + 1}` : t("chat.image", { index: idx + 1 }) }}</span>
          <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="$emit('removeClipboardImage', idx)">
            <X class="h-3 w-3" />
          </button>
        </div>
      </div>
      <div v-if="transcribing" class="mb-1 text-[11px] opacity-80 flex items-center gap-1">
        <span class="loading loading-spinner loading-sm"></span>
        <span>语音转写中...</span>
      </div>
      <div class="flex flex-row items-center gap-2">
        <textarea
          ref="chatInputRef"
          v-model="localChatInput"
          class="flex-1 textarea textarea-sm resize-none overflow-y-hidden chat-input-no-focus min-h-12.5"
          rows="1"
          :disabled="frozen"
          :placeholder="chatInputPlaceholder"
          @input="scheduleResizeChatInput"
          @keydown.enter.exact.prevent="!frozen && $emit('sendChat')"
        ></textarea>
        <div class="flex flex-col gap-2 mt-auto">
          <button
            class="btn btn-sm btn-circle shrink-0"
            :class="recording ? 'btn-error' : 'bg-base-100'"
            :disabled="!canRecord || chatting || frozen"
            :title="recording ? t('chat.recording', { seconds: Math.max(1, Math.round(recordingMs / 1000)) }) : t('chat.holdRecord', { hotkey: recordHotkey })"
            @mousedown.prevent="$emit('startRecording')"
            @mouseup.prevent="$emit('stopRecording')"
            @mouseleave.prevent="recording && $emit('stopRecording')"
            @touchstart.prevent="$emit('startRecording')"
            @touchend.prevent="$emit('stopRecording')"
          >
            <Mic class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-circle btn-primary shrink-0"
            :disabled="frozen"
            :title="t('chat.send')"
            @click="$emit('sendChat')"
          >
            <Send class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDown, ArrowUp, Copy, FileText, Image as ImageIcon, Lock, LockOpen, Mic, Pause, Play, RotateCcw, Send, Square, Undo2, X } from "lucide-vue-next";
import MarkdownIt from "markdown-it";
import { katex } from "@mdit/plugin-katex";
import { mark } from "@mdit/plugin-mark";
import { alert } from "@mdit/plugin-alert";
import mermaid from "mermaid";
import DOMPurify from "dompurify";
import twemoji from "twemoji";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessageBlock, ChatPersonaPresenceChip } from "../../../types/app";
import ChatQueuePreview from "../components/ChatQueuePreview.vue";
import { useChatQueue } from "../composables/use-chat-queue";

const props = defineProps<{
  userAlias: string;
  personaName: string;
  userAvatarUrl: string;
  assistantAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  personaPresenceChips: ChatPersonaPresenceChip[];
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string }>;
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  chatInput: string;
  chatInputPlaceholder: string;
  canRecord: boolean;
  recording: boolean;
  recordingMs: number;
  transcribing: boolean;
  recordHotkey: string;
  mediaDragActive: boolean;
  chatting: boolean;
  frozen: boolean;
  messageBlocks: ChatMessageBlock[];
  hasMoreMessageBlocks: boolean;
  currentWorkspaceName: string;
  workspaceLocked: boolean;
}>();

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "loadMoreMessageBlocks"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "lockWorkspace"): void;
  (e: "unlockWorkspace"): void;
}>();
const { t } = useI18n();

// 队列管理
const { queueEvents, sessionState, removeFromQueue } = useChatQueue();

// 从队列退回到输入框
function handleRecallToInput(event: any) {
  if (event.source === "user") {
    localChatInput.value = event.messagePreview;
    removeFromQueue(event.id);
  }
}

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});

const scrollContainer = ref<HTMLElement | null>(null);
const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const autoFollowOutput = ref(true);
const playingAudioId = ref("");
const linkOpenErrorText = ref("");
let activeAudio: HTMLAudioElement | null = null;
let followScrollRaf = 0;
let resizeInputRaf = 0;
let mermaidInited = false;
let mermaidRenderToken = 0;
let mermaidApi: any | null = null;
let mermaidObserver: MutationObserver | null = null;
let mermaidRenderQueued = false;
const MERMAID_SENTINEL = "__EASY_CALL_MERMAID__";

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
})
  .use(katex)
  .use(mark)
  .use(alert);
const mdAny = md as any;
const defaultFenceRenderer =
  mdAny.renderer?.rules?.fence ??
  ((tokens: any[], idx: number, options: unknown, _env: unknown, self: any) =>
    self.renderToken(tokens, idx, options));
mdAny.renderer.rules.fence = (
  tokens: any[],
  idx: number,
  options: unknown,
  env: unknown,
  self: any,
) => {
  const token = tokens[idx];
  const info = String(token?.info || "").trim().toLowerCase();
  if (info === "mermaid") {
    const content = String(token?.content || "");
    return `<pre><code>${escapeHtml(`${MERMAID_SENTINEL}\n${content}`)}</code></pre>`;
  }
  return defaultFenceRenderer(tokens, idx, options, env, self);
};

function escapeHtml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function ensureMermaidInited(): boolean {
  if (mermaidInited && mermaidApi) return true;
  try {
    const api = (mermaid as any).default ?? mermaid;
    api.initialize({
      startOnLoad: false,
      theme: "default",
      securityLevel: "strict",
    });
    mermaidApi = api;
    mermaidInited = true;
    return true;
  } catch {
    return false;
  }
}

function scheduleRenderMermaidBlocks() {
  if (mermaidRenderQueued) return;
  mermaidRenderQueued = true;
  requestAnimationFrame(() => {
    mermaidRenderQueued = false;
    void renderMermaidBlocks();
  });
}

async function renderMermaidBlocks() {
  const root = scrollContainer.value;
  if (!root) return;
  const candidates = Array.from(
    root.querySelectorAll<HTMLElement>(
      ".assistant-markdown pre > code",
    ),
  );
  if (!candidates.length) return;
  const token = ++mermaidRenderToken;
  const ok = ensureMermaidInited();
  if (!ok || !mermaidApi) return;
  for (let index = 0; index < candidates.length; index += 1) {
    if (token !== mermaidRenderToken) return;
    const node = candidates[index];
    const host =
      node.tagName === "CODE" ? (node.parentElement as HTMLElement | null) : node;
    if (!host) continue;
    const rawText = (
      node.tagName === "CODE"
        ? node.textContent
        : node.querySelector("code")?.textContent || node.textContent
    || "").trim();
    if (!rawText.startsWith(MERMAID_SENTINEL)) continue;
    const source = rawText
      .slice(MERMAID_SENTINEL.length)
      .replace(/^\r?\n/, "")
      .trim();
    if (!source) continue;
    const sourceKey = `${source.length}:${source}`;
    if (host.dataset.failedFor === sourceKey) continue;
    host.classList.remove("ecall-mermaid-error");
    try {
      const id = `ecall-mermaid-${Date.now()}-${index}`;
      const { svg } = await mermaidApi.render(id, source);
      if (token !== mermaidRenderToken) return;
      const container = document.createElement("div");
      container.className = "ecall-mermaid-diagram";
      container.innerHTML = svg;
      host.replaceWith(container);
    } catch {
      try {
        const normalized = normalizeMermaidSource(source);
        if (normalized && normalized !== source) {
          const id = `ecall-mermaid-retry-${Date.now()}-${index}`;
          const { svg } = await mermaidApi.render(id, normalized);
          if (token !== mermaidRenderToken) return;
          const container = document.createElement("div");
          container.className = "ecall-mermaid-diagram";
          container.innerHTML = svg;
          host.replaceWith(container);
          continue;
        }
      } catch {
        // fall through to failed mark
      }
      host.dataset.failedFor = sourceKey;
      host.classList.add("ecall-mermaid-error");
    }
  }
}

function normalizeMermaidSource(source: string): string {
  return source
    .replace(/\r\n/g, "\n")
    .replace(/[：]/g, ":")
    .replace(/[，]/g, ",")
    .replace(/[；]/g, ";")
    .replace(/[（]/g, "(")
    .replace(/[）]/g, ")")
    .replace(/[【]/g, "[")
    .replace(/[】]/g, "]")
    .replace(/[“”]/g, "\"")
    .replace(/[‘’]/g, "'");
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function messageName(block: ChatMessageBlock): string {
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaNameMap[id]) return props.personaNameMap[id];
  if (!id || id === "user-persona") return props.userAlias || t("archives.roleUser");
  return id;
}

function messageAvatarUrl(block: ChatMessageBlock): string {
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaAvatarUrlMap[id]) return props.personaAvatarUrlMap[id];
  if (!id || id === "user-persona") return props.userAvatarUrl || "";
  return "";
}

function isOwnMessage(block: ChatMessageBlock): boolean {
  const id = String(block.speakerAgentId || "").trim();
  return !id || id === "user-persona";
}

function formattedBlockTime(value?: string): string {
  const raw = String(value || "").trim();
  if (!raw) return "";
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return raw;
  const parts = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).formatToParts(parsed);
  const pick = (type: string) => parts.find((part) => part.type === type)?.value || "00";
  return `${pick("hour")}:${pick("minute")}:${pick("second")}`;
}

function splitThinkText(raw: string): { visible: string; inline: string } {
  const input = raw || "";
  const openTag = "<think>";
  const closeTag = "</think>";
  const blocks: string[] = [];
  let visible = "";
  let cursor = 0;

  while (cursor < input.length) {
    const openIdx = input.indexOf(openTag, cursor);
    if (openIdx < 0) {
      visible += input.slice(cursor);
      break;
    }

    visible += input.slice(cursor, openIdx);
    const afterOpen = openIdx + openTag.length;
    const closeIdx = input.indexOf(closeTag, afterOpen);
    if (closeIdx < 0) {
      const tail = input.slice(afterOpen).trim();
      if (tail) blocks.push(tail);
      cursor = input.length;
      break;
    }

    const inner = input.slice(afterOpen, closeIdx).trim();
    if (inner) blocks.push(inner);
    cursor = closeIdx + closeTag.length;
  }

  return {
    visible: visible.trim(),
    inline: blocks.join("\n\n"),
  };
}

function renderMarkdown(text: string): string {
  const raw = md.render(text || "");
  const safeHtml = DOMPurify.sanitize(raw);
  const withEmoji = twemoji.parse(safeHtml, {
    folder: "svg",
    ext: ".svg",
    className: "twemoji",
  });
  return DOMPurify.sanitize(withEmoji);
}

const latestAssistantParts = computed(() => splitThinkText(props.latestAssistantText));
const latestInlineReasoningText = computed(
  () => latestAssistantParts.value.inline || props.latestReasoningInlineText || "",
);
const renderedAssistantHtml = computed(() => renderMarkdown(latestAssistantParts.value.visible));
const latestPersistedAssistantBlock = computed(() => {
  for (let i = props.messageBlocks.length - 1; i >= 0; i -= 1) {
    const block = props.messageBlocks[i];
    if (block.role === "assistant") return block;
  }
  return null;
});
const streamingPreviewDuplicatedInHistory = computed(() => {
  const lastAssistant = latestPersistedAssistantBlock.value;
  if (!lastAssistant) return false;
  if (!props.latestAssistantText.trim()) return false;
  return (
    String(splitThinkText(lastAssistant.text || "").visible).trim() === latestAssistantParts.value.visible.trim()
    && String(lastAssistant.reasoningStandard || "").trim() === String(props.latestReasoningStandardText || "").trim()
    && String(resolvedInlineReasoning(lastAssistant) || "").trim() === String(latestInlineReasoningText.value || "").trim()
  );
});
const showAssistantStreamingPreview = computed(() => {
  // 只要当前存在前台主助理轮次，就应立即显示助理气泡。
  // 这样用户一点击发送，就能看到“助理已接手当前轮次”的稳定反馈，
  // 而不是等第一段 delta 到来后气泡才突然出现。
  return props.chatting && !streamingPreviewDuplicatedInHistory.value;
});

function resolvedInlineReasoning(block: ChatMessageBlock): string {
  return splitThinkText(block.text).inline || block.reasoningInline || "";
}

async function copyMessage(block: ChatMessageBlock) {
  const copyText = splitThinkText(block.text).visible || block.text || "";
  if (!copyText) return;
  try {
    await navigator.clipboard.writeText(copyText);
  } catch {
    // Ignore clipboard failures to avoid interrupting chat flow.
  }
}

function firstLinePreview(raw: string): string {
  if (!raw) return "";
  const lines = raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return lines.length ? lines[lines.length - 1] : raw.trim();
}

function buildAudioDataUrl(audio: { mime: string; bytesBase64: string }): string {
  return `data:${audio.mime};base64,${audio.bytesBase64}`;
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function stopAudioPlayback() {
  if (activeAudio) {
    activeAudio.pause();
    activeAudio.currentTime = 0;
    activeAudio = null;
  }
  playingAudioId.value = "";
}

function toggleAudioPlayback(id: string, audio: { mime: string; bytesBase64: string }) {
  if (playingAudioId.value === id && activeAudio) {
    stopAudioPlayback();
    return;
  }
  stopAudioPlayback();
  const player = new Audio(buildAudioDataUrl(audio));
  activeAudio = player;
  playingAudioId.value = id;
  player.onended = () => {
    if (activeAudio === player) {
      activeAudio = null;
      playingAudioId.value = "";
    }
  };
  void player.play().catch(() => {
    if (activeAudio === player) {
      activeAudio = null;
      playingAudioId.value = "";
    }
  });
}

function scrollToBottom(behavior: ScrollBehavior = "auto") {
  const el = scrollContainer.value;
  if (el) {
    el.scrollTo({ top: el.scrollHeight, behavior });
  }
}

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.min(el.scrollHeight, maxHeight);
  el.style.height = `${nextHeight}px`;
  el.style.overflowY = el.scrollHeight > maxHeight ? "auto" : "hidden";
}

function scheduleResizeChatInput() {
  if (resizeInputRaf) cancelAnimationFrame(resizeInputRaf);
  resizeInputRaf = requestAnimationFrame(() => {
    resizeChatInput();
    resizeInputRaf = 0;
  });
}

function isNearBottom(el: HTMLElement): boolean {
  const threshold = 24;
  const distance = el.scrollHeight - (el.scrollTop + el.clientHeight);
  return distance <= threshold;
}

const showJumpToBottom = computed(() => !autoFollowOutput.value);

function jumpToBottom() {
  autoFollowOutput.value = true;
  nextTick(() => scrollToBottom("smooth"));
}

let loadingMore = false;
let loadingMoreOldHeight = 0;
let lastScrollTop = 0;

function evaluateFollowState(el: HTMLElement) {
  // Hysteresis: avoid jitter around the boundary during streaming updates.
  const enterFollowThreshold = 24;
  const leaveFollowThreshold = 72;
  const distance = el.scrollHeight - (el.scrollTop + el.clientHeight);
  if (autoFollowOutput.value) {
    if (distance > leaveFollowThreshold) {
      autoFollowOutput.value = false;
    }
    return;
  }
  if (distance <= enterFollowThreshold) {
    autoFollowOutput.value = true;
  }
}

function onScroll() {
  const el = scrollContainer.value;
  if (!el) return;
  const scrollingUp = el.scrollTop < lastScrollTop;
  lastScrollTop = el.scrollTop;
  if (followScrollRaf) cancelAnimationFrame(followScrollRaf);
  followScrollRaf = requestAnimationFrame(() => {
    evaluateFollowState(el);
    followScrollRaf = 0;
  });
  if (scrollingUp && el.scrollTop <= 20 && props.hasMoreMessageBlocks && !loadingMore) {
    loadingMore = true;
    loadingMoreOldHeight = el.scrollHeight;
    emit("loadMoreMessageBlocks");
  }
}

function onWheel(event: WheelEvent) {
  const el = scrollContainer.value;
  if (!el) return;
  const pushingUpAtTop = event.deltaY < 0 && el.scrollTop <= 20;
  if (pushingUpAtTop && props.hasMoreMessageBlocks && !loadingMore) {
    loadingMore = true;
    loadingMoreOldHeight = el.scrollHeight;
    emit("loadMoreMessageBlocks");
  }
}

async function handleAssistantLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = anchor.getAttribute("href")?.trim() || "";
  if (!href || (!href.startsWith("http://") && !href.startsWith("https://"))) return;
  event.preventDefault();
  event.stopPropagation();
  try {
    await invokeTauri("open_external_url", { url: href });
    linkOpenErrorText.value = "";
  } catch (error) {
    linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
  }
}

onMounted(() => {
  nextTick(() => {
    scrollToBottom();
    autoFollowOutput.value = true;
    resizeChatInput();
    scheduleRenderMermaidBlocks();
    const root = scrollContainer.value;
    if (!root) return;
    mermaidObserver = new MutationObserver(() => {
      scheduleRenderMermaidBlocks();
    });
    mermaidObserver.observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
    });
  });
});

onBeforeUnmount(() => {
  if (followScrollRaf) {
    cancelAnimationFrame(followScrollRaf);
    followScrollRaf = 0;
  }
  if (resizeInputRaf) {
    cancelAnimationFrame(resizeInputRaf);
    resizeInputRaf = 0;
  }
  if (mermaidObserver) {
    mermaidObserver.disconnect();
    mermaidObserver = null;
  }
  stopAudioPlayback();
});

watch(
  () => props.chatInput,
  () => {
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => props.chatting,
  (isChatting, wasChatting) => {
    if (!autoFollowOutput.value) return;
    nextTick(() => scrollToBottom());
    if (wasChatting && !isChatting && !props.frozen) {
      nextTick(() => chatInputRef.value?.focus({ preventScroll: true }));
    }
  },
);

watch(
  () => props.messageBlocks.length,
  (newLen, oldLen) => {
    if (loadingMore && newLen > oldLen) {
      nextTick(() => {
        const el = scrollContainer.value;
        if (!el) {
          loadingMore = false;
          return;
        }
        const newHeight = el.scrollHeight;
        el.scrollTop = Math.max(0, newHeight - loadingMoreOldHeight);
        loadingMore = false;
      });
      return;
    }
    if (newLen > oldLen && autoFollowOutput.value) {
      nextTick(() => scrollToBottom());
    }
    nextTick(() => {
      scheduleRenderMermaidBlocks();
    });
  },
);

watch(
  () => props.hasMoreMessageBlocks,
  (hasMore) => {
    if (hasMore) return;
    loadingMore = false;
  },
);

watch(
  () => [
    props.latestAssistantText,
    props.latestReasoningStandardText,
    props.latestReasoningInlineText,
    props.toolStatusText,
  ],
  () => {
    if (autoFollowOutput.value) {
      nextTick(() => scrollToBottom());
    }
    nextTick(() => {
      scheduleRenderMermaidBlocks();
    });
  },
);
</script>

<style scoped>
.scrollbar-gutter-stable {
  scrollbar-gutter: stable;
}

.assistant-markdown :deep(.ecall-markdown-content) {
  overflow-wrap: anywhere;
  word-break: break-word;
}

.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: currentColor;
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: hsl(var(--bc) / 0.5);
  --tw-prose-hr: hsl(var(--bc) / 0.15);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: hsl(var(--bc) / 0.2);
  --tw-prose-captions: hsl(var(--bc) / 0.75);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: hsl(var(--b2));
  --tw-prose-th-borders: hsl(var(--bc) / 0.2);
  --tw-prose-td-borders: hsl(var(--bc) / 0.15);
}

.assistant-markdown :deep(.ecall-markdown-content pre) {
  overflow-x: auto;
}

.assistant-markdown :deep(.ecall-mermaid-diagram) {
  margin: 0.3rem 0 0.45rem;
  overflow-x: auto;
}

.assistant-markdown :deep(.ecall-mermaid-diagram svg) {
  max-width: 100%;
  height: auto;
}

.assistant-markdown :deep(img.twemoji) {
  width: 1.1em;
  height: 1.1em;
  margin: 0 0.06em;
  vertical-align: -0.14em;
  display: inline-block;
}

:deep(.chat-bubble) {
  min-width: 0;
  min-height: 0;
}

:deep(.chat-input-no-focus),
:deep(.chat-input-no-focus:hover),
:deep(.chat-input-no-focus:focus),
:deep(.chat-input-no-focus:focus-visible) {
  border: none !important;
  outline: none !important;
  box-shadow: none !important;
}

</style>
