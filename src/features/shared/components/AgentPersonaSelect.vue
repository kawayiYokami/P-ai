<template>
  <div class="flex flex-col gap-2">
    <div ref="rootRef" class="relative">
      <button
        type="button"
        class="select select-bordered flex w-full items-center justify-between gap-2 pr-3 text-left"
        :disabled="disabled || normalizedOptions.length === 0"
        @click="toggleOpen"
      >
        <div class="flex min-w-0 flex-1 items-center gap-2">
          <div
            v-if="selectedOption"
            class="avatar shrink-0"
          >
            <div class="h-7 w-7 rounded-full">
              <img
                v-if="resolveAvatarUrl(selectedOption.agentId)"
                :src="resolveAvatarUrl(selectedOption.agentId)"
                :alt="selectedOption.agentName"
                class="h-7 w-7 rounded-full object-cover"
              />
              <div
                v-else
                class="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-content"
              >
                {{ agentInitials(selectedOption.agentName) }}
              </div>
            </div>
          </div>
          <span class="min-w-0 flex-1 truncate" :class="selectedOption ? '' : 'text-base-content/50'">
            {{ selectedSummaryLabel }}
          </span>
        </div>
        <ChevronDown class="h-4 w-4 shrink-0 opacity-70 transition-transform" :class="dropdownOpen ? 'rotate-180' : ''" />
      </button>

      <div
        v-if="dropdownOpen && !disabled && (normalizedOptions.length > 0 || placeholder)"
        ref="dropdownPanelRef"
        class="absolute z-50 w-full max-w-full overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
        :class="dropdownDirection === 'up' ? 'bottom-full mb-2' : 'top-full mt-2'"
      >
        <div>
          <button
            v-if="placeholder"
            type="button"
            class="flex w-full items-center rounded-none px-3 py-2 text-left text-sm transition-colors hover:bg-base-200"
            :class="!selectedOption ? 'bg-base-200 font-medium' : ''"
            @click="clearSelection"
          >
            <span class="truncate">{{ placeholder }}</span>
          </button>

          <div class="min-h-0" :style="{ height: `${dropdownBodyHeight}px` }">
            <PersonaGroupGrid
              ref="personaGroupGridRef"
              :options="normalizedOptions"
              :selected-id="selectedValue"
              :avatar-url-map="props.personaAvatarUrlMap"
              @select="selectOption"
            />
          </div>
        </div>
      </div>
    </div>

    <div v-if="showModelSelector" class="min-w-0 pt-2">
      <ApiConfigPicker
        :model-value="selectedApiConfigId"
        :api-configs="textApiConfigs"
        :disabled="disabled"
        @update:model-value="handleModelSelect"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ChevronDown } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, PersonaProfile } from "../../../types/app";
import ApiConfigPicker from "../../config/components/ApiConfigPicker.vue";
import PersonaGroupGrid from "./PersonaGroupGrid.vue";
import {
  buildAgentPersonaOptions,
  agentPersonaOptionId,
  type AgentPersonaOption,
} from "../agent-persona-options";

const props = withDefaults(defineProps<{
  agentId?: string;
  apiConfigId?: string;
  personas?: PersonaProfile[];
  apiConfigs?: ApiConfigItem[];
  expertApiConfigId?: string;
  toolReviewApiConfigId?: string | null;
  options?: AgentPersonaOption[];
  placeholder?: string;
  disabled?: boolean;
  showModel?: boolean;
  autoSelectFirst?: boolean;
  preserveCurrent?: boolean;
  personaAvatarUrlMap?: Record<string, string>;
}>(), {
  agentId: "",
  apiConfigId: "",
  expertApiConfigId: "",
  placeholder: "",
  disabled: false,
  showModel: true,
  autoSelectFirst: false,
  preserveCurrent: true,
});

const emit = defineEmits<{
  "update:agentId": [value: string];
  "update:apiConfigId": [value: string];
  change: [value: { agentId: string; option: AgentPersonaOption | null }];
}>();

const { t } = useI18n();

const rootRef = ref<HTMLElement | null>(null);
const dropdownPanelRef = ref<HTMLElement | null>(null);
const personaGroupGridRef = ref<InstanceType<typeof PersonaGroupGrid> | null>(null);
const dropdownOpen = ref(false);
const dropdownDirection = ref<"up" | "down">("down");
const dropdownBodyHeight = ref(320);

const DROPDOWN_MARGIN = 16;
const DROPDOWN_GAP = 8;
const DROPDOWN_MIN_HEIGHT = 120;
const DROPDOWN_MAX_HEIGHT = 520;

const baseOptions = computed(() => (
  Array.isArray(props.options) && props.options.length > 0
    ? props.options
    : buildAgentPersonaOptions({
      personas: props.personas || [],
      apiConfigs: props.apiConfigs || [],
      expertApiConfigId: props.expertApiConfigId,
      toolReviewApiConfigId: props.toolReviewApiConfigId,
    })
));

function findAgentName(agentId: string): string {
  const option = baseOptions.value.find((item) => item.agentId === agentId);
  if (option?.agentName) return option.agentName;
  const persona = (props.personas || []).find((item) => String(item.id || "").trim() === agentId);
  return String(persona?.name || "").trim() || agentId;
}

function buildCurrentMissingOption(agentId: string): AgentPersonaOption {
  const agentName = findAgentName(agentId);
  return {
    id: agentPersonaOptionId(agentId),
    agentId,
    agentName,
    label: `${agentName} (${t("chat.personaRemoved")})`,
    name: agentName,
    ownerAgentId: agentId,
    ownerName: agentName,
    childAgentIds: [],
    unavailable: true,
  };
}

