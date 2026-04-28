<template>
  <div class="rounded-box border border-base-300 bg-base-100/70 px-2 py-1.5 flex items-center justify-between gap-2 text-[11px]">
    <div class="flex min-w-0 items-center gap-1.5">
      <div
        v-if="!hideMenuButton"
        class="dropdown dropdown-start"
        :class="menuPlacement === 'top' ? 'dropdown-top' : 'dropdown-bottom'"
      >
        <button
          ref="menuButtonRef"
          type="button"
          tabindex="0"
          class="btn btn-sm btn-ghost btn-circle shrink-0"
          :title="t('chat.conversationMenu.title')"
          @mousedown="updateMenuPlacement"
        >
          <Grip class="h-4 w-4" />
        </button>
        <ul
          tabindex="0"
          class="dropdown-content menu z-50 w-64 rounded-box border border-base-300 bg-base-100 p-3 text-sm shadow-xl"
          :class="menuPlacement === 'top' ? 'mb-3' : 'mt-3'"
        >
          <li v-if="!busy">
            <button type="button" class="flex min-h-10 items-center justify-start gap-3 px-4 py-2 text-left" :disabled="busy" @click="emit('openBranchSelection')">
              <GitBranch class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.conversationMenu.branchFromSelection") }}</span>
            </button>
          </li>
          <li>
            <button type="button" class="flex min-h-10 items-center justify-start gap-3 px-4 py-2 text-left" :disabled="frozen || supervisionDisabled" @click="emit('openSupervisionTask')">
              <Timer class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.conversationMenu.startSupervision") }}</span>
            </button>
          </li>
          <li v-if="!busy && !workspaceButtonDisabled">
            <button type="button" class="flex min-h-10 items-center justify-start gap-3 px-4 py-2 text-left" :disabled="busy || workspaceButtonDisabled" @click="emit('lockWorkspace')">
              <Folder class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.conversationMenu.setWorkspace") }}</span>
            </button>
          </li>
          <li v-if="showDetachButton && !busy && !detachDisabled">
            <button
              type="button"
              class="flex min-h-10 items-center justify-start gap-3 px-4 py-2 text-left"
              :disabled="busy || detachDisabled"
              @mousedown="handleDetachConversationMouseDown"
              @click="handleDetachConversationClick"
            >
              <ExternalLink class="h-4 w-4 shrink-0" />
              <span class="leading-5">{{ t("chat.conversationMenu.openDetachedWindow") }}</span>
            </button>
          </li>
        </ul>
      </div>
      <button
        v-if="!hideWorkspaceButton"
        class="btn btn-sm btn-ghost gap-1.5"
        :disabled="busy || workspaceButtonDisabled"
        @click="emit('lockWorkspace')"
      >
        <SquareTerminal class="h-3.5 w-3.5" />
        {{ workspaceButtonName || workspaceButtonLabel }}
      </button>
    </div>
    <div class="flex min-w-0 items-center justify-end gap-1.5">
      <button
        type="button"
        class="btn btn-sm btn-circle overflow-visible p-0 shrink-0 border relative"
        :class="reviewPanelOpen ? 'border-primary/60 bg-primary/10 text-primary hover:border-primary hover:bg-primary/15' : 'border-base-300/70 bg-base-100/70 hover:border-base-300 hover:bg-base-200'"
        :disabled="!reviewButtonEnabled"
        :title="reviewButtonLabel"
        @click="emit('toggleToolReview')"
      >
        <Glasses class="h-4 w-4" />
        <span
          v-if="normalizedReviewButtonCount > 0"
          class="badge badge-primary badge-xs absolute -right-1.5 -top-1.5 min-w-4 px-1 text-[10px]"
        >
          {{ normalizedReviewButtonCount > 99 ? "99+" : normalizedReviewButtonCount }}
        </span>
      </button>
      <button
        v-for="persona in personaPresenceChips"
        :key="persona.id"
        type="button"
        class="btn btn-ghost btn-sm btn-circle overflow-visible p-0 shrink-0 border relative"
        :class="personaChipClass(persona)"
        :title="`部门：${persona.departmentName}\n人格：${persona.name}`"
        :disabled="chatting || frozen || !mentionableAgentIds.includes(persona.id)"
        @click="emit('mentionPersona', persona.id)"
      >
        <div class="indicator">
          <span
            v-if="selectedMentionAgentIds.includes(persona.id)"
            class="indicator-item indicator-top indicator-end inline-flex h-4 w-4 translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-primary text-[9px] font-bold text-primary-content"
          >
            @
          </span>
          <span
            v-if="props.selectedMentionAgentIds.length > 0 && persona.isFrontSpeaking"
            class="indicator-item indicator-top indicator-start inline-flex h-4 w-4 -translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-base-300 text-[9px] font-bold text-base-content"
          >
            禁
          </span>
          <div class="avatar">
            <div class="w-7 rounded-full">
              <img
                v-if="persona.avatarUrl"
                :src="persona.avatarUrl"
                :alt="persona.name"
                class="w-7 h-7 rounded-full object-cover"
                :class="frontSpeakingMuted(persona) ? 'grayscale opacity-75' : ''"
              />
              <div
                v-else
                class="w-7 h-7 rounded-full flex items-center justify-center text-[10px]"
                :class="frontSpeakingMuted(persona)
                  ? 'bg-base-300 text-base-content/70'
                  : 'bg-neutral text-neutral-content'"
              >
                {{ avatarInitial(persona.name) }}
              </div>
            </div>
          </div>
        </div>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ExternalLink, Folder, GitBranch, Glasses, Grip, SquareTerminal, Timer } from "lucide-vue-next";
