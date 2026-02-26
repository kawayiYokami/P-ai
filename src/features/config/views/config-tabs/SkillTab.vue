<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <div class="text-sm opacity-70">SKILL 列表</div>
        <select
          v-if="skills.length > 0"
          v-model="selectedSkillPath"
          class="select select-bordered w-[clamp(14rem,40vw,34rem)] max-w-full"
          :disabled="loading"
        >
          <option v-for="item in skills" :key="item.path" :value="item.path">
            {{ item.name }}
          </option>
        </select>
      </div>
      <div class="flex items-center gap-2">
        <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200" type="button" @click="reload" :disabled="loading">刷新</button>
        <button class="btn btn-sm btn-primary" type="button" @click="openSkillsDir" :disabled="loading">打开目录</button>
      </div>
    </div>

    <div v-if="loading" class="text-sm opacity-70">加载中...</div>

    <div v-if="selectedSkill" class="rounded-md border border-base-300 bg-base-100 p-3 space-y-2">
      <div class="text-sm font-semibold">{{ selectedSkill.name }}</div>
      <div class="text-sm opacity-80 whitespace-pre-wrap">{{ selectedSkill.description || "(无描述)" }}</div>
      <div class="text-[11px] opacity-60 break-all">{{ selectedSkill.path }}</div>
    </div>

    <div v-if="statusText" class="text-sm" :class="statusError ? 'text-error' : 'opacity-70'">
      {{ statusText }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { invokeTauri } from "../../../../services/tauri-api";
import type { SkillListResult, SkillSummaryItem } from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";

let skillTabCacheLoaded = false;
let skillTabCacheItems: SkillSummaryItem[] = [];

const loading = ref(false);
const statusText = ref("");
const statusError = ref(false);
const skills = ref<SkillSummaryItem[]>([]);
const selectedSkillPath = ref("");

const selectedSkill = computed(() => skills.value.find((v) => v.path === selectedSkillPath.value) ?? null);

function ensureSelectedSkill() {
  if (skills.value.length === 0) {
    selectedSkillPath.value = "";
    return;
  }
  if (!skills.value.some((v) => v.path === selectedSkillPath.value)) {
    selectedSkillPath.value = skills.value[0].path;
  }
}

function setStatus(text: string, isError = false) {
  statusText.value = text;
  statusError.value = isError;
}

async function reload() {
  loading.value = true;
  try {
    const result = await invokeTauri<SkillListResult>("mcp_list_skills");
    skills.value = result?.skills || [];
    skillTabCacheItems = [...skills.value];
    skillTabCacheLoaded = true;
    ensureSelectedSkill();
    if ((result?.errors?.length || 0) > 0) {
      setStatus(`已加载 ${skills.value.length} 个 SKILL，${result.errors.length} 个目录读取失败`, true);
    } else {
      setStatus(`已加载 ${skills.value.length} 个 SKILL`);
    }
  } catch (error) {
    setStatus(`刷新失败: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function openSkillsDir() {
  if (loading.value) return;
  loading.value = true;
  try {
    const opened = await invokeTauri<string>("skill_open_workspace_dir");
    setStatus(`已打开目录: ${opened}`);
  } catch (error) {
    setStatus(`打开目录失败: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

if (skillTabCacheLoaded) {
  skills.value = [...skillTabCacheItems];
  ensureSelectedSkill();
  setStatus(`已加载 ${skills.value.length} 个 SKILL（缓存）`);
} else {
  void reload();
}
</script>
