<template>
  <ConfigCard v-if="items.length > 0" :title="cardTitle">
    <template #actions>
      <span v-if="typeof elapsedMs === 'number'" class="text-xs opacity-70">{{ elapsedLabel }}: {{ elapsedMs }}ms</span>
      <button
        v-if="refreshable"
        type="button"
        class="btn btn-sm"
        :disabled="disabled"
        @click="emit('refresh')"
      >
        {{ refreshLabel }}
      </button>
    </template>
    <div class="divide-y divide-base-300">
      <div
        v-for="item in items"
        :key="item.id"
        class="py-2.5"
      >
        <div class="flex items-start gap-3">
          <input
            type="checkbox"
            class="toggle toggle-sm toggle-success mt-1 shrink-0"
            :checked="item.enabled"
            :disabled="disabled || item.toggleDisabled"
            @change="onToggle($event, item.id)"
          />
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <div v-if="item.statusClass" class="w-2.5 h-2.5 rounded-full shrink-0" :class="item.statusClass" :title="item.statusTitle || ''"></div>
              <div class="font-medium">{{ item.name }}</div>
              <span v-if="item.running" class="loading loading-spinner loading-sm"></span>
            </div>
            <div class="text-xs opacity-60">{{ item.description || noDescriptionText }}</div>
            <slot name="item-extra" :item="item" />
            <slot name="item-debug" :item="item" />
          </div>
          <div class="shrink-0">
            <slot name="item-actions" :item="item" />
          </div>
        </div>
      </div>
    </div>
  </ConfigCard>
</template>

<script setup lang="ts">
import { computed } from "vue";
import ConfigCard from "./ConfigCard.vue";

export type ToolListItem = {
  id: string;
  name: string;
  description?: string;
  enabled: boolean;
  toggleDisabled?: boolean;
  running?: boolean;
  statusClass?: string;
  statusTitle?: string;
};

const props = withDefaults(defineProps<{
  title: string;
  items: ToolListItem[];
  disabled?: boolean;
  refreshable?: boolean;
  refreshLabel?: string;
  showCount?: boolean;
  noDescriptionText?: string;
  elapsedLabel?: string;
  elapsedMs?: number;
}>(), {
  disabled: false,
  refreshable: false,
  refreshLabel: "Refresh",
  showCount: true,
  noDescriptionText: "-",
  elapsedLabel: "Elapsed",
  elapsedMs: undefined,
});

const emit = defineEmits<{
  (e: "toggleItem", payload: { id: string; enabled: boolean }): void;
  (e: "refresh"): void;
}>();

const cardTitle = computed(() =>
  props.showCount ? `${props.title}（${props.items.length}）` : props.title,
);

function onToggle(event: Event, id: string) {
  const target = event.target as HTMLInputElement | null;
  emit("toggleItem", {
    id,
    enabled: !!target?.checked,
  });
}
</script>
