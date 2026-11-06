<template>
  <div class="flex items-center gap-2">
    <button
      class="btn btn-sm btn-square btn-ghost shrink-0"
      type="button"
      :title="props.addTitle"
      @click="emit('add')"
    >
      <AppIcon name="plus" class="h-4 w-4" />
    </button>
    <button
      class="btn btn-sm btn-square btn-error shrink-0"
      type="button"
      :title="props.removeTitle"
      :disabled="props.removeDisabled"
      @click="emit('remove')"
    >
      <AppIcon name="trash" class="h-4 w-4" />
    </button>
    <select
      :value="props.modelValue"
      class="select select-bordered select-md min-w-0 flex-1"
      :disabled="props.selectDisabled"
      @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
    >
      <option v-if="props.providers.length === 0" value="">{{ props.emptyLabel }}</option>
      <option v-for="provider in props.providers" :key="provider.id" :value="provider.id">
        {{ provider.label }}
      </option>
    </select>
    <button
      class="btn btn-sm btn-square btn-ghost shrink-0"
      type="button"
      :title="props.restoreTitle"
      :disabled="props.restoreDisabled"
      @click="emit('restore')"
    >
      <AppIcon name="rotate-ccw" class="h-4 w-4" />
    </button>
    <button
      class="btn btn-sm btn-square"
      :class="props.dirty ? 'btn-primary' : 'btn-ghost'"
      type="button"
      :title="props.saveTitle"
      :disabled="props.saveDisabled"
      @click="emit('save')"
    >
      <AppIcon name="save" v-if="!props.saving" class="h-4 w-4" />
      <span v-else class="loading loading-spinner loading-sm"></span>
    </button>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";

export type ProviderToolbarOption = {
  id: string;
  label: string;
};

const props = withDefaults(defineProps<{
  providers: ProviderToolbarOption[];
  modelValue: string;
  emptyLabel: string;
  addTitle: string;
  removeTitle: string;
  restoreTitle: string;
  saveTitle: string;
  dirty?: boolean;
  saving?: boolean;
  removeDisabled?: boolean;
  restoreDisabled?: boolean;
  saveDisabled?: boolean;
  selectDisabled?: boolean;
}>(), {
  dirty: false,
  saving: false,
  removeDisabled: false,
  restoreDisabled: false,
  saveDisabled: false,
  selectDisabled: false,
});

const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
  (e: "add"): void;
  (e: "remove"): void;
  (e: "restore"): void;
  (e: "save"): void;
}>();
</script>
