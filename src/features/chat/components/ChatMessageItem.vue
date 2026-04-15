<template>
  <div
    :data-message-id="String(block.id || '')"
    :data-message-role="isOwnMessage(block) ? 'user' : block.role"
    :data-active-turn-user="activeTurnUser ? 'true' : undefined"
    :class="[
      'chat group/user-turn mt-3 ecall-message-enter',
      isOwnMessage(block) ? 'chat-end' : 'chat-start',
    ]"
  >
    <div class="chat-image self-start ecall-chat-avatar-col">
      <div class="flex w-7 flex-col items-center gap-2">
        <div class="avatar">
          <div class="w-7 rounded-full">
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              :alt="displayName"
              class="w-7 h-7 rounded-full object-cover"
            />
            <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-xs">
              {{ avatarInitial(displayName) }}
            </div>
          </div>
        </div>
      </div>
    </div>
    <template v-if="!isOwnMessage(block)">
      <div class="ecall-message-stack min-w-0 flex flex-col self-stretch">
        <div
          :class="[
            'ecall-message-content min-w-0',
            blockNeedsWideBubble(block) ? 'ecall-message-content-wide' : '',
          ]"
        >
          <div class="chat-header mb-1 flex items-center gap-2">
            <span class="text-xs text-base-content">{{ displayName }}</span>
            <span class="inline-flex h-4 items-center text-[10px] leading-none">
              <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
                <span class="loading loading-infinity loading-sm"></span>
              </span>
              <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
            </span>
          </div>
          <div :class="[
            'chat-bubble',
            'self-start bg-base-100 text-base-content border border-base-300/70 assistant-markdown ecall-assistant-bubble max-w-full',
            blockNeedsWideBubble(block) ? 'ecall-assistant-bubble-wide' : '',
          ]">
            <div v-if="block.planCard" class="space-y-3">
              <div class="flex items-center gap-2">
                <span class="badge badge-sm badge-ghost">
                  {{ block.planCard.action === "complete" ? t("chat.plan.completeBadge") : t("chat.plan.badge") }}
                </span>
              </div>
              <div class="whitespace-pre-wrap text-sm leading-6">{{ block.planCard.context }}</div>
              <div v-if="block.planCard.action === 'present'" class="space-y-2">
                <button
                  type="button"
                  class="btn btn-sm btn-primary"
                  :disabled="chatting || frozen || !canConfirmPlan"
                  @click="emit('confirmPlan', { messageId: block.sourceMessageId || block.id })"
                >
                  {{ t("chat.plan.confirmAction") }}
                </button>
                <div class="text-xs opacity-60">{{ t("chat.plan.confirmHint") }}</div>
              </div>
            </div>
            <div v-if="block.taskTrigger" class="space-y-2">
        <div class="flex items-center gap-2">
          <span class="badge badge-sm badge-ghost">{{ t("chat.taskTrigger.badge") }}</span>
        </div>
        <div v-if="block.taskTrigger.taskId" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.taskId") }}</div>
          <div class="font-mono text-xs leading-6 break-all">{{ block.taskTrigger.taskId }}</div>
        </div>
        <div v-if="block.taskTrigger.goal" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.goal") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.goal }}</div>
        </div>
        <div v-if="block.taskTrigger.why" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.why") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.why }}</div>
        </div>
        <div v-if="block.taskTrigger.todo" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.todo") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.todo }}</div>
        </div>
        <div v-if="block.taskTrigger.runAtLocal || block.taskTrigger.endAtLocal || block.taskTrigger.nextRunAtLocal || block.taskTrigger.everyMinutes" class="grid gap-1 text-sm leading-6">
          <div v-if="block.taskTrigger.runAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.runAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.runAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.nextRunAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.nextRunAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.nextRunAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.endAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.endAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.endAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.everyMinutes">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.everyMinutes") }}</span>
            <span class="ml-2">{{ block.taskTrigger.everyMinutes }}</span>
          </div>
        </div>
      </div>
      <div v-if="!isOwnMessage(block) && block.reasoningStandard" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 hover:bg-base-200">
            <span class="inline-block shrink-0 text-[10px] leading-none text-success">▲</span>
            <span
              :class="['block min-w-0 flex-1 truncate ecall-shimmer-text font-medium', reasoningSummaryClass(block)]"
              :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div class="collapse-content px-0 pb-1 pt-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70">
            {{ block.reasoningStandard }}
          </div>
        </details>
      </div>
      <div v-if="!isOwnMessage(block) && resolvedInlineReasoning(block)" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 cursor-pointer hover:bg-base-200">
            <span class="inline-block shrink-0 text-[10px] leading-none text-success">▲</span>
            <span
              :class="['block min-w-0 flex-1 truncate ecall-shimmer-text font-medium', reasoningSummaryClass(block)]"
              :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div class="collapse-content px-0 pb-1 pt-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70">
            {{ resolvedInlineReasoning(block) }}
          </div>
        </details>
      </div>
      <div v-if="toolCallsForBlock(block).length > 0" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 hover:bg-base-200">
            <span class="inline-block h-2 w-2 rounded-full bg-success"></span>
            <span
              :class="['ecall-shimmer-text font-medium', toolSummaryClass(block)]"
              :data-shimmer-text="showStreamingUi(block) ? '工具执行中' : ''"
            >{{ toolStatusLabel(block) }}</span>
            <span v-if="toolNamesLabel(block)" class="truncate">{{ ` · ${toolNamesLabel(block)}` }}</span>
          </summary>
          <div class="collapse-content px-0 pb-1 pt-2 text-xs text-base-content/70">
            <ul class="timeline timeline-vertical timeline-compact">
              <li
                v-for="(toolCall, idx) in toolCallsForBlock(block)"
                :key="`${block.id}-tool-${idx}`"
              >
                <div class="timeline-start pr-2 text-xs font-semibold opacity-55">#{{ idx + 1 }}</div>
                <div class="timeline-middle">
                  <span
                    class="inline-block h-2.5 w-2.5 rounded-full"
                    :class="toolTimelineDotClass(block, toolCall)"
                  ></span>
                </div>
                <div class="timeline-end mb-3 w-full min-w-0 pb-3 pl-3">
                  <div class="mb-1 flex items-center gap-2 text-xs font-semibold opacity-85">
                    <span>{{ toolCall.name }}</span>
                    <span
                      v-if="showStreamingUi(block)"
                      class="badge badge-ghost badge-xs font-normal"
                      :class="toolCall.status === 'doing' ? 'text-primary border-primary/35' : 'text-success border-success/35'"
                    >{{ toolCall.status === "doing" ? "doing" : "done" }}</span>
                  </div>
                  <pre
                    v-if="toolCall.argsText"
                    class="whitespace-pre-wrap break-all text-xs leading-relaxed text-base-content/70"
                  >{{ toolCall.argsText }}</pre>
                </div>
                <hr
                  v-if="idx < toolCallsForBlock(block).length - 1"
                  :class="toolTimelineHrClass(block, toolCall)"
                />
              </li>
            </ul>
          </div>
        </details>
      </div>
      <div v-if="block.text" :class="block.taskTrigger ? 'mt-3' : ''">
        <div
          v-if="isOwnMessage(block)"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ block.text }}</div>
        <div ref="markdownContainerRef">
          <MarkdownRender
            class="ecall-markdown-content max-w-none"
            custom-id="chat-markstream"
            :nodes="markdownNodesForBlock(block)"
            :is-dark="markdownIsDark"
            :final="!block.isStreaming"
            :max-live-nodes="0"
            :batch-rendering="!!block.isStreaming"
            :initial-render-batch-size="block.isStreaming ? STREAM_INITIAL_RENDER_BATCH_SIZE : 0"
            :render-batch-size="block.isStreaming ? STREAM_RENDER_BATCH_SIZE : 0"
            :render-batch-delay="block.isStreaming ? STREAM_RENDER_BATCH_DELAY : 0"
            :render-batch-budget-ms="block.isStreaming ? STREAM_RENDER_BATCH_BUDGET_MS : 0"
            :code-block-props="markdownCodeBlockProps"
            :mermaid-props="markdownMermaidProps"
            :typewriter="true"
            @click="emit('assistantLinkClick', $event)"
          />
        </div>
      </div>
      <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
        <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
          <img
            v-if="isImageMime(img.mime) && resolvedImageSrc(img, idx)"
            :src="resolvedImageSrc(img, idx)"
            loading="lazy"
            decoding="async"
            class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
            @dblclick.stop="openResolvedImagePreview(img, idx)"
          />
          <div
            v-else-if="isImageMime(img.mime)"
            class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-[11px] text-base-content/55"
          >
            <span class="loading loading-spinner loading-xs mr-2"></span>
            <span>图片加载中</span>
          </div>
          <div v-else-if="isPdfMime(img.mime)" class="badge badge-ghost gap-1 py-3 w-fit">
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
          @click="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
        >
          <Pause v-if="playingAudioId === `${block.id}-aud-${idx}`" class="h-3 w-3" />
          <Play v-else class="h-3 w-3" />
          <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
        </button>
      </div>
      <div
        v-if="block.attachmentFiles.length > 0"
        :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap gap-1' : 'flex flex-wrap gap-1'"
      >
        <div
          v-for="(file, idx) in block.attachmentFiles"
          :key="`${block.id}-file-${idx}`"
          class="badge badge-ghost gap-1 py-3"
          :title="file.relativePath"
        >
          <FileText class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ file.fileName }}</span>
        </div>
            </div>
          </div>
          <div
            :class="[
              'chat-footer mt-1 flex h-6 items-center gap-1.5 transition-opacity',
              canRegenerate
                ? 'opacity-100 pointer-events-auto'
                : !chatting && !frozen
                  ? 'opacity-0 pointer-events-none group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto'
                  : 'opacity-0 pointer-events-none',
            ]"
          >
            <button
              type="button"
              class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.copy')"
              :class="!block.isStreaming && !chatting && !frozen ? '' : 'opacity-0 pointer-events-none'"
              :disabled="block.isStreaming || chatting || frozen"
              @click="emit('copyMessage', block)"
            >
              <Copy class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.regenerate')"
              :class="!block.isStreaming && !chatting && !frozen && canRegenerate ? '' : 'opacity-0 pointer-events-none'"
              :disabled="block.isStreaming || chatting || frozen || !canRegenerate"
              @click="emit('regenerateTurn', { turnId: block.sourceMessageId || block.id })"
            >
              <RotateCcw class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>
    </template>
    <template v-else>
      <div class="chat-header mb-1 flex items-center gap-2">
        <button
          v-if="isOwnMessage(block)"
          type="button"
          :class="[
            'inline-flex h-5 w-5 items-center justify-center rounded text-base-content/40 hover:text-base-content opacity-0 pointer-events-none transition-opacity',
            !chatting && !frozen ? 'group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto' : '',
          ]"
          :title="t('chat.recall')"
          :disabled="chatting || frozen"
          @click="emit('recallTurn', { turnId: block.sourceMessageId || block.id })"
        >
          <Undo2 class="h-3 w-3" />
        </button>
        <span v-if="isOwnMessage(block)" class="inline-flex h-4 items-center text-[10px] leading-none">
          <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
            <span class="loading loading-infinity loading-sm"></span>
          </span>
          <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
        </span>
        <span class="text-xs text-base-content">{{ displayName }}</span>
        <span v-if="!isOwnMessage(block)" class="inline-flex h-4 items-center text-[10px] leading-none">
          <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
            <span class="loading loading-infinity loading-sm"></span>
          </span>
          <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
        </span>
      </div>
      <div :class="[
        'chat-bubble',
        isOwnMessage(block)
          ? ''
          : [
            'self-start bg-base-100 text-base-content border border-base-300/70 assistant-markdown ecall-assistant-bubble max-w-full',
            blockNeedsWideBubble(block) ? 'ecall-assistant-bubble-wide' : '',
          ],
      ]">
        <div
          v-if="block.text"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ block.text }}</div>
        <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
          <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
            <img
              v-if="isImageMime(img.mime) && resolvedImageSrc(img, idx)"
              :src="resolvedImageSrc(img, idx)"
              loading="lazy"
              decoding="async"
              class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
              @dblclick.stop="openResolvedImagePreview(img, idx)"
            />
            <div
              v-else-if="isImageMime(img.mime)"
              class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-[11px] text-base-content/55"
            >
              <span class="loading loading-spinner loading-xs mr-2"></span>
              <span>图片加载中</span>
            </div>
            <div v-else-if="isPdfMime(img.mime)" class="badge badge-ghost gap-1 py-3 w-fit">
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
            @click="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
          >
            <Pause v-if="playingAudioId === `${block.id}-aud-${idx}`" class="h-3 w-3" />
            <Play v-else class="h-3 w-3" />
            <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
          </button>
        </div>
        <div
          v-if="block.attachmentFiles.length > 0"
          :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap gap-1' : 'flex flex-wrap gap-1'"
        >
          <div
            v-for="(file, idx) in block.attachmentFiles"
            :key="`${block.id}-file-${idx}`"
            class="badge badge-ghost gap-1 py-3"
            :title="file.relativePath"
          >
            <FileText class="h-3.5 w-3.5" />
            <span class="text-[11px]">{{ file.fileName }}</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watchEffect, watchPostEffect } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, FileText, Pause, Play, RotateCcw, Undo2 } from "lucide-vue-next";
