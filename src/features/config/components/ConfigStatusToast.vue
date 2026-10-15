<template>
  <Transition name="config-status-toast">
    <div
      v-if="visible && text"
      class="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex justify-center px-4"
    >
      <div
        class="alert pointer-events-auto flex w-fit max-w-full items-start gap-2 px-4 py-2 text-sm shadow-lg"
        :class="toneClass"
      >
        <span class="block min-w-0 whitespace-pre-wrap break-words leading-5">{{ text }}</span>
        <button
          v-if="sticky"
          type="button"
          class="btn btn-ghost btn-xs -mr-1 shrink-0"
          :title="t('common.close')"
          @click="hide"
        >
          <X class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { X } from "@lucide/vue";
import type { StatusTone } from "../../shell/composables/use-app-core";

const props = withDefaults(defineProps<{
  text: string;
  tone: StatusTone;
  /** 每次提示自增，保证内容相同的重复提示也能重新弹出 */
  nonce?: number;
  /** 常驻提示：不自动消失，需手动关闭 */
  sticky?: boolean;
}>(), {
  text: "",
  tone: "default",
  nonce: 0,
  sticky: false,
});

const { t } = useI18n();

const visible = ref(false);
let hideTimer = 0;

const toneClass = computed(() => {
  if (props.tone === "error") return "alert-error alert-soft";
  if (props.tone === "success") return "alert-success alert-soft";
  return "bg-base-200 text-base-content";
});

function hide() {
  visible.value = false;
  if (hideTimer) {
    window.clearTimeout(hideTimer);
    hideTimer = 0;
  }
}

watch(
  [() => props.text, () => props.nonce],
  ([next]) => {
    if (!next) return;
    visible.value = true;
    if (hideTimer) {
      window.clearTimeout(hideTimer);
      hideTimer = 0;
    }
    // 常驻提示不挂自动隐藏定时器，等用户点关闭
    if (props.sticky) return;
    hideTimer = window.setTimeout(() => {
      visible.value = false;
      hideTimer = 0;
    }, 4000);
  },
);

defineExpose({ hide });
</script>

<style scoped>
.config-status-toast-enter-active,
.config-status-toast-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.config-status-toast-enter-from,
.config-status-toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
