<template>
  <div class="modal-box w-[90vw] max-w-none h-[90vh] flex min-h-0 flex-col overflow-hidden p-4">
    <h3 class="font-semibold text-sm mb-2 shrink-0">{{ title }}</h3>
    <div v-if="mode !== 'system'" class="mb-2 flex shrink-0 gap-2">
      <button class="btn btn-sm" :class="mode === 'chat' ? 'btn-primary' : 'bg-base-200 border-base-300'" @click="$emit('selectMode', 'chat')">
        {{ chatText }}
      </button>
      <button class="btn btn-sm" :class="mode === 'compaction' ? 'btn-primary' : 'bg-base-200 border-base-300'" @click="$emit('selectMode', 'compaction')">
        {{ compactionText }}
      </button>
      <button class="btn btn-sm" :class="mode === 'archive' ? 'btn-primary' : 'bg-base-200 border-base-300'" @click="$emit('selectMode', 'archive')">
        {{ archiveText }}
      </button>
    </div>
    <div v-if="loading" class="text-sm opacity-70">{{ loadingText }}</div>
    <div v-else-if="!mode" class="flex flex-1 items-center justify-center text-sm opacity-70 whitespace-pre-line">
      {{ emptyHint }}
    </div>
    <div v-else class="flex flex-1 min-h-0 flex-col gap-2">
      <div v-if="mode !== 'system'" class="text-[11px] opacity-70">
        {{ latestInputLengthText }}: {{ latestUserText.length }} |
        {{ imagesText }}: {{ latestImages }} |
        {{ audiosText }}: {{ latestAudios }}
      </div>
      <textarea
        class="textarea textarea-bordered textarea-sm w-full flex-1 min-h-0 resize-none font-mono text-sm leading-6"
        readonly
        :value="text"
      ></textarea>
    </div>
    <div class="modal-action shrink-0">
      <button class="btn btn-sm" @click="$emit('close')">{{ closeText }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { RequestPreviewMode } from "../../composables/use-prompt-preview";

defineProps<{
  mode: RequestPreviewMode | "system" | null;
  loading: boolean;
  title: string;
  loadingText: string;
  emptyHint: string;
  chatText: string;
  compactionText: string;
  archiveText: string;
  latestInputLengthText: string;
  imagesText: string;
  audiosText: string;
  closeText: string;
  latestUserText: string;
  latestImages: number;
  latestAudios: number;
  text: string;
}>();

defineEmits<{
  close: [];
  selectMode: [mode: RequestPreviewMode];
}>();
</script>