import MarkdownRender, { enableKatex, enableMermaid, getMarkdown, parseMarkdownToStructure } from "markstream-vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessageBlock } from "../../../types/app";
import { formatIsoToLocalHourMinute } from "../../../utils/time";
import { registerChatMarkstreamComponents } from "../markdown/register-chat-markstream";
import { normalizeLocalLinkHref } from "../utils/local-link";

enableMermaid();
enableKatex();
registerChatMarkstreamComponents();

const STREAM_MARKDOWN_PARSE_THROTTLE_MS = 100;
const MARKDOWN_NODE_CACHE_LIMIT = 100;
const STREAM_INITIAL_RENDER_BATCH_SIZE = 20;
const STREAM_RENDER_BATCH_SIZE = 10;
const STREAM_RENDER_BATCH_DELAY = 24;
const STREAM_RENDER_BATCH_BUDGET_MS = 4;
const markstreamMarkdown = getMarkdown();
const markdownNodeCache = new Map<string, { text: string; final: boolean; nodes: any[]; lastParseTime: number }>();
const markdownCodeBlockProps = {
  showHeader: true,
  showCopyButton: true,
  showPreviewButton: false,
  showExpandButton: true,
  showCollapseButton: true,
  showFontSizeButtons: false,
  enableFontSizeControl: false,
  isShowPreview: false,
};
const markdownMermaidProps = {
  showHeader: true,
  showCopyButton: true,
  showExportButton: false,
  showFullscreenButton: true,
  showCollapseButton: false,
  showZoomControls: true,
  showModeToggle: false,
  enableWheelZoom: true,
};
const imageDataUrlCache = new Map<string, string>();
const imageDataUrlPromiseCache = new Map<string, Promise<string>>();

