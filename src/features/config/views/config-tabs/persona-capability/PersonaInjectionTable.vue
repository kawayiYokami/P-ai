<template>
  <div class="space-y-3 min-w-0 max-w-full">
    <!-- 随身技能与加载顺序分组卡片 -->
    <ConfigCard flush class="shadow-xs min-w-0 max-w-full">
      <div class="divide-y divide-base-200/60">
        <!-- 首位固定：人格设定 -->
      <div class="flex items-center gap-3 px-4 py-3 bg-base-200/20 min-w-0 max-w-full overflow-hidden">
        <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-selector bg-base-200 text-xs font-bold text-base-content/60">
          1
        </span>
        <div class="min-w-0 flex-1 overflow-hidden">
          <div class="flex items-center gap-2 min-w-0">
            <span class="truncate text-sm font-medium text-base-content">{{ t("config.persona.injection.settings") }}</span>
            <span class="badge badge-ghost badge-xs gap-1 text-base-content/60 shrink-0">
              <Lock class="h-3 w-3" />
              {{ t("config.persona.injection.locked") }}
            </span>
          </div>
          <div class="mt-0.5 block truncate text-xs text-base-content/50 max-w-full">{{ promptSummary }}</div>
        </div>
      </div>

      <!-- 随身技能列表：按注入顺序排列 -->
      <div
        v-for="(name, index) in residentNames"
        :key="name"
        class="flex items-center gap-3 px-4 py-3 transition hover:bg-base-200/30 min-w-0 max-w-full overflow-hidden"
      >
        <span class="flex h-6 w-6 shrink-0 items-center justify-center rounded-selector bg-base-200 text-xs font-bold text-base-content/60">
          {{ index + 2 }}
        </span>
        <div class="min-w-0 flex-1 overflow-hidden">
          <div class="truncate text-sm font-medium text-base-content max-w-full">{{ name }}</div>
          <div v-if="descriptionOf(name)" class="mt-0.5 block truncate text-xs text-base-content/50 max-w-full">
            {{ descriptionOf(name) }}
          </div>
        </div>
        <!-- 操作按钮（移动与移除） -->
        <div class="flex shrink-0 items-center gap-1">
          <button
            type="button"
            class="btn btn-ghost btn-circle btn-xs h-8 w-8 text-base-content/70 hover:text-base-content"
            :disabled="index === 0"
            :title="t('config.persona.injection.moveUp')"
            @click="move(index, -1)"
          >
            <ArrowUp class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-circle btn-xs h-8 w-8 text-base-content/70 hover:text-base-content"
            :disabled="index === residentNames.length - 1"
            :title="t('config.persona.injection.moveDown')"
            @click="move(index, 1)"
          >
            <ArrowDown class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-circle btn-xs h-8 w-8 text-error/70 hover:text-error hover:bg-error/10"
            :title="t('config.persona.injection.remove')"
            @click="remove(name)"
          >
            <X class="h-4 w-4" />
          </button>
        </div>
      </div>

      <!-- 空状态（无随身技能时） -->
      <div v-if="residentNames.length === 0" class="px-4 py-4 text-center text-xs text-base-content/50">
        {{ t("config.persona.injection.empty") }}
      </div>

      <!-- 添加随身技能行 -->
      <div class="bg-base-200/10">
        <button
          v-if="!picking"
          type="button"
          class="flex w-full items-center justify-center gap-1.5 py-3 text-xs font-medium text-primary hover:bg-primary/5 transition"
          @click="picking = true"
        >
          <Plus class="h-4 w-4" />
          <span>{{ t("config.persona.injection.add") }}</span>
        </button>

        <!-- 选取器 -->
        <div v-else class="p-3 space-y-2">
          <div class="relative">
            <input
              v-model="query"
              type="text"
              class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
              :placeholder="t('config.persona.injection.addPlaceholder')"
              autofocus
            />
            <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
            <button
              v-if="query"
              type="button"
              class="btn btn-ghost btn-xs btn-circle absolute right-1.5 top-1.5 h-6 w-6 min-h-0"
              @click="query = ''"
            >
              ✕
            </button>
          </div>

          <div class="max-h-52 overflow-y-auto rounded-box border border-base-300 bg-base-100 divide-y divide-base-200/60">
            <button
              v-for="skill in candidates"
              :key="skill.name"
              type="button"
              class="flex w-full items-center justify-between gap-2 px-3.5 py-2.5 text-left hover:bg-base-200/50 transition"
              @click="append(skill.name)"
            >
              <div class="min-w-0 flex-1">
                <span class="block truncate text-xs font-medium text-base-content">{{ skill.name }}</span>
                <span v-if="skill.description" class="block truncate text-caption text-base-content/50">
                  {{ skill.description }}
                </span>
              </div>
              <Plus class="h-4 w-4 shrink-0 text-primary opacity-70" />
            </button>
            <div v-if="candidates.length === 0" class="px-4 py-5 text-center text-xs text-base-content/50">
              {{ t("config.persona.injection.noCandidate") }}
            </div>
          </div>

          <div class="flex justify-end pt-1">
            <button type="button" class="btn btn-ghost btn-xs text-xs" @click="cancelPicking">
              {{ t("common.cancel") }}
            </button>
          </div>
        </div>
      </div>
      </div>
    </ConfigCard>

    <!-- 底部微提示 -->
    <p class="px-1 text-xs text-base-content/40 leading-normal">
      {{ t("config.persona.injection.hint") }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDown, ArrowUp, Lock, Plus, Search, X } from "@lucide/vue";
import ConfigCard from "../../../components/ConfigCard.vue";
import type { PersonaProfile, SkillSummaryItem } from "../../../../../types/app";

const props = defineProps<{
  persona: PersonaProfile;
  skills: SkillSummaryItem[];
}>();

const { t } = useI18n();

const picking = ref(false);
const query = ref("");

const residentNames = computed(() =>
  (props.persona?.residentSkillNames || [])
    .map((item) => String(item || "").trim())
    .filter(Boolean),
);

const promptSummary = computed(() => {
  const raw = String(props.persona?.systemPrompt || "").replace(/\s+/g, " ").trim();
  return raw || t("config.persona.injection.promptEmpty");
});

function descriptionOf(name: string): string {
  const skill = props.skills.find((item) => String(item.name || "").trim() === name);
  return String(skill?.description || "").trim();
}

const candidates = computed(() => {
  const mounted = new Set(residentNames.value);
  const keyword = query.value.trim().toLowerCase();
  return props.skills.filter((skill) => {
    const name = String(skill.name || "").trim();
    if (!name || mounted.has(name)) return false;
    if (!keyword) return true;
    return (
      name.toLowerCase().includes(keyword) ||
      String(skill.description || "").toLowerCase().includes(keyword)
    );
  });
});

function setNames(names: string[]) {
  props.persona.residentSkillNames = names;
}

function move(index: number, delta: number) {
  const names = [...residentNames.value];
  const target = index + delta;
  if (target < 0 || target >= names.length) return;
  const [item] = names.splice(index, 1);
  names.splice(target, 0, item);
  setNames(names);
}

function remove(name: string) {
  setNames(residentNames.value.filter((item) => item !== name));
}

function append(name: string) {
  const trimmed = String(name || "").trim();
  if (!trimmed || residentNames.value.includes(trimmed)) return;
  setNames([...residentNames.value, trimmed]);
  cancelPicking();
}

function cancelPicking() {
  picking.value = false;
  query.value = "";
}
</script>
