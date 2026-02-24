<template>
  <div class="flex flex-col h-full min-h-0 relative">
    <div
      v-if="mediaDragActive && !chatting && !frozen"
      class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
    >
      <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
        Drop image or PDF
      </div>
    </div>
    <div
      ref="scrollContainer"
      class="flex-1 min-h-0 overflow-y-auto p-3 flex flex-col"
      @scroll="onScroll"
      @wheel.passive="onWheel"
    >
      <!-- 历史对话 turns -->
      <template v-for="turn in turns" :key="turn.id">
        <div class="chat chat-end group/user-turn">
          <div class="chat-header mb-1 flex items-center gap-2">
            <button
              v-if="!chatting && !frozen"
              type="button"
              class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content opacity-0 pointer-events-none transition-opacity group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto"
              :title="t('chat.recall')"
              @click="$emit('recallTurn', { turnId: turn.id })"
            >
              <Undo2 class="h-3.5 w-3.5" />
            </button>
            <div v-if="userAvatarUrl" class="avatar">
              <div class="w-7 rounded-full">
                <img :src="userAvatarUrl" :alt="userAlias || t('archives.roleUser')" :title="userAlias || t('archives.roleUser')" />
              </div>
            </div>
            <div v-else class="avatar placeholder">
              <div class="bg-neutral text-neutral-content w-7 rounded-full">
                <span>{{ avatarInitial(userAlias || t("archives.roleUser")) }}</span>
              </div>
            </div>
          </div>
          <div class="chat-bubble max-w-[92%]">
            <div v-if="turn.userText" class="whitespace-pre-wrap">{{ turn.userText }}</div>
            <div v-if="turn.userImages.length > 0" class="mt-2 grid gap-1">
              <template v-for="(img, idx) in turn.userImages" :key="`${turn.id}-img-${idx}`">
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
            <div v-if="turn.userAudios.length > 0" class="mt-2 flex flex-col gap-1">
              <button
                v-for="(aud, idx) in turn.userAudios"
                :key="`${turn.id}-aud-${idx}`"
                class="btn btn-xs bg-base-100/70 w-fit"
                @click="toggleAudioPlayback(`${turn.id}-aud-${idx}`, aud)"
              >
                <Pause v-if="playingAudioId === `${turn.id}-aud-${idx}`" class="h-3 w-3" />
                <Play v-else class="h-3 w-3" />
                <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
              </button>
            </div>
          </div>
        </div>
        <div v-if="turn.assistantText || turn.assistantReasoningStandard || turn.assistantReasoningInline" class="chat chat-start">
          <div class="chat-header mb-1 flex items-center gap-1">
            <div v-if="turnAssistantAvatarUrl(turn)" class="avatar">
              <div class="w-7 rounded-full">
                <img :src="turnAssistantAvatarUrl(turn)" :alt="turnAssistantName(turn)" :title="turnAssistantName(turn)" />
              </div>
            </div>
            <div v-else class="avatar placeholder">
              <div class="bg-neutral text-neutral-content w-7 rounded-full">
                <span>{{ avatarInitial(turnAssistantName(turn)) }}</span>
              </div>
            </div>
            <details
              v-if="turn.assistantReasoningStandard"
              class="collapse bg-base-200 min-w-0 max-w-[min(90vw,40rem)]"
            >
              <summary class="collapse-title py-2 px-3 min-h-0 text-xs italic flex items-center">
                <span class="block min-w-0 flex-1 whitespace-normal break-words">
                  {{ firstLinePreview(turn.assistantReasoningStandard) || "..." }}
                </span>
              </summary>
              <div class="collapse-content px-3 pb-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/80">
                {{ turn.assistantReasoningStandard }}
              </div>
            </details>
          </div>
          <div v-if="turn.assistantText" class="chat-bubble max-w-[92%] bg-base-100 text-base-content border border-base-300 assistant-markdown">
            <details
              v-if="resolvedTurnInlineReasoning(turn)"
              class="collapse border border-base-content/10 bg-base-200/50 mb-2"
            >
              <summary class="collapse-title py-1.5 px-2.5 min-h-0 text-[11px] italic flex items-center text-base-content/60 cursor-pointer">
                <span class="block min-w-0 flex-1 whitespace-normal break-words">
                  {{ firstLinePreview(resolvedTurnInlineReasoning(turn)) || "..." }}
                </span>
              </summary>
              <div class="collapse-content max-w-full px-2.5 pb-1.5 whitespace-pre-wrap break-words text-[11px] leading-relaxed text-base-content/60" style="overflow-wrap: anywhere;">
                {{ resolvedTurnInlineReasoning(turn) }}
              </div>
            </details>
            <div
              v-html="renderMarkdown(splitThinkText(turn.assistantText).visible)"
              @click="handleAssistantLinkClick"
            ></div>
            <div v-if="!chatting && !frozen" class="mt-2 flex items-center gap-1.5">
              <button
                type="button"
                class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
                :title="t('chat.copy')"
                @click="copyAssistantTurn(turn)"
              >
                <Copy class="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
                :title="t('chat.regenerate')"
                @click="$emit('regenerateTurn', { turnId: turn.id })"
              >
                <RotateCcw class="h-3.5 w-3.5" />
              </button>
            </div>
            <div v-if="turn.assistantToolCallCount > 0" class="mt-1 text-[11px] opacity-80 flex items-center gap-1">
              <span class="inline-block w-1.5 h-1.5 rounded-full bg-success"></span>
              <span>{{ toolSummaryText(turn.assistantLastToolName, turn.assistantToolCallCount) }}</span>
            </div>
          </div>
        </div>
      </template>

      <!-- 发送中的即时反馈 -->
      <template v-if="chatting">
        <!-- 用户消息 (与历史消息样式一致) -->
        <div class="chat chat-end">
          <div class="chat-header mb-1">
            <div v-if="userAvatarUrl" class="avatar">
              <div class="w-7 rounded-full">
                <img :src="userAvatarUrl" :alt="userAlias || t('archives.roleUser')" :title="userAlias || t('archives.roleUser')" />
              </div>
            </div>
            <div v-else class="avatar placeholder">
              <div class="bg-neutral text-neutral-content w-7 rounded-full">
                <span>{{ avatarInitial(userAlias || t("archives.roleUser")) }}</span>
              </div>
            </div>
          </div>
          <div class="chat-bubble max-w-[92%]">
            <div v-if="latestUserText" class="whitespace-pre-wrap">{{ latestUserText }}</div>
            <div v-if="latestUserImages.length > 0" class="mt-2 grid gap-1">
              <template v-for="(img, idx) in latestUserImages" :key="`streaming-user-img-${idx}`">
                <img v-if="isImageMime(img.mime)" :src="`data:${img.mime};base64,${img.bytesBase64}`" loading="lazy" decoding="async" class="rounded max-h-28 object-contain bg-base-100/40" />
                <div v-else-if="isPdfMime(img.mime)" class="badge badge-outline gap-1 py-3 w-fit">
                  <FileText class="h-3.5 w-3.5" />
                  <span class="text-[11px]">PDF</span>
                </div>
              </template>
            </div>
          </div>
        </div>
        <!-- 助手流式响应 -->
        <div class="chat chat-start">
          <div class="chat-header mb-1 flex items-center gap-1">
            <div v-if="assistantAvatarUrl" class="avatar">
              <div class="w-7 rounded-full">
                <img :src="assistantAvatarUrl" :alt="personaName || t('archives.roleAssistant')" :title="personaName || t('archives.roleAssistant')" />
              </div>
            </div>
            <div v-else class="avatar placeholder">
              <div class="bg-neutral text-neutral-content w-7 rounded-full">
                <span>{{ avatarInitial(personaName || t("archives.roleAssistant")) }}</span>
              </div>
            </div>
            <details
              v-if="latestReasoningStandardText"
              class="collapse bg-base-200 min-w-0 max-w-[min(90vw,40rem)]"
            >
              <summary class="collapse-title py-2 px-3 min-h-0 text-xs italic flex items-center gap-1">
                <span class="block min-w-0 flex-1 whitespace-normal break-words">{{ firstLinePreview(latestReasoningStandardText) || "..." }}</span>
                <span class="loading loading-dots loading-xs opacity-60"></span>
              </summary>
              <div class="collapse-content px-3 pb-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/80">
                {{ latestReasoningStandardText }}
              </div>
            </details>
          </div>
          <div class="chat-bubble max-w-[92%] bg-base-100 text-base-content border border-base-300 assistant-markdown">
            <details
              v-if="latestInlineReasoningText"
              class="collapse border border-base-content/10 bg-base-200/50 mb-2"
            >
              <summary class="collapse-title py-1.5 px-2.5 min-h-0 text-[11px] italic flex items-center gap-1 text-base-content/60 cursor-pointer">
                <span class="block min-w-0 flex-1 whitespace-normal break-words">{{ firstLinePreview(latestInlineReasoningText) || "..." }}</span>
                <span class="loading loading-dots loading-xs opacity-60"></span>
              </summary>
              <div class="collapse-content max-w-full px-2.5 pb-1.5 whitespace-pre-wrap break-words text-[11px] leading-relaxed text-base-content/60" style="overflow-wrap: anywhere;">
                {{ latestInlineReasoningText }}
              </div>
            </details>
            <div v-if="latestAssistantText" v-html="renderedAssistantHtml" @click="handleAssistantLinkClick"></div>
            <div class="mt-1">
              <span v-if="!latestAssistantText" class="loading loading-dots loading-sm"></span>
              <span v-else class="loading loading-spinner loading-xs opacity-60"></span>
            </div>
            <div v-if="toolStatusText" class="mt-1 text-[11px] opacity-80 flex items-center gap-1">
              <span v-if="toolStatusState === 'running'" class="loading loading-spinner loading-xs"></span>
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
            class="btn btn-xs bg-base-100"
            :title="workspaceLocked ? '已锁定，点击还原到默认工作空间' : '未锁定'"
            :disabled="chatting || frozen || !workspaceLocked"
            @click="$emit('unlockWorkspace')"
          >
            <Lock v-if="workspaceLocked" class="h-3.5 w-3.5" />
            <LockOpen v-else class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-xs bg-base-100"
            :disabled="chatting || frozen"
            @click="$emit('lockWorkspace')"
          >
            {{ currentWorkspaceName }}{{ workspaceLocked ? " (临时)" : "" }}
          </button>
          <button
            class="btn btn-xs bg-base-100 ml-auto"
            :disabled="chatting || frozen"
            @click="$emit('openSkillList')"
          >
            技能
          </button>
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
      <div
        v-if="linkOpenErrorText"
        class="alert alert-warning mb-2 py-2 px-3 text-xs whitespace-pre-wrap break-all max-h-24 overflow-auto"
      >
        <span>{{ linkOpenErrorText }}</span>
      </div>
      <div
        v-if="chatErrorText"
        class="alert alert-error mb-2 py-2 px-3 text-xs whitespace-pre-wrap break-all max-h-28 overflow-auto"
      >
        <span>{{ chatErrorText }}</span>
      </div>
      <div v-if="clipboardImages.length > 0" class="flex flex-wrap gap-1 mb-2">
        <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-outline gap-1 py-3">
          <ImageIcon v-if="isImageMime(img.mime)" class="h-3.5 w-3.5" />
          <FileText v-else-if="isPdfMime(img.mime)" class="h-3.5 w-3.5" />
          <ImageIcon v-else class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ isPdfMime(img.mime) ? `PDF ${idx + 1}` : t("chat.image", { index: idx + 1 }) }}</span>
          <button class="btn btn-ghost btn-xs btn-square" :disabled="chatting || frozen" @click="$emit('removeClipboardImage', idx)">
            <X class="h-3 w-3" />
          </button>
        </div>
      </div>
      <div v-if="transcribing" class="mb-1 text-[11px] opacity-80 flex items-center gap-1">
        <span class="loading loading-spinner loading-xs"></span>
        <span>语音转写中...</span>
      </div>
      <div class="flex flex-row items-center gap-2">
        <button
          class="btn btn-xs btn-circle shrink-0"
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
        <textarea
          ref="chatInputRef"
          v-model="localChatInput"
          class="flex-1 textarea textarea-xs resize-none overflow-y-hidden chat-input-no-focus min-h-[50px]"
          rows="1"
          :disabled="frozen"
          :placeholder="chatInputPlaceholder"
          @input="scheduleResizeChatInput"
          @keydown.enter.exact.prevent="!frozen && $emit('sendChat')"
        ></textarea>
        <button class="btn btn-xs btn-circle shrink-0" :class="{ 'btn-error': chatting, 'btn-primary': !chatting }" :disabled="frozen" @click="chatting ? $emit('stopChat') : $emit('sendChat')">
          <Square v-if="chatting" class="h-3 w-3 fill-current" />
          <ArrowUp v-else class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDown, ArrowUp, Copy, FileText, Image as ImageIcon, Lock, LockOpen, Mic, Pause, Play, RotateCcw, Square, Undo2, X } from "lucide-vue-next";
