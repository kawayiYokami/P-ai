<template>
  <div class="modal-box max-w-2xl">
    <h3 class="font-semibold text-sm mb-2">{{ title }}</h3>
    <div v-if="loading" class="text-xs opacity-70">{{ loadingText }}</div>
    <div v-else class="grid gap-2">
      <div v-if="mode === 'full'" class="text-[11px] opacity-70">
        {{ latestInputLengthText }}: {{ latestUserText.length }} |
        {{ imagesText }}: {{ latestImages }} |
        {{ audiosText }}: {{ latestAudios }}
      </div>
      <textarea
        class="textarea textarea-bordered textarea-sm w-full h-80 font-mono text-xs"
        readonly
        :value="text"
      ></textarea>
    </div>
    <div class="modal-action"><button class="btn btn-sm" @click="$emit('close')">{{ closeText }}</button></div>
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