const props = defineProps<{
  block: ChatMessageBlock;
  chatting: boolean;
  frozen: boolean;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
  markdownIsDark: boolean;
  playingAudioId: string;
  activeTurnUser: boolean;
  canRegenerate: boolean;
  canConfirmPlan: boolean;
}>();

const emit = defineEmits<{
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "confirmPlan", payload: { messageId: string }): void;
  (e: "copyMessage", block: ChatMessageBlock): void;
  (e: "openImagePreview", image: { mime: string; bytesBase64?: string; dataUrl?: string }): void;
  (e: "toggleAudioPlayback", payload: { id: string; audio: { mime: string; bytesBase64: string } }): void;
  (e: "assistantLinkClick", event: MouseEvent): void;
}>();

const { t } = useI18n();
const resolvedImageSrcMap = ref<Record<string, string>>({});
const markdownContainerRef = ref<HTMLElement | null>(null);
let disposed = false;

const displayName = computed(() => messageName(props.block));
const avatarUrl = computed(() => messageAvatarUrl(props.block));
const formattedCreatedAt = computed(() => formattedBlockTime(props.block.createdAt));

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function messageName(block: ChatMessageBlock): string {
  if (block.remoteImOrigin) {
    return block.remoteImOrigin.senderName || block.remoteImOrigin.remoteContactName || "IM";
  }
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaNameMap[id]) return props.personaNameMap[id];
  if (!id || id === "user-persona") return props.userAlias || t("archives.roleUser");
  return id;
}