const normalizedOptions = computed(() => {
  const options = [...baseOptions.value];
  if (!props.preserveCurrent) return options;
  const agentId = String(props.agentId || "").trim();
  if (!agentId) return options;
  const key = agentPersonaOptionId(agentId);
  if (options.some((option) => option.id === key)) return options;
  return [buildCurrentMissingOption(agentId), ...options];
});

const selectedAgentId = computed(() => String(props.agentId || "").trim());
const selectedApiConfigId = computed(() => String(props.apiConfigId || "").trim());

const selectedValue = computed(() => {
  const agentId = selectedAgentId.value;
  if (!agentId) return "";
  const key = agentPersonaOptionId(agentId);
  if (normalizedOptions.value.some((option) => option.id === key)) return key;
  return "";
});

const selectedOption = computed(() =>
  normalizedOptions.value.find((option) => option.id === selectedValue.value) || null
);

const textApiConfigs = computed(() =>
  (props.apiConfigs || []).filter((item) => !!item.enableText),
);

const showModelSelector = computed(() => props.showModel && textApiConfigs.value.length > 0);

const selectedSummaryLabel = computed(() => {
  if (selectedOption.value) {
    const agentName = String(selectedOption.value.agentName || "").trim();
    const modelName = String(selectedOption.value.modelName || "").trim();
    return [agentName, modelName].filter(Boolean).join(" · ");
  }
  if (props.placeholder) return props.placeholder;
  const firstOption = normalizedOptions.value[0];
  if (!firstOption) return "";
  return [
    String(firstOption.agentName || "").trim(),
    String(firstOption.modelName || "").trim(),
  ].filter(Boolean).join(" · ");
});

function emitSelection(option: AgentPersonaOption | null) {
  const agentId = String(option?.agentId || "").trim();
  emit("update:agentId", agentId);
  emit("change", { agentId, option });
}

function selectOption(option: AgentPersonaOption) {
  emitSelection(option);
  const configId = String(option.apiConfigId || "").trim();
  if (configId) {
    emit("update:apiConfigId", configId);
  }
  dropdownOpen.value = false;
}

function clearSelection() {
  emitSelection(null);
  dropdownOpen.value = false;
}

function handleModelSelect(value: string) {
  emit("update:apiConfigId", String(value || "").trim());
}

function resolveAvatarUrl(agentId: string): string {
  return props.personaAvatarUrlMap?.[agentId] || "";
}

function agentInitials(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  const firstTwo = text.slice(0, 2);
  if (/^[A-Za-z]{2}/.test(firstTwo)) {
    return firstTwo.toUpperCase();
  }
  return text.charAt(0).toUpperCase();
}

function toggleOpen() {
  if (props.disabled || (normalizedOptions.value.length === 0 && !props.placeholder)) return;
  dropdownOpen.value = !dropdownOpen.value;
}

function clampDropdownBodyHeight(space: number): number {
  return Math.max(
    Math.min(Math.floor(space), DROPDOWN_MAX_HEIGHT),
    Math.min(DROPDOWN_MIN_HEIGHT, Math.max(96, Math.floor(space))),
  );
}

function updateDropdownLayout() {
  if (!dropdownOpen.value) return;
  const root = rootRef.value;
  if (!root) return;
  const rect = root.getBoundingClientRect();
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  const availableAbove = Math.max(0, rect.top - DROPDOWN_MARGIN - DROPDOWN_GAP);
  const availableBelow = Math.max(0, viewportHeight - rect.bottom - DROPDOWN_MARGIN - DROPDOWN_GAP);
  const shouldOpenUp = availableAbove > availableBelow && availableAbove >= DROPDOWN_MIN_HEIGHT;
  const preferredDirection = shouldOpenUp ? "up" : "down";
  const chosenSpace = preferredDirection === "up" ? availableAbove : availableBelow;
  const fallbackSpace = preferredDirection === "up" ? availableBelow : availableAbove;
  const finalSpace = Math.max(chosenSpace, fallbackSpace);
  dropdownDirection.value = preferredDirection;
  const availableHeight = clampDropdownBodyHeight(
    finalSpace >= DROPDOWN_MIN_HEIGHT ? chosenSpace : finalSpace,
  );
  const contentHeight = personaGroupGridRef.value?.contentRef?.scrollHeight ?? 0;
  const naturalHeight = contentHeight > 0 ? contentHeight : availableHeight;
  dropdownBodyHeight.value = Math.min(availableHeight, Math.max(96, naturalHeight));
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!dropdownOpen.value) return;
  const target = event.target as Node | null;
  if (rootRef.value && target && !rootRef.value.contains(target)) {
    dropdownOpen.value = false;
  }
}

watch(
  () => [props.agentId, normalizedOptions.value.map((option) => option.id).join("|")] as const,
  () => {
    if (!props.autoSelectFirst) return;
    if (selectedValue.value || normalizedOptions.value.length === 0) return;
    const firstAvailable = normalizedOptions.value.find(
      (option) => !option.unavailable && !option.personaMissing && !!String(option.agentId || "").trim(),
    );
    if (!firstAvailable) return;
    emitSelection(firstAvailable);
    const configId = String(firstAvailable.apiConfigId || "").trim();
    if (configId) {
      emit("update:apiConfigId", configId);
    }
  },
  { immediate: true },
);

watch(() => props.disabled, (disabled) => {
  if (disabled) dropdownOpen.value = false;
});

watch(dropdownOpen, async (open) => {
  if (!open) return;
  await nextTick();
  updateDropdownLayout();
});

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  window.addEventListener("resize", updateDropdownLayout, { passive: true });
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  window.removeEventListener("resize", updateDropdownLayout);
});
</script>
