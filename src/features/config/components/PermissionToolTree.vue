<template>
  <div class="space-y-3 min-w-0 max-w-full">
    <div
      v-for="section in sections"
      :key="section.key"
      class="rounded-box border border-base-300 bg-base-100 overflow-hidden shadow-xs transition-opacity min-w-0 max-w-full"
      :class="section.disabled ? 'opacity-50' : ''"
    >
      <!-- 分区头部（内置工具 / 技能 / MCP 工具） -->
      <div class="flex items-center justify-between px-4 py-3 bg-base-200/30 gap-2 min-w-0 max-w-full">
        <button
          type="button"
          class="flex items-center gap-2.5 text-left min-w-0 flex-1 py-0.5 cursor-pointer select-none"
          @click="toggleExpand(expandKeySection(section.key))"
        >
          <ChevronDown
            v-if="isExpanded(expandKeySection(section.key))"
            class="h-4 w-4 shrink-0 text-base-content/50"
          />
          <ChevronRight
            v-else
            class="h-4 w-4 shrink-0 text-base-content/50"
          />
          <span class="truncate text-sm font-semibold text-base-content">
            {{ section.label }}
          </span>
          <span class="badge badge-sm badge-ghost text-xs font-mono shrink-0">
            {{ enabledCountOf(section) }}/{{ editableLeaves(section).length }}
          </span>
        </button>

        <!-- 分区全选 / 全不选按钮 -->
        <button
          v-if="!section.disabled && editableLeaves(section).length > 0"
          type="button"
          class="btn btn-ghost btn-xs h-7 px-2.5 rounded-selector text-xs font-normal gap-1.5 shrink-0"
          :title="sectionState(section) === 'all' ? t('config.persona.permission.selectNone') : t('config.persona.permission.selectAll')"
          @click="emitSectionToggle(section, sectionState(section) !== 'all')"
        >
          <span
            class="flex h-4 w-4 items-center justify-center rounded-selector border transition"
            :class="stateBoxClasses(sectionState(section))"
          >
            <Check v-if="sectionState(section) === 'all'" class="h-3 w-3" stroke-width="3" />
            <Minus v-else-if="sectionState(section) === 'partial'" class="h-3 w-3" stroke-width="3" />
          </span>
          <span class="text-xs text-base-content/70">
            {{ sectionState(section) === 'all' ? t("config.persona.permission.selectNone") : t("config.persona.permission.selectAll") }}
          </span>
        </button>
      </div>

      <!-- 分区展开内容 -->
      <div v-if="isExpanded(expandKeySection(section.key))" class="border-t border-base-200/60">
        <!-- 带分组的内容（如内置工具分类、MCP 分组） -->
        <div v-if="section.groups.length > 0" class="divide-y divide-base-200/50">
          <div v-for="group in section.groups" :key="group.key" class="overflow-hidden">
            <!-- 子分组标题栏 -->
            <div class="flex items-center justify-between px-4 py-2 bg-base-200/20 text-xs min-w-0 max-w-full">
              <button
                type="button"
                class="flex items-center gap-2 text-left min-w-0 flex-1 cursor-pointer font-medium text-base-content/70 select-none"
                @click="toggleExpand(expandKeyGroup(section.key, group.key))"
              >
                <ChevronDown
                  v-if="isExpanded(expandKeyGroup(section.key, group.key))"
                  class="h-3.5 w-3.5 shrink-0 text-base-content/40"
                />
                <ChevronRight
                  v-else
                  class="h-3.5 w-3.5 shrink-0 text-base-content/40"
                />
                <span class="truncate">{{ group.label }}</span>
                <span class="text-caption font-mono text-base-content/40 shrink-0">
                  ({{ groupEnabledCount(group) }}/{{ groupEditableCount(group) }})
                </span>
              </button>

              <button
                v-if="!section.disabled && groupEditableCount(group) > 0"
                type="button"
                class="btn btn-ghost btn-xs h-6 px-1.5 rounded-selector text-caption font-normal gap-1 shrink-0"
                :title="group.state === 'all' ? t('config.persona.permission.selectNone') : t('config.persona.permission.selectAll')"
                @click="emitGroupToggle(section, group, group.state !== 'all')"
              >
                <span
                  class="flex h-3.5 w-3.5 items-center justify-center rounded-selector border transition"
                  :class="stateBoxClasses(group.state)"
                >
                  <Check v-if="group.state === 'all'" class="h-2.5 w-2.5" stroke-width="3" />
                  <Minus v-else-if="group.state === 'partial'" class="h-2.5 w-2.5" stroke-width="3" />
                </span>
                <span class="text-base-content/60">
                  {{ group.state === 'all' ? t("config.persona.permission.selectNone") : t("config.persona.permission.selectAll") }}
                </span>
              </button>
            </div>

            <!-- 子分组内的叶子列表 -->
            <div
              v-if="isExpanded(expandKeyGroup(section.key, group.key))"
              class="divide-y divide-base-200/40 bg-base-100"
            >
              <button
                v-for="leaf in group.leaves"
                :key="leaf.name"
                type="button"
                class="flex w-full min-w-0 max-w-full items-center justify-between gap-3 px-4 py-3 text-left transition hover:bg-base-200/30 active:bg-base-200/50 overflow-hidden"
                :class="isLeafDisabled(section, leaf) ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
                :disabled="isLeafDisabled(section, leaf)"
                @click="emitLeafToggle(section, leaf, !leaf.enabled)"
              >
                <div class="min-w-0 flex-1 overflow-hidden">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="font-mono text-xs sm:text-sm font-medium text-base-content truncate">{{ leaf.displayName }}</span>
                    <span
                      v-if="leaf.badge"
                      class="badge badge-ghost badge-xs gap-1 opacity-70 shrink-0"
                    >
                      <Lock v-if="leaf.locked" class="h-3 w-3" />
                      {{ leaf.badge }}
                    </span>
                  </div>
                  <div v-if="leaf.description" class="mt-0.5 block truncate text-xs text-base-content/50 max-w-full">
                    {{ leaf.description }}
                  </div>
                </div>

                <span
                  class="flex h-5 w-5 shrink-0 items-center justify-center rounded-selector border transition ml-auto"
                  :class="leaf.enabled
                    ? 'border-primary bg-primary text-primary-content shadow-xs'
                    : 'border-base-content/25 bg-transparent text-transparent'"
                >
                  <Check class="h-3.5 w-3.5" stroke-width="3" />
                </span>
              </button>
            </div>
          </div>
        </div>

        <!-- 无分组的直接叶子列表（如技能列表） -->
        <div v-else class="divide-y divide-base-200/40 bg-base-100">
          <button
            v-for="leaf in section.leaves"
            :key="leaf.name"
            type="button"
            class="flex w-full min-w-0 max-w-full items-center justify-between gap-3 px-4 py-3 text-left transition hover:bg-base-200/30 active:bg-base-200/50 overflow-hidden"
            :class="isLeafDisabled(section, leaf) ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
            :disabled="isLeafDisabled(section, leaf)"
            @click="emitLeafToggle(section, leaf, !leaf.enabled)"
          >
            <div class="min-w-0 flex-1 overflow-hidden">
              <div class="flex items-center gap-2 min-w-0">
                <span class="text-sm font-medium text-base-content truncate">{{ leaf.displayName }}</span>
                <span
                  v-if="leaf.badge"
                  class="badge badge-ghost badge-xs gap-1 opacity-70 shrink-0"
                >
                  <Lock v-if="leaf.locked" class="h-3 w-3" />
                  {{ leaf.badge }}
                </span>
              </div>
              <div v-if="leaf.description" class="mt-0.5 block truncate text-xs text-base-content/50 max-w-full">
                {{ leaf.description }}
              </div>
            </div>

            <span
              class="flex h-5 w-5 shrink-0 items-center justify-center rounded-selector border transition ml-auto"
              :class="leaf.enabled
                ? 'border-primary bg-primary text-primary-content shadow-xs'
                : 'border-base-content/25 bg-transparent text-transparent'"
            >
              <Check class="h-3.5 w-3.5" stroke-width="3" />
            </span>
          </button>

          <div v-if="section.leaves.length === 0" class="px-4 py-6 text-center text-xs text-base-content/50">
            {{ t("config.persona.injection.noCandidate") }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, ChevronDown, ChevronRight, Lock, Minus } from "@lucide/vue";
