<template>
  <div class="flex flex-col h-full min-h-0 relative">
    <div ref="scrollContainer" class="flex-1 min-h-0 overflow-y-auto p-3 space-y-2" @scroll="onScroll">
      <!-- 加载更多提示 -->
      <div v-if="hasMoreTurns" class="text-center">
        <button class="btn btn-ghost btn-xs text-base-content/50" @click="$emit('loadMoreTurns')">{{ t("chat.loadMore") }}</button>
      </div>

      <!-- 历史对话 turns -->
      <template v-for="turn in turns" :key="turn.id">
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
            <div v-if="turn.userText" class="whitespace-pre-wrap">{{ turn.userText }}</div>
            <div v-if="turn.userImages.length > 0" class="mt-2 grid gap-1">
              <img
                v-for="(img, idx) in turn.userImages"
                :key="`${turn.id}-img-${idx}`"
                :src="`data:${img.mime};base64,${img.bytesBase64}`"
                loading="lazy"
                decoding="async"
                class="rounded max-h-28 object-contain bg-base-100/40"
              />
            </div>
            <div v-if="turn.userAudios.length > 0" class="mt-2 flex flex-col gap-1">
              <button
                v-for="(aud, idx) in turn.userAudios"
                :key="`${turn.id}-aud-${idx}`"
                class="btn btn-xs btn-ghost bg-base-100/70 w-fit"
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
            <div v-if="turn.assistantReasoningStandard" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title min-w-0">
                <span class="block truncate italic">{{ lastLinePreview(turn.assistantReasoningStandard) || "..." }}</span>
              </div>
              <div class="collapse-content">
                {{ turn.assistantReasoningStandard }}
              </div>
            </div>
          </div>
          <div v-if="turn.assistantText" class="chat-bubble max-w-[92%] bg-white text-black assistant-markdown">
            <div
              v-if="splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline"
              class="mb-1 whitespace-pre-wrap italic text-base-content/60"
            >
              {{ splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline }}
            </div>
            <div
              v-html="renderMarkdown(splitThinkText(turn.assistantText).visible)"
              @click="handleAssistantLinkClick"
            ></div>
          </div>
        </div>
      </template>

      <!-- 发送中的即时反馈 -->
      <template v-if="chatting">
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
            <div v-if="latestReasoningStandardText" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title min-w-0 flex items-center gap-1">
                <span class="block truncate italic">{{ lastLinePreview(latestReasoningStandardText) || "..." }}</span>
                <span class="loading loading-dots loading-xs opacity-60"></span>
              </div>
              <div class="collapse-content">
                {{ latestReasoningStandardText }}
              </div>
            </div>
          </div>
          <div class="chat-bubble max-w-[92%] bg-white text-black assistant-markdown">
            <div
              v-if="latestInlineReasoningText"
              class="mb-1 whitespace-pre-wrap italic text-base-content/60"
            >
              {{ latestInlineReasoningText }}
            </div>
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
          <ImageIcon class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ t("chat.image", { index: idx + 1 }) }}</span>
          <button class="btn btn-ghost btn-xs btn-square" :disabled="chatting || frozen" @click="$emit('removeClipboardImage', idx)">
            <X class="h-3 w-3" />
          </button>
        </div>
      </div>
      <div class="flex flex-row items-center gap-2">
        <button
          class="btn btn-xs btn-circle shrink-0"
          :class="recording ? 'btn-error' : 'btn-ghost bg-base-100'"
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
          class="flex-1 textarea textarea-xs resize-none overflow-y-hidden"
          rows="1"
          :disabled="chatting || frozen"
          :placeholder="chatInputPlaceholder"
          @input="scheduleResizeChatInput"
          @keydown.enter.exact.prevent="!chatting && !frozen && $emit('sendChat')"
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
import { ArrowDown, ArrowUp, Image as ImageIcon, Mic, Pause, Play, Square, X } from "lucide-vue-next";
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
  recordHotkey: string;
  chatting: boolean;
  frozen: boolean;
  turns: ChatTurn[];
  hasMoreTurns: boolean;
}>();

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "loadMoreTurns"): void;
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

function lastLinePreview(raw: string): string {
  if (!raw) return "";
  const lines = raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const last = lines.length ? lines[lines.length - 1] : raw.trim();
  return last || "";
}

function buildAudioDataUrl(audio: { mime: string; bytesBase64: string }): string {
  return `data:${audio.mime};base64,${audio.bytesBase64}`;
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
  if (followScrollRaf) cancelAnimationFrame(followScrollRaf);
  followScrollRaf = requestAnimationFrame(() => {
    evaluateFollowState(el);
    followScrollRaf = 0;
  });
  if (el.scrollTop <= 20 && props.hasMoreTurns && !loadingMore) {
    loadingMore = true;
    const oldHeight = el.scrollHeight;
    emit("loadMoreTurns");
    nextTick(() => {
      const newHeight = el.scrollHeight;
      el.scrollTop = newHeight - oldHeight;
      loadingMore = false;
    });
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
  () => {
    if (!autoFollowOutput.value) return;
    nextTick(() => scrollToBottom());
  },
);

watch(
  () => props.turns.length,
  (newLen, oldLen) => {
    if (newLen > oldLen && autoFollowOutput.value) {
      nextTick(() => scrollToBottom());
    }
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

</style>