import type { ChatPersonaPresenceChip } from "../../../types/app";

const props = defineProps<{
  chatting: boolean;
  frozen: boolean;
  conversationBusy?: boolean;
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  workspaceButtonDisabled?: boolean;
  personaPresenceChips: ChatPersonaPresenceChip[];
  mentionableAgentIds: string[];
  selectedMentionAgentIds: string[];
  supervisionActive: boolean;
  supervisionLabel: string;
  supervisionActiveLabel: string;
  supervisionTitle: string;
  supervisionDisabled?: boolean;
  reviewButtonLabel: string;
  reviewButtonCount?: number;
  reviewPanelOpen: boolean;
  reviewButtonEnabled: boolean;
  hideMenuButton?: boolean;
  hideWorkspaceButton?: boolean;
  showDetachButton?: boolean;
  detachDisabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "lockWorkspace"): void;
  (e: "openSupervisionTask"): void;
  (e: "openBranchSelection"): void;
  (e: "detachConversation"): void;
  (e: "toggleToolReview"): void;
  (e: "mentionPersona", agentId: string): void;
}>();

const { t } = useI18n();
const busy = computed(() => props.chatting || props.frozen || !!props.conversationBusy);
const normalizedReviewButtonCount = computed(() =>
  Math.max(0, Math.round(Number(props.reviewButtonCount || 0))),
);
const menuButtonRef = ref<HTMLButtonElement | null>(null);
const menuPlacement = ref<"top" | "bottom">("top");

function updateMenuPlacement() {
  const rect = menuButtonRef.value?.getBoundingClientRect();
  if (!rect) return;
  menuPlacement.value = rect.top >= window.innerHeight / 2 ? "top" : "bottom";
}

function handleDetachConversationMouseDown() {
  updateMenuPlacement();
  console.info("[独立聊天窗口][前端入口] 工具栏按钮 mousedown", {
    chatting: props.chatting,
    frozen: props.frozen,
    detachDisabled: !!props.detachDisabled,
  });
}

function handleDetachConversationClick() {
  console.info("[独立聊天窗口][前端入口] 工具栏按钮已点击，准备向上派发 detachConversation", {
    chatting: props.chatting,
    frozen: props.frozen,
    detachDisabled: !!props.detachDisabled,
  });
  emit("detachConversation");
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function personaChipClass(persona: ChatPersonaPresenceChip): string {
  const selected = props.selectedMentionAgentIds.includes(persona.id);
  const muted = frontSpeakingMuted(persona);
  if (selected) {
    return "border-primary/60 bg-primary/10 hover:border-primary hover:bg-primary/15";
  }
  if (muted) {
    return "border-base-300/70 bg-base-200/70 hover:border-base-300 hover:bg-base-200";
  }
  return "border-base-300/70 bg-base-100/70 hover:border-base-300 hover:bg-base-200";
}

function frontSpeakingMuted(persona: ChatPersonaPresenceChip): boolean {
  return props.selectedMentionAgentIds.length > 0 && persona.isFrontSpeaking;
}

onMounted(() => {
  updateMenuPlacement();
  window.addEventListener("resize", updateMenuPlacement);
  window.addEventListener("scroll", updateMenuPlacement, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", updateMenuPlacement);
  window.removeEventListener("scroll", updateMenuPlacement, true);
});
</script>