function messageAvatarUrl(block: ChatMessageBlock): string {
  if (block.remoteImOrigin) return "";
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaAvatarUrlMap[id]) return props.personaAvatarUrlMap[id];
  if (!id || id === "user-persona") return props.userAvatarUrl || "";
  return "";
}

function isOwnMessage(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) return false;
  const id = String(block.speakerAgentId || "").trim();
  return !id || id === "user-persona";
}

function showStreamingUi(block: ChatMessageBlock): boolean {
  return !!block.isStreaming && !isOwnMessage(block);
}

function toolCallsForBlock(block: ChatMessageBlock): Array<{ name: string; argsText: string; status?: "doing" | "done" }> {
  if (showStreamingUi(block) && props.streamToolCalls.length > 0) {
    return props.streamToolCalls;
  }
  return block.toolCalls;
}

function toolStatusLabel(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return "工具执行毕";
  return toolCallsForBlock(block).some((call) => call.status === "doing") ? "工具执行中" : "工具执行毕";
}

function toolTimelineDotClass(block: ChatMessageBlock, toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
  if (!showStreamingUi(block)) return "bg-success";
  return toolCall.status === "doing" ? "bg-primary" : "bg-success";
}

function toolTimelineHrClass(block: ChatMessageBlock, toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
  if (!showStreamingUi(block)) return "bg-success/35";
  return toolCall.status === "doing" ? "bg-primary/35" : "bg-success/35";
}

