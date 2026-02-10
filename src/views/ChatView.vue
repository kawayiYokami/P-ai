<template>
  <div class="flex flex-col h-full">
    <div class="flex-1 overflow-y-auto p-3 space-y-2">
      <div class="chat chat-end">
      <div class="chat-header text-[11px] opacity-70 mb-1">{{ userAlias }}</div>
        <div class="chat-bubble max-w-[92%]">
          <div v-if="latestUserText" class="whitespace-pre-wrap">{{ latestUserText }}</div>
          <div v-if="latestUserImages.length > 0" class="mt-2 grid gap-1">
            <img
              v-for="(img, idx) in latestUserImages"
              :key="`${img.mime}-${idx}`"
              :src="`data:${img.mime};base64,${img.bytesBase64}`"
              class="rounded max-h-28 object-contain bg-base-100/40"
            />
          </div>
        </div>
      </div>
      <div class="chat chat-start">
        <div class="chat-header text-[11px] opacity-70 mb-1">{{ agentName || "助理" }}</div>
        <div class="chat-bubble max-w-[92%] bg-white text-black assistant-markdown">
          <span v-if="chatting" class="loading loading-dots loading-sm"></span>
          <div
            v-else-if="latestAssistantText"
            v-html="renderedAssistantHtml"
            @click="handleAssistantLinkClick"
          ></div>
          <template v-else>...</template>
        </div>
      </div>
    </div>

    <div class="shrink-0 border-t border-base-300 bg-base-100 p-2">
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
import { computed } from "vue";
import { ArrowUp, Image as ImageIcon, Square, X } from "lucide-vue-next";
import MarkdownIt from "markdown-it";
import DOMPurify from "dompurify";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{
  userAlias: string;
  agentName: string;
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  chatInput: string;
  chatInputPlaceholder: string;
  chatting: boolean;
}>();

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
}>();

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
});

const renderedAssistantHtml = computed(() => {
  const raw = md.render(props.latestAssistantText || "");
  return DOMPurify.sanitize(raw);
});

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
