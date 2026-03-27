<template>
  <div class="modal-box w-[90vw] max-w-none h-[90vh] flex min-h-0 flex-col overflow-hidden p-4">
    <h3 class="font-semibold text-sm mb-2 shrink-0">{{ title }}</h3>
    <div v-if="loading" class="text-sm opacity-70">{{ loadingText }}</div>
    <div v-else class="flex flex-1 min-h-0 flex-col gap-2">
      <div v-if="mode === 'full'" class="text-[11px] opacity-70">
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
defineProps<{
  mode: "full" | "system";
  loading: boolean;
  title: string;
  loadingText: string;
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
}>();
</script>