function toolNamesLabel(block: ChatMessageBlock): string {
  const calls = toolCallsForBlock(block);
  if (calls.length === 0) return "";
  const counts = new Map<string, number>();
  const order: string[] = [];
  for (const call of calls) {
    const name = String(call.name || "").trim() || "未知工具";
    if (!counts.has(name)) {
      counts.set(name, 0);
      order.push(name);
    }
    counts.set(name, (counts.get(name) || 0) + 1);
  }
  return order
    .map((name) => {
      const total = counts.get(name) || 0;
      return total > 1 ? `${name}（+${total - 1}）` : name;
    })
    .join("，");
}

function formattedBlockTime(value?: string): string {
  return formatIsoToLocalHourMinute(value, "");
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

function markdownNodesForText(
  block: ChatMessageBlock,
  rawText: string,
  final: boolean,
  cacheSuffix = "main",
): any[] {
  const text = splitThinkText(rawText).visible;
  const cacheKey = `${String(block.id || "")}::${cacheSuffix}`;
  const cached = markdownNodeCache.get(cacheKey);
  if (cached && cached.text === text && cached.final === final) {
    markdownNodeCache.delete(cacheKey);
    markdownNodeCache.set(cacheKey, cached);
    return cached.nodes;
  }

  const now = Date.now();
  if (!final && cached && now - cached.lastParseTime < STREAM_MARKDOWN_PARSE_THROTTLE_MS) {
    return cached.nodes;
  }

  const nodes = parseMarkdownToStructure(text, markstreamMarkdown, { final });
  if (markdownNodeCache.size >= MARKDOWN_NODE_CACHE_LIMIT) {
    const oldestKey = markdownNodeCache.keys().next().value;
    if (typeof oldestKey === "string") markdownNodeCache.delete(oldestKey);
  }
  markdownNodeCache.set(cacheKey, { text, final, nodes, lastParseTime: now });
  return nodes;
}

function markdownNodesForBlock(block: ChatMessageBlock): any[] {
  return markdownNodesForText(block, block.text, !block.isStreaming, "main");
}

function normalizeRenderedLocalLinks() {
  const container = markdownContainerRef.value;
  if (!container) return;
  const anchors = Array.from(container.querySelectorAll("a[href]"));
  for (const anchor of anchors) {
    const rawHref = anchor.getAttribute("href")?.trim() || "";
    const normalizedHref = normalizeLocalLinkHref(rawHref);
    if (normalizedHref && normalizedHref !== rawHref) {
      anchor.setAttribute("href", normalizedHref);
    }
  }
}

function blockHasMermaid(block: ChatMessageBlock): boolean {
  const text = splitThinkText(block.text).visible;
  return /```(?:\s*)mermaid\b/i.test(text);
}

function blockHasCodeFence(block: ChatMessageBlock): boolean {
  const text = splitThinkText(block.text).visible;
  return /```[\w-]*\s*[\r\n]/i.test(text);
}

function blockNeedsWideBubble(block: ChatMessageBlock): boolean {
  return blockHasMermaid(block) || blockHasCodeFence(block);
}

function resolvedInlineReasoning(block: ChatMessageBlock): string {
  return splitThinkText(block.text).inline || block.reasoningInline || "";
}

function reasoningSummaryLabel(block: ChatMessageBlock): string {
  if (block.isStreaming) return "正在思考中";
  const elapsedMs = Number((block as ChatMessageBlock & { reasoningElapsedMs?: number }).reasoningElapsedMs ?? 0);
  if (!Number.isFinite(elapsedMs) || elapsedMs <= 0) return "思考完成";
  const elapsedSeconds = Math.max(1, Math.round(elapsedMs / 1000));
  return `思考了${elapsedSeconds}秒`;
}

function reasoningSummaryClass(block: ChatMessageBlock): string {
  return block.isStreaming ? "ecall-reasoning-shimmer" : "";
}

function toolSummaryClass(block: ChatMessageBlock): string {
  return showStreamingUi(block) ? "ecall-reasoning-shimmer" : "";
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function imageCacheKey(image: { mime: string; bytesBase64?: string; mediaRef?: string }): string {
  const mime = String(image.mime || "").trim().toLowerCase();
  const mediaRef = String(image.mediaRef || "").trim();
  if (mediaRef) return `${mime}::${mediaRef}`;
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  return `${mime}::inline::${bytesBase64}`;
}

function imageRenderKey(index: number): string {
  return `${String(props.block.id || "").trim() || "message"}::${index}`;
}

async function loadImageDataUrl(image: { mime: string; bytesBase64?: string; mediaRef?: string }): Promise<string> {
  const mime = String(image.mime || "").trim() || "image/webp";
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  if (bytesBase64) {
    return `data:${mime};base64,${bytesBase64}`;
  }
  const mediaRef = String(image.mediaRef || "").trim();
  if (!mediaRef) return "";
  const cacheKey = imageCacheKey(image);
  const cached = imageDataUrlCache.get(cacheKey);
  if (cached) return cached;
  const pending = imageDataUrlPromiseCache.get(cacheKey);
  if (pending) return pending;
  const task = invokeTauri<{ dataUrl: string }>("read_chat_image_data_url", {
    input: {
      mediaRef,
      mime,
    },
  })
    .then((result) => {
      const dataUrl = String(result?.dataUrl || "").trim();
      if (dataUrl) imageDataUrlCache.set(cacheKey, dataUrl);
      imageDataUrlPromiseCache.delete(cacheKey);
      return dataUrl;
    })
    .catch((error) => {
      imageDataUrlPromiseCache.delete(cacheKey);
      throw error;
    });
  imageDataUrlPromiseCache.set(cacheKey, task);
  return task;
}

watchEffect(() => {
  const nextEntries = props.block.images
    .map((image, index) => {
      const src = image.bytesBase64
        ? `data:${image.mime};base64,${image.bytesBase64}`
        : "";
      return [imageRenderKey(index), src] as const;
    })
    .filter((entry) => !!entry[1]);
  if (nextEntries.length <= 0) return;
  resolvedImageSrcMap.value = {
    ...resolvedImageSrcMap.value,
    ...Object.fromEntries(nextEntries),
  };
});

watchEffect(() => {
  for (const [index, image] of props.block.images.entries()) {
    if (!isImageMime(image.mime) || image.bytesBase64 || !image.mediaRef) continue;
    const key = imageRenderKey(index);
    if (resolvedImageSrcMap.value[key]) continue;
    void loadImageDataUrl(image)
      .then((dataUrl) => {
        if (!dataUrl || disposed) return;
        resolvedImageSrcMap.value = {
          ...resolvedImageSrcMap.value,
          [key]: dataUrl,
        };
      })
      .catch((error) => {
        console.warn("[聊天图片] 懒加载失败", {
          messageId: props.block.id,
          mediaRef: image.mediaRef,
          error,
        });
      });
  }
});

watchPostEffect(() => {
  void nextTick(() => {
    normalizeRenderedLocalLinks();
  });
});

onBeforeUnmount(() => {
  disposed = true;
});

function resolvedImageSrc(
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
): string {
  const direct = String(image.bytesBase64 || "").trim();
  if (direct) return `data:${image.mime};base64,${direct}`;
  return String(resolvedImageSrcMap.value[imageRenderKey(index)] || "").trim();
}

function openResolvedImagePreview(
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
) {
  const dataUrl = resolvedImageSrc(image, index);
  if (!dataUrl) return;
  emit("openImagePreview", {
    mime: image.mime,
    dataUrl,
  });
}
</script>

<style scoped>
.ecall-chat-avatar-col {
  width: 1.75rem;
  min-width: 1.75rem;
}

.ecall-message-stack {
  min-height: 100%;
  flex: 1 1 auto;
  width: 100%;
}

.ecall-message-content {
  min-width: 0;
  flex: 0 1 auto;
}

.ecall-message-content-wide {
  width: 100%;
  max-width: none;
}

.ecall-time-loading {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  transform: scale(0.82);
  transform-origin: right center;
}

.ecall-message-enter {
  animation: ecall-message-enter 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

.ecall-shimmer-text {
  position: relative;
  display: inline-block;
  color: currentColor;
}

.ecall-reasoning-shimmer::after {
  content: attr(data-shimmer-text);
  position: absolute;
  inset: 0;
  pointer-events: none;
  color: transparent;
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    transparent 44%,
    rgb(255 255 255 / 0.92) 50%,
    transparent 56%,
    transparent 100%
  );
  background-size: 280px 100%;
  background-position: 280px 0;
  will-change: background-position;
  transform: translateZ(0);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: ecall-reasoning-shimmer 2.5s linear infinite;
}

@keyframes ecall-message-enter {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes ecall-reasoning-shimmer {
  from {
    background-position: 280px 0;
  }
  to {
    background-position: -280px 0;
  }
}

.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: hsl(var(--bc));
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

.assistant-markdown :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
  font-size: 0.9rem;
  line-height: 1.5;
}

.assistant-markdown :deep(.ecall-markdown-content.markdown-renderer) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-from) {
  opacity: 0;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-active) {
  transition: opacity 1000ms cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-to) {
  opacity: 1;
}

.assistant-markdown :deep(.ecall-markdown-content .code-block-container),
.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.ecall-markdown-content > :first-child) {
  margin-top: 0;
}

