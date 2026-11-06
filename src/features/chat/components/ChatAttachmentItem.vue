<template>
  <img
    v-if="attachment.kind === 'image' && attachment.src"
    :src="attachment.src"
    :alt="attachment.label"
    class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
    @click.stop="handleActivate"
  />
  <component
    :is="interactive ? 'button' : 'span'"
    v-else
    :type="interactive ? 'button' : undefined"
    class="inline-flex min-h-9 max-w-64 items-center gap-2 rounded-xl border border-base-content/10 bg-base-100/35 px-2.5 py-1.5 text-left text-xs text-base-content/75 transition"
    :class="[
      interactive ? 'hover:bg-base-100/55' : '',
      selected ? 'border-primary/35 bg-primary/10 text-primary' : '',
    ]"
    :title="title || attachment.label"
    @click.stop="handleActivate"
  >
    <span class="inline-flex shrink-0 items-center justify-center">
      <slot name="leading">
        <span
          v-if="attachment.kind === 'audio'"
          class="inline-flex size-6 items-center justify-center rounded-full bg-primary/15 text-primary"
        >
          <AppIcon name="pause" v-if="playing" class="size-3.5" aria-hidden="true" />
          <AppIcon name="play" v-else class="size-3.5" aria-hidden="true" />
        </span>
        <AppIcon name="code" v-else-if="attachment.kind === 'context'" class="size-3.5 text-info" aria-hidden="true" />
        <AppIcon name="file-text" v-else class="size-3.5 text-base-content/55" aria-hidden="true" />
      </slot>
    </span>
    <span class="min-w-0 truncate">{{ attachment.label }}</span>
    <span v-if="attachment.detail" class="shrink-0 text-base-content/45">{{ attachment.detail }}</span>
    <slot name="suffix" />
  </component>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ChatAttachmentView } from "../utils/chat-attachment-display";

const props = withDefaults(defineProps<{
  attachment: ChatAttachmentView;
  interactive?: boolean;
  playing?: boolean;
  selected?: boolean;
  title?: string;
}>(), {
  interactive: false,
  playing: false,
  selected: false,
  title: "",
});

const emit = defineEmits<{
  (e: "activate"): void;
}>();

function handleActivate(): void {
  if (props.interactive || props.attachment.kind === "image") emit("activate");
}
</script>
