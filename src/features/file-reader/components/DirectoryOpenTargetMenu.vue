<template>
  <ul tabindex="0" class="menu p-1.5 text-sm" @click.stop>
    <li class="menu-title px-2 py-1 text-xs uppercase tracking-wide opacity-60">
      <span>{{ title }}</span>
    </li>
    <li v-for="item in targets" :key="item.kind">
      <button
        type="button"
        class="flex min-h-9 w-52 items-center justify-between gap-3 rounded-btn px-3 py-2 text-left"
        :class="selectedKind === item.kind ? 'active' : ''"
        :disabled="disabled"
        :title="item.label"
        @click="emit('select', item.kind)"
      >
        <span class="flex min-w-0 items-center gap-2">
          <img
            v-if="item.iconDataUrl"
            :src="item.iconDataUrl"
            alt=""
            class="h-4 w-4 shrink-0 object-contain"
          />
          <SquareTerminal v-else-if="item.type === 'shell'" class="h-4 w-4 shrink-0" />
          <Code2 v-else-if="item.type === 'vscode'" class="h-4 w-4 shrink-0" />
          <Folders v-else class="h-4 w-4 shrink-0" />
          <span class="min-w-0 truncate">{{ item.label }}</span>
        </span>
        <Check v-if="selectedKind === item.kind" class="h-4 w-4 shrink-0" />
      </button>
    </li>
  </ul>
</template>

<script setup lang="ts">
import { Check, Code2, Folders, SquareTerminal } from "@lucide/vue";
import type { DirectoryOpenTargetOption } from "../composables/use-directory-open-targets";

withDefaults(defineProps<{
  targets: DirectoryOpenTargetOption[];
  selectedKind?: string;
  disabled?: boolean;
  title?: string;
}>(), {
  selectedKind: "",
  disabled: false,
  title: "打开当前目录",
});

const emit = defineEmits<{
  (e: "select", kind: string): void;
}>();
</script>