import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";
import twemoji from "twemoji";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatTurn } from "../../../types/app";

const props = defineProps<{
  userAlias: string;
  personaName: string;
  userAvatarUrl: string;
  assistantAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
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
  turns: ChatTurn[];
  hasMoreTurns: boolean;
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
  (e: "loadMoreTurns"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "lockWorkspace"): void;
  (e: "unlockWorkspace"): void;
  (e: "openSkillList"): void;
}>();
const { t } = useI18n();

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

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
});

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function turnAssistantName(turn: ChatTurn): string {
  const id = String(turn.assistantAgentId || "").trim();
  if (id && props.personaNameMap[id]) return props.personaNameMap[id];
  return props.personaName || t("archives.roleAssistant");
}

function turnAssistantAvatarUrl(turn: ChatTurn): string {
  const id = String(turn.assistantAgentId || "").trim();
  if (id && props.personaAvatarUrlMap[id]) return props.personaAvatarUrlMap[id];
  return props.assistantAvatarUrl || "";
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

function resolvedTurnInlineReasoning(turn: ChatTurn): string {
  return splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline || "";
}

function toolSummaryText(lastToolName: string, count: number): string {
  const extraCount = Math.max(0, Number(count || 0) - 1);
  return extraCount > 0
    ? `调用 ${String(lastToolName || "-")} (+${extraCount})`
    : `调用 ${String(lastToolName || "-")}`;
}

async function copyAssistantTurn(turn: ChatTurn) {
  const copyText = splitThinkText(turn.assistantText).visible || turn.assistantText || "";
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
  const first = lines.length ? lines[0] : raw.trim();
  if (!first) return "";
  const chars = Array.from(first);
  if (chars.length <= 20) return first;
  return chars.slice(0, 20).join("") + "...";
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
  if (!props.chatting && scrollingUp && el.scrollTop <= 20 && props.hasMoreTurns && !loadingMore) {
    loadingMore = true;
    loadingMoreOldHeight = el.scrollHeight;
    emit("loadMoreTurns");
  }
}

function onWheel(event: WheelEvent) {
  const el = scrollContainer.value;
  if (!el) return;
  const pushingUpAtTop = event.deltaY < 0 && el.scrollTop <= 20;
  if (!props.chatting && pushingUpAtTop && props.hasMoreTurns && !loadingMore) {
    loadingMore = true;
    loadingMoreOldHeight = el.scrollHeight;
    emit("loadMoreTurns");
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
  () => props.turns.length,
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
  },
);

watch(
  () => props.hasMoreTurns,
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
    if (!autoFollowOutput.value) return;
    nextTick(() => scrollToBottom());
  },
);
</script>

