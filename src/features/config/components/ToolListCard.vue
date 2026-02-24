<template>
  <div v-if="items.length > 0" class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
    <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
      <div class="font-medium">{{ title }}<span v-if="showCount">（{{ items.length }}）</span></div>
      <div class="ml-auto flex items-center gap-2">
        <span v-if="typeof elapsedMs === 'number'" class="text-[11px] opacity-70">{{ elapsedLabel }}: {{ elapsedMs }}ms</span>
        <button
          v-if="refreshable"
          type="button"
          class="btn btn-xs"
          :disabled="disabled"
          @click="emit('refresh')"
        >
          {{ refreshLabel }}
        </button>
      </div>
    </div>
    <div class="divide-y divide-base-300/60">
      <div
        v-for="item in items"
        :key="item.id"
        class="px-3 py-2"
      >
        <div class="flex items-start gap-3">
          <input
            type="checkbox"
            class="toggle toggle-xs toggle-success mt-1 shrink-0"
            :checked="item.enabled"
            :disabled="disabled || item.toggleDisabled"
            @change="onToggle($event, item.id)"
          />
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <div v-if="item.statusClass" class="w-2.5 h-2.5 rounded-full shrink-0" :class="item.statusClass" :title="item.statusTitle || ''"></div>
              <div class="font-medium">{{ item.name }}</div>
              <span v-if="item.running" class="loading loading-spinner loading-xs"></span>
            </div>
            <div class="text-[11px] opacity-60">{{ item.description || noDescriptionText }}</div>
            <slot name="item-extra" :item="item" />
            <slot name="item-debug" :item="item" />
          </div>
          <div class="shrink-0">
            <slot name="item-actions" :item="item" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
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

function onToggle(event: Event, id: string) {
  const target = event.target as HTMLInputElement | null;
  emit("toggleItem", {
    id,
    enabled: !!target?.checked,
  });
}
</script>
