<template>
  <button
    v-if="uniqueEntries.length > 0"
    ref="buttonRef"
    v-bind="attrs"
    type="button"
    class="btn btn-sm btn-ghost btn-circle shrink-0"
    :title="t('chat.toolbar.personaList')"
    @click="togglePopup"
  >
    <span class="text-base font-semibold leading-none">@</span>
  </button>
  <Teleport to="body">
    <div
      v-if="popupOpen"
      ref="popupRef"
      class="fixed z-[1200]"
      :style="popupStyle"
    >
      <div class="relative overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl">
        <OverlayScrollArea
          ref="popupAreaRef"
          scroller-class="max-h-[min(56vh,24rem)] min-w-56 max-w-[min(80vw,20rem)] overscroll-contain p-1"
        >
          <ul class="flex flex-col gap-1">
            <li
              v-for="entry in uniqueEntries"
              :key="entry.agentId"
            >
              <button
                type="button"
                class="flex min-h-0 w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors hover:bg-base-200/80"
                :disabled="props.busy"
                @click="handleEntryClick(entry)"
              >
                <div class="indicator shrink-0">
                  <span
                    v-if="entry.selected"
                    class="indicator-item indicator-top indicator-end inline-flex h-4 w-4 translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-primary text-caption font-bold text-primary-content"
                  >
                    @
                  </span>
                  <span
                    v-else-if="entry.hasBackgroundTask"
                    class="indicator-item indicator-bottom indicator-end inline-flex min-w-5 translate-x-1/4 translate-y-1/4 items-center justify-center rounded-full border border-base-300 bg-base-100 px-1 py-0.5 text-caption text-base-content shadow-sm"
                  >
                    <span class="loading loading-dots loading-xs"></span>
                  </span>
                  <div class="avatar">
                    <div class="w-7 rounded-full">
                      <img
                        v-if="entry.avatarUrl"
                        :src="entry.avatarUrl"
                        :alt="entry.agentName"
                        class="h-7 w-7 rounded-full object-cover"
                      />
                      <div
                        v-else
                        class="flex h-7 w-7 items-center justify-center rounded-full bg-neutral text-caption text-neutral-content"
                      >
                        {{ avatarInitial(entry.agentName) }}
                      </div>
                    </div>
                  </div>
                </div>
                <div class="min-w-0 flex-1 truncate text-sm leading-5">@{{ entry.agentName }}</div>
              </button>
            </li>
          </ul>
        </OverlayScrollArea>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useAttrs } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatMentionEntry, ChatMentionTarget } from "../../../types/app";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";

defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  mentionEntries?: ChatMentionEntry[];
  selectedMentions?: ChatMentionTarget[];
  busy?: boolean;
}>();

const emit = defineEmits<{
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", value: { agentId: string }): void;
}>();

const attrs = useAttrs();
const { t } = useI18n();

const POPUP_OFFSET = 8;
const POPUP_VIEWPORT_PADDING = 8;

const buttonRef = ref<HTMLButtonElement | null>(null);
const popupOpen = ref(false);
const popupRef = ref<HTMLElement | null>(null);
const popupAreaRef = ref<InstanceType<typeof OverlayScrollArea> | null>(null);
const popupStyle = ref<Record<string, string>>({
  left: "0px",
  top: "0px",
});

/** 同一人格只保留一条，并回填当前选中态 */
const uniqueEntries = computed<ChatMentionEntry[]>(() => {
  const selectedAgentIds = new Set(
    (props.selectedMentions || []).map((item) => String(item.agentId || "").trim()),
  );
  const seen = new Map<string, ChatMentionEntry>();
  for (const entry of props.mentionEntries || []) {
    if (!entry.mentionable) continue;
    const agentId = String(entry.agentId || "").trim();
    if (!agentId || seen.has(agentId)) continue;
    seen.set(agentId, { ...entry, selected: selectedAgentIds.has(agentId) });
  }
  return Array.from(seen.values());
});

function clampPopupPosition(anchorRect: DOMRect, panelEl: HTMLElement | null, options?: {
  preferredWidth?: number;
  alignRight?: boolean;
}) {
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  const measuredWidth = Math.round(panelEl?.offsetWidth || options?.preferredWidth || 0);
  const measuredHeight = Math.round(panelEl?.offsetHeight || 0);
  const spaceAbove = Math.max(0, anchorRect.top - POPUP_VIEWPORT_PADDING - POPUP_OFFSET);
  const spaceBelow = Math.max(0, viewportHeight - anchorRect.bottom - POPUP_VIEWPORT_PADDING - POPUP_OFFSET);
  const openUpward = spaceAbove >= measuredHeight || spaceAbove > spaceBelow;
  const maxLeft = Math.max(
    POPUP_VIEWPORT_PADDING,
    viewportWidth - measuredWidth - POPUP_VIEWPORT_PADDING,
  );
  const preferredLeft = options?.alignRight
    ? Math.round(anchorRect.right - measuredWidth)
    : Math.round(anchorRect.left);
  const left = Math.min(Math.max(POPUP_VIEWPORT_PADDING, preferredLeft), maxLeft);
  const top = openUpward
    ? Math.max(POPUP_VIEWPORT_PADDING, Math.round(anchorRect.top) - measuredHeight - POPUP_OFFSET)
    : Math.min(
      Math.round(anchorRect.bottom) + POPUP_OFFSET,
      Math.max(POPUP_VIEWPORT_PADDING, viewportHeight - measuredHeight - POPUP_VIEWPORT_PADDING),
    );
  return {
    left: `${left}px`,
    top: `${top}px`,
  };
}

async function updatePopupPlacement() {
  const rect = buttonRef.value?.getBoundingClientRect();
  if (!rect) return;
  await nextTick();
  popupStyle.value = clampPopupPosition(rect, popupRef.value, {
    preferredWidth: 320,
    alignRight: true,
  });
  popupAreaRef.value?.updateThumb();
}

function closePopup() {
  popupOpen.value = false;
}

function togglePopup() {
  if (props.busy) return;
  popupOpen.value = !popupOpen.value;
  if (popupOpen.value) {
    void updatePopupPlacement();
  }
}

function handleEntryClick(entry: ChatMentionEntry) {
  if (props.busy) return;
  const agentId = String(entry.agentId || "").trim();
  if (!agentId) return;
  if (entry.selected) {
    emit("removeMention", { agentId });
  } else {
    const agentName = String(entry.agentName || "").trim();
    const avatarUrl = String(entry.avatarUrl || "").trim();
    emit("addMention", { agentId, agentName, ...(avatarUrl ? { avatarUrl } : {}) });
  }
  closePopup();
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

/** 点击按钮与浮层之外的区域时收起 */
function handleGlobalPointerDown(event: PointerEvent) {
  if (!popupOpen.value) return;
  if (!(event.target instanceof Node)) return;
  if (buttonRef.value?.contains(event.target)) return;
  if (popupRef.value?.contains(event.target)) return;
  closePopup();
}

function handleViewportChange() {
  if (popupOpen.value) {
    void updatePopupPlacement();
  }
}

onMounted(() => {
  window.addEventListener("resize", handleViewportChange);
  window.addEventListener("scroll", handleViewportChange, true);
  window.addEventListener("pointerdown", handleGlobalPointerDown, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", handleViewportChange);
  window.removeEventListener("scroll", handleViewportChange, true);
  window.removeEventListener("pointerdown", handleGlobalPointerDown, true);
});
</script>
