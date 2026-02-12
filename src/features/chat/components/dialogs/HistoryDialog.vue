<template>
  <div class="modal-box max-w-xl">
    <h3 class="font-semibold text-sm mb-2">{{ title }}</h3>
    <div class="max-h-96 overflow-auto space-y-2">
      <div v-for="m in messages" :key="m.id" class="text-xs border border-base-300 rounded p-2">
        <div class="font-semibold uppercase text-[11px]">{{ m.role }}</div>
        <div v-if="messageText(m)" class="whitespace-pre-wrap">{{ messageText(m) }}</div>
        <div v-if="extractImages(m).length > 0" class="mt-2 grid gap-1">
          <img
            v-for="(img, idx) in extractImages(m)"
            :key="`${img.mime}-${idx}`"
            :src="`data:${img.mime};base64,${img.bytesBase64}`"
            loading="lazy"
            decoding="async"
            class="rounded max-h-32 object-contain bg-base-100/40"
          />
        </div>
      </div>
    </div>
    <div class="modal-action"><button class="btn btn-sm" @click="$emit('close')">{{ closeText }}</button></div>
  </div>
</template>

<script setup lang="ts">
import type { ChatMessage } from "../../types/app";

defineProps<{
  title: string;
  closeText: string;
  messages: ChatMessage[];
  messageText: (msg: ChatMessage) => string;
  extractImages: (msg?: ChatMessage) => Array<{ mime: string; bytesBase64: string }>;
}>();

defineEmits<{
  close: [];
}>();
</script>
