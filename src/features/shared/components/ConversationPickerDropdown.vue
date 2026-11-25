<!-- 会话选择器：一个下拉按钮，选项按「会话 #N」倒序排列（最近更新的排最前）。
     数据由宿主传入，组件本身不发请求；`modelValue` 为空串表示「全局」，即不绑定具体会话。 -->
<template>
  <div ref="rootRef" class="relative">
    <button
      type="button"
      class="select select-bordered flex w-full items-center justify-between gap-2 pr-3 text-left"
      :disabled="disabled"
      @click="toggleOpen"
    >
      <span class="min-w-0 flex-1 truncate" :class="selectedLabel ? '' : 'text-base-content/50'">
        {{ selectedLabel || placeholderText }}
      </span>
      <AppIcon
        name="chevron-down"
        class="h-4 w-4 shrink-0 opacity-70 transition-transform"
        :class="dropdownOpen ? 'rotate-180' : ''"
      />
    </button>

    <div
      v-if="dropdownOpen"
      class="absolute right-0 z-50 w-full min-w-72 max-w-[calc(100vw-2rem)] overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
      :class="dropdownDirection === 'up' ? 'bottom-full mb-2' : 'top-full mt-2'"
    >
      <div v-if="allowGlobal" class="border-b border-base-200">
        <button
          type="button"
          class="flex w-full items-center px-3 py-2 text-left text-xs transition-colors hover:bg-base-200"
          :class="!modelValue ? 'bg-base-200 font-medium' : ''"
          @click="selectConversation('')"
        >
          <span class="truncate">{{ t("chat.conversationPicker.global") }}</span>
        </button>
      </div>

      <OverlayScrollArea v-if="options.length" scroller-class="max-h-64">
        <button
          v-for="item in options"
          :key="item.id"
          type="button"
          class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-base-200"
          :class="modelValue === item.id ? 'bg-base-200 font-medium' : ''"
          @click="selectConversation(item.id)"
        >
          <span class="shrink-0 font-mono tabular-nums opacity-60">
            {{ t("chat.conversationPicker.label", { index: item.index }) }}
          </span>
          <span class="min-w-0 flex-1 truncate">{{ item.title }}</span>
        </button>
      </OverlayScrollArea>

      <div v-else class="px-3 py-3 text-xs opacity-50">
        {{ t("chat.conversationPicker.empty") }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "./AppIcon.vue";
import OverlayScrollArea from "./OverlayScrollArea.vue";
import { indexConversations } from "./conversation-picker";
import type { UnarchivedConversationSummary } from "../../../types/app";

const props = withDefaults(defineProps<{
  modelValue: string;
  conversations: UnarchivedConversationSummary[];
  /** 空串是否可以选：定时器的「全局」就是一个合法的空目标 */
  allowGlobal?: boolean;
  disabled?: boolean;
  placeholder?: string;
}>(), {
  allowGlobal: true,
  disabled: false,
  placeholder: "",
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t, locale } = useI18n();

const rootRef = ref<HTMLElement | null>(null);
const dropdownOpen = ref(false);
const dropdownDirection = ref<"up" | "down">("down");

/** 倒序：最近更新的排最前，#1 就是最新的那一条 */
const options = computed(() => indexConversations(props.conversations, {
  untitledLabel: t("chat.untitledConversation"),
  locale: locale.value,
}));

const selectedOption = computed(() => options.value.find((item) => item.id === props.modelValue) ?? null);

const selectedLabel = computed(() => {
  if (!props.modelValue) {
    return props.allowGlobal ? t("chat.conversationPicker.global") : "";
  }
  if (selectedOption.value) {
    return `${t("chat.conversationPicker.label", { index: selectedOption.value.index })} · ${selectedOption.value.title}`;
  }
  // 选中的会话不在当前列表里（已归档/被筛掉）时，缩成短号也不铺整条 UUID
  return props.modelValue.slice(0, 8);
});

const placeholderText = computed(() => props.placeholder || t("chat.conversationPicker.placeholder"));

function toggleOpen() {
  if (props.disabled) return;
  dropdownOpen.value = !dropdownOpen.value;
  if (dropdownOpen.value) {
    updateDirection();
  }
}

function updateDirection() {
  const root = rootRef.value;
  if (!root) return;
  const rect = root.getBoundingClientRect();
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  const availableAbove = rect.top;
  const availableBelow = viewportHeight - rect.bottom;
  dropdownDirection.value = availableAbove > availableBelow && availableBelow < 256 ? "up" : "down";
}

function selectConversation(value: string) {
  emit("update:modelValue", value);
  dropdownOpen.value = false;
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!dropdownOpen.value) return;
  const target = event.target as Node | null;
  if (rootRef.value && target && !rootRef.value.contains(target)) {
    dropdownOpen.value = false;
  }
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    dropdownOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  window.addEventListener("keydown", handleWindowKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>
