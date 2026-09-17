<template>
  <!-- 工作区：权限用文字，工作树模式在右上角挂一颗胶囊标签 -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition duration-200 ease-out"
    leave-to-class="opacity-0 translate-y-1"
  >
    <button
      v-if="showWorkspaceButton"
      type="button"
      :class="[SESSION_GHOST_PILL, 'relative max-w-[min(24rem,100%)]']"
      :disabled="workspaceButtonDisabled"
      :title="workspaceTitle"
      @click="emit('lockWorkspace')"
    >
      <span
        v-if="workspaceWorkMode === 'worktree'"
        class="absolute -right-1 -top-1.5 shrink-0 rounded-full border border-primary/30 bg-primary/15 px-1.5 py-0.5 text-caption font-medium leading-none text-primary backdrop-blur-md"
      >
        {{ t("chat.workspaceStatusModeWorktree") }}
      </span>
      <span class="shrink-0 text-base-content/60">{{ workspacePermissionText }}</span>
      <span class="h-4 w-px shrink-0 bg-base-300"></span>
      <span class="truncate max-w-[min(14rem,40vw)]">{{ workspaceButtonName || workspaceButtonLabel }}</span>
    </button>
  </Transition>

  <!-- 自动推送：状态标签，不是操作 -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition duration-200 ease-out"
    leave-to-class="opacity-0 translate-y-1"
  >
    <span
      v-if="autoPushActive"
      class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-info/30 bg-info/15 text-info backdrop-blur-md"
      :title="autoPushTitle"
    >
      <Send class="h-4 w-4" />
    </span>
  </Transition>

</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Send } from "@lucide/vue";
import type { ShellWorkMode } from "../../../types/app";
import { SESSION_GHOST_PILL } from "./session-float-styles";

defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  showWorkspaceButton?: boolean;
  workspaceButtonDisabled?: boolean;
  workspaceWorkMode?: ShellWorkMode;
  workspacePermissionKind?: "approval" | "full_access" | "autonomous";
  autoPushActive?: boolean;
}>();

const emit = defineEmits<{
  lockWorkspace: [];
}>();

const { t } = useI18n();

const workspaceTitle = computed(() => {
  if (!props.workspaceButtonName) return props.workspaceButtonLabel;
  return `${workspaceModeText.value} · ${workspacePermissionText.value} · ${props.workspaceButtonName}`;
});
const workspaceModeText = computed(() =>
  props.workspaceWorkMode === "worktree"
    ? t("chat.workspaceStatusModeWorktree")
    : t("chat.workspaceStatusModeDirectory"),
);
const workspacePermissionText = computed(() => {
  if (props.workspacePermissionKind === "autonomous") return t("chat.workspaceStatusPermissionAutonomous");
  if (props.workspacePermissionKind === "full_access") return t("chat.workspaceStatusPermissionFull");
  return t("chat.workspaceStatusPermissionApproval");
});
const autoPushTitle = computed(() => t("chat.autoPush.activeHint"));
</script>