.assistant-markdown :deep(.ecall-markdown-content > :last-child) {
  margin-bottom: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(p,ul,ol,blockquote,pre,table,figure)) {
  margin-top: 0.25rem;
  margin-bottom: 0.25rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4)) {
  margin-top: 0.7rem;
  margin-bottom: 0.32rem;
  font-weight: 600;
  line-height: 1.5;
  letter-spacing: -0.015em;
}

.assistant-markdown :deep(.ecall-markdown-content h1) {
  font-size: 1.12rem;
}

.assistant-markdown :deep(.ecall-markdown-content h2) {
  font-size: 1.04rem;
}

.assistant-markdown :deep(.ecall-markdown-content h3) {
  font-size: 0.98rem;
}

.assistant-markdown :deep(.ecall-markdown-content h4) {
  font-size: 0.94rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol)) {
  padding-left: 1.05rem;
}

.assistant-markdown :deep(.ecall-markdown-content li) {
  margin: 0.12rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content li > :where(p,ul,ol)) {
  margin-top: 0.16rem;
  margin-bottom: 0.16rem;
}

.assistant-markdown :deep(.ecall-markdown-content a) {
  text-decoration: underline;
  text-underline-offset: 0.18em;
  text-decoration-color: hsl(var(--bc) / 0.28);
}