import type {
  PermissionGroupState,
  PermissionTreeGroup,
  PermissionTreeLeaf,
  PermissionTreeSection,
} from "../utils/permission-tree";

defineProps<{
  sections: PermissionTreeSection[];
}>();

const emit = defineEmits<{
  (e: "leafToggle", payload: { category: PermissionTreeLeaf["category"]; name: string; checked: boolean }): void;
  (e: "groupToggle", payload: { category: PermissionTreeLeaf["category"]; names: string[]; checked: boolean }): void;
}>();

const { t } = useI18n();

const collapsedKeys = ref<Set<string>>(new Set());

function expandKeySection(sectionKey: string) {
  return `sec:${sectionKey}`;
}

function expandKeyGroup(sectionKey: string, groupKey: string) {
  return `grp:${sectionKey}:${groupKey}`;
}

function isExpanded(key: string) {
  return !collapsedKeys.value.has(key);
}

function toggleExpand(key: string) {
  const next = new Set(collapsedKeys.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  collapsedKeys.value = next;
}

function sectionLeaves(section: PermissionTreeSection) {
  return [...section.groups.flatMap((group) => group.leaves), ...section.leaves];
}

// 被禁用（如已是随身技能）的叶子不受名单管辖，不参与状态聚合与全选。
function editableLeaves(section: PermissionTreeSection) {
  return sectionLeaves(section).filter((leaf) => !leaf.disabled);
}

function enabledCountOf(section: PermissionTreeSection): number {
  return editableLeaves(section).filter((leaf) => leaf.enabled).length;
}

function groupEditableCount(group: PermissionTreeGroup): number {
  return group.leaves.filter((leaf) => !leaf.disabled).length;
}

function groupEnabledCount(group: PermissionTreeGroup): number {
  return group.leaves.filter((leaf) => !leaf.disabled && leaf.enabled).length;
}

function isLeafDisabled(section: PermissionTreeSection, leaf: PermissionTreeLeaf) {
  return section.disabled || !!leaf.disabled;
}

function aggregateState(leaves: PermissionTreeLeaf[]): PermissionGroupState {
  const enabledCount = leaves.filter((leaf) => leaf.enabled).length;
  if (enabledCount === 0) return "none";
  if (enabledCount === leaves.length) return "all";
  return "partial";
}

function sectionState(section: PermissionTreeSection) {
  return aggregateState(editableLeaves(section));
}

function stateBoxClasses(state: PermissionGroupState) {
  if (state === "all") return "border-primary bg-primary text-primary-content shadow-xs";
  if (state === "partial") return "border-primary/50 bg-primary/20 text-primary";
  return "border-base-content/25 bg-transparent text-transparent";
}

function emitLeafToggle(section: PermissionTreeSection, leaf: PermissionTreeLeaf, checked: boolean) {
  if (isLeafDisabled(section, leaf)) return;
  emit("leafToggle", { category: leaf.category, name: leaf.name, checked });
}

function emitGroupToggle(section: PermissionTreeSection, group: PermissionTreeGroup, checked: boolean) {
  if (section.disabled) return;
  const names = group.leaves.filter((leaf) => !leaf.disabled).map((leaf) => leaf.name);
  emit("groupToggle", { category: section.key, names, checked });
}

function emitSectionToggle(section: PermissionTreeSection, checked: boolean) {
  if (section.disabled) return;
  emit("groupToggle", { category: section.key, names: editableLeaves(section).map((leaf) => leaf.name), checked });
}
</script>
