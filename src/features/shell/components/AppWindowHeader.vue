<template>
  <div
    class="navbar min-h-10 h-10 px-2 relative z-20 overflow-visible cursor-move select-none"
    :class="viewMode === 'chat' ? '' : 'bg-base-200 border-b border-base-300'"
    @mousedown.left.prevent="$emit('start-drag')"
  >
    <div v-if="viewMode === 'chat'" class="flex-none" @mousedown.stop>
      <button
        class="btn btn-ghost btn-xs"
        :title="openConfigTitle"
        @click.stop="$emit('open-config')"
      >
        <Settings class="h-3.5 w-3.5" />
      </button>
    </div>
    <div class="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex items-center px-2">
      <span class="font-semibold text-sm">{{ titleText }}</span>
      <template v-if="viewMode === 'chat'">
        <div class="tooltip tooltip-bottom" :data-tip="forceArchiveTip">
          <button
            class="btn btn-ghost btn-xs ml-2"
            :disabled="forcingArchive || chatting"
            @mousedown.stop
            @click.stop="$emit('force-archive')"
          >
            {{ chatUsagePercent }}%
          </button>
        </div>
      </template>
    </div>
    <div class="flex-none flex gap-1 ml-auto" @mousedown.stop>
      <template v-if="viewMode === 'chat'">
        <button
          class="btn btn-ghost btn-xs"
          :class="{ 'btn-active': alwaysOnTop }"
          :title="alwaysOnTop ? alwaysOnTopOffTitle : alwaysOnTopOnTitle"
          @click.stop="$emit('toggle-always-on-top')"
          :disabled="!windowReady"
        >
          <Pin class="h-3.5 w-3.5" />
        </button>
        <button
          class="btn btn-ghost btn-xs hover:bg-error/20"
          title="Close"
          @click.stop="$emit('close-window')"
          :disabled="!windowReady"
        >
          <X class="h-3.5 w-3.5" />
        </button>
      </template>
      <template v-else>
        <button
          class="btn btn-ghost btn-xs hover:bg-error/20"
          title="Close"
          @click.stop="$emit('close-window')"
          :disabled="!windowReady"
        >
          <X class="h-3.5 w-3.5" />
        </button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Pin, Settings, X } from "lucide-vue-next";

defineProps<{
  viewMode: "chat" | "archives" | "config";
  titleText: string;
  chatUsagePercent: number;
  forcingArchive: boolean;
  chatting: boolean;
  alwaysOnTop: boolean;
  windowReady: boolean;
  forceArchiveTip: string;
  alwaysOnTopOnTitle: string;
  alwaysOnTopOffTitle: string;
  openConfigTitle: string;
}>();

defineEmits<{
  (e: "start-drag"): void;
  (e: "force-archive"): void;
  (e: "toggle-always-on-top"): void;
  (e: "open-config"): void;
  (e: "close-window"): void;
}>();
</script>