.assistant-markdown :deep(.ecall-markdown-content a:hover) {
  text-decoration-color: hsl(var(--bc) / 0.5);
}

.assistant-markdown :deep(.ecall-markdown-content strong) {
  font-weight: 600;
}

.assistant-markdown :deep(.ecall-markdown-content blockquote) {
  border-left: 3px solid hsl(var(--bc) / 0.16);
  padding-left: 0.68rem;
  color: hsl(var(--bc) / 0.82);
}

.assistant-markdown :deep(.ecall-markdown-content hr) {
  border: 0;
  border-top: 1px solid hsl(var(--bc) / 0.14);
  margin: 0.65rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content :not(pre) > code) {
  border: 1px solid hsl(var(--bc) / 0.12);
  background: hsl(var(--b2));
  border-radius: 0.4rem;
  padding: 0.08rem 0.3rem;
  font-size: 0.86em;
  font-weight: 500;
}


.assistant-markdown :deep(.ecall-markdown-content table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}

.assistant-markdown :deep(.ecall-markdown-content th) {
  border-bottom: 1px solid hsl(var(--bc) / 0.16);
  padding: 0.36rem 0.5rem;
  text-align: left;
  font-weight: 600;
}

.assistant-markdown :deep(.ecall-markdown-content td) {
  border-bottom: 1px solid hsl(var(--bc) / 0.1);
  padding: 0.34rem 0.5rem;
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
}

:deep(.chat-bubble) {
  min-width: 0;
  min-height: 0;
}

.ecall-assistant-bubble {
  min-width: 3rem;
  min-height: 2.25rem;
  transition:
    box-shadow 220ms ease,
    transform 220ms ease,
    border-color 220ms ease,
    background-color 220ms ease;
  transform-origin: top left;
}

.ecall-assistant-bubble-wide {
  display: block;
  width: 100%;
  max-width: none;
}
</style>
