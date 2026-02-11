<template>
  <div class="flex flex-col h-full">
    <div ref="scrollContainer" class="flex-1 overflow-y-auto p-3 space-y-2" @scroll="onScroll">
      <!-- 加载更多提示 -->
      <div v-if="hasMoreTurns" class="text-center">
        <button class="btn btn-ghost btn-xs text-base-content/50" @click="$emit('loadMoreTurns')">加载更多...</button>
      </div>

      <!-- 历史对话 turns -->
      <template v-for="turn in turns" :key="turn.id">
        <div class="chat chat-end">
          <div class="chat-header text-[11px] opacity-70 mb-1">{{ userAlias }}</div>
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
                <span>语音{{ idx + 1 }}</span>
              </button>
            </div>
          </div>
        </div>
        <div v-if="turn.assistantText || turn.assistantReasoningStandard || turn.assistantReasoningInline" class="chat chat-start">
          <div class="chat-header text-[11px] opacity-70 mb-1 flex items-center gap-1">
            <span>{{ personaName || "助理" }}</span>
            <div v-if="turn.assistantReasoningStandard" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title">
                {{ lastLinePreview(turn.assistantReasoningStandard) || "..." }}
              </div>
              <div class="collapse-content">
                {{ turn.assistantReasoningStandard }}
              </div>
            </div>
            <div v-if="(splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline)" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title">
                {{ lastLinePreview(splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline) || "..." }}
              </div>
              <div class="collapse-content">
                {{ splitThinkText(turn.assistantText).inline || turn.assistantReasoningInline }}
              </div>
            </div>
          </div>
          <div v-if="turn.assistantText" class="chat-bubble max-w-[92%] bg-white text-black assistant-markdown">
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
          <div class="chat-header text-[11px] opacity-70 mb-1 flex items-center gap-1">
            <span>{{ personaName || "助理" }}</span>
            <div v-if="latestReasoningStandardText" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title flex items-center gap-1">
                <span>{{ lastLinePreview(latestReasoningStandardText) || "..." }}</span>
                <span class="loading loading-dots loading-xs opacity-60"></span>
              </div>
              <div class="collapse-content">
                {{ latestReasoningStandardText }}
              </div>
            </div>
            <div v-if="latestInlineReasoningText" class="collapse collapse-arrow">
              <input type="checkbox" />
              <div class="collapse-title flex items-center gap-1">
                <span>{{ lastLinePreview(latestInlineReasoningText) || "..." }}</span>
                <span class="loading loading-dots loading-xs opacity-60"></span>
              </div>
              <div class="collapse-content">
                {{ latestInlineReasoningText }}
              </div>
            </div>
          </div>
          <div class="chat-bubble max-w-[92%] bg-white text-black assistant-markdown">
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

    <div class="shrink-0 border-t border-base-300 bg-base-100 p-2">
      <div
        v-if="chatErrorText"
        class="alert alert-error mb-2 py-2 px-3 text-xs whitespace-pre-wrap break-all max-h-28 overflow-auto"
      >
        <span>{{ chatErrorText }}</span>
      </div>
      <div v-if="clipboardImages.length > 0" class="flex flex-wrap gap-1 mb-2">
        <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-outline gap-1 py-3">
          <ImageIcon class="h-3.5 w-3.5" />
          <span class="text-[11px]">图片{{ idx + 1 }}</span>
          <button class="btn btn-ghost btn-xs btn-square" @click="$emit('removeClipboardImage', idx)">
            <X class="h-3 w-3" />
          </button>
        </div>
      </div>
      <div class="flex flex-row items-center gap-2">
        <button
          class="btn btn-sm btn-circle shrink-0"
          :class="recording ? 'btn-error' : 'btn-ghost bg-base-100'"
          :disabled="!canRecord || chatting"
          :title="recording ? `录音中 ${Math.max(1, Math.round(recordingMs / 1000))}s` : `按住${recordHotkey}或按钮录音`"
          @mousedown.prevent="$emit('startRecording')"
          @mouseup.prevent="$emit('stopRecording')"
          @mouseleave.prevent="recording && $emit('stopRecording')"
          @touchstart.prevent="$emit('startRecording')"
          @touchend.prevent="$emit('stopRecording')"
        >
          <Mic class="h-3.5 w-3.5" />
        </button>
        <textarea
          v-model="localChatInput"
          class="flex-1 textarea textarea-sm resize-none border-none bg-transparent focus:outline-none"
          :rows="Math.max(1, Math.min(10, Math.round(((localChatInput.match(/\n/g) || []).length + 1) * 1.5)))"
          :placeholder="chatInputPlaceholder"
          @keydown.enter.exact.prevent="!chatting && $emit('sendChat')"
        ></textarea>
        <button class="btn btn-sm btn-circle shrink-0" :class="{ 'btn-error': chatting, 'btn-primary': !chatting }" @click="chatting ? $emit('stopChat') : $emit('sendChat')">
          <Square v-if="chatting" class="h-3 w-3 fill-current" />
          <ArrowUp v-else class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onBeforeUnmount, onMounted, watch } from "vue";
import { ArrowUp, Image as ImageIcon, Mic, Pause, Play, Square, X } from "lucide-vue-next";
import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";
import { invoke } from "@tauri-apps/api/core";
import type { ChatTurn } from "../types/app";

const props = defineProps<{
  userAlias: string;
  personaName: string;
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

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});

const scrollContainer = ref<HTMLElement | null>(null);
const playingAudioId = ref("");
let activeAudio: HTMLAudioElement | null = null;

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
});

function splitThinkText(raw: string): { visible: string; inline: string } {
  const input = raw || "";
  const regex = /<think>([\s\S]*?)<\/think>/gi;
  const blocks: string[] = [];
  let m: RegExpExecArray | null;
  while ((m = regex.exec(input)) !== null) {
    const text = (m[1] || "").trim();
    if (text) blocks.push(text);
  }
  const visible = input.replace(regex, "").trim();
  return {
    visible,
    inline: blocks.join("\n\n"),
  };
}

function renderMarkdown(text: string): string {
  const raw = md.render(text || "");
  return DOMPurify.sanitize(raw);
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
  if (!last) return "";
  return last.length > 42 ? `${last.slice(0, 42)}...` : last;
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

function scrollToBottom() {
  const el = scrollContainer.value;
  if (el) el.scrollTop = el.scrollHeight;
}

let loadingMore = false;

function onScroll() {
  const el = scrollContainer.value;
  if (!el) return;
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
    await invoke("open_external_url", { url: href });
  } catch {
    // ignore
  }
}

onMounted(() => {
  nextTick(() => scrollToBottom());
});

onBeforeUnmount(() => {
  stopAudioPlayback();
});

watch(
  () => props.chatting,
  () => nextTick(() => scrollToBottom()),
);

watch(
  () => props.turns.length,
  (newLen, oldLen) => {
    if (newLen > oldLen) {
      nextTick(() => scrollToBottom());
    }
  },
);
</script>

<style scoped>
.assistant-markdown :deep(p) {
  margin: 0 0 0.4rem;
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

</style>