<style scoped>
.assistant-markdown :deep(p) {
  margin: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.assistant-markdown :deep(ul),
.assistant-markdown :deep(ol) {
  margin: 0.2rem 0 0.4rem 1rem;
  padding: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.assistant-markdown :deep(li),
.assistant-markdown :deep(a) {
  overflow-wrap: anywhere;
  word-break: break-word;
}

.assistant-markdown :deep(code) {
  background: rgb(0 0 0 / 8%);
  border-radius: 0.25rem;
  padding: 0.1rem 0.25rem;
  font-size: 0.8em;
}

.assistant-markdown :deep(pre) {
  background: rgb(0 0 0 / 8%);
  border-radius: 0.4rem;
  padding: 0.45rem 0.55rem;
  overflow-x: auto;
  margin: 0.3rem 0 0.45rem;
}

.assistant-markdown :deep(pre code) {
  background: transparent;
  padding: 0;
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

:deep(.chat-input-no-focus:focus),
:deep(.chat-input-no-focus:focus-visible) {
  outline: none !important;
  box-shadow: none !important;
  border-color: transparent !important;
}

:deep(.chat-input-no-focus),
:deep(.chat-input-no-focus:hover),
:deep(.chat-input-no-focus:focus),
:deep(.chat-input-no-focus:focus-visible) {
  border-color: transparent !important;
}

</style>

