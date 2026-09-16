<template>
  <div class="space-y-4 min-w-0 max-w-full">
    <!-- 权限模式选择器 -->
    <ConfigCard flush class="p-3.5 sm:p-4 space-y-2.5 min-w-0 max-w-full">
      <SegmentedControl
        :model-value="uiMode"
        :options="permissionModeOptions"
        size="sm"
        class="w-full"
        @change="setUiMode"
      />
      <p class="text-xs text-base-content/50 leading-normal px-0.5">
        {{ modeHint }}
      </p>
    </ConfigCard>

    <div v-if="loading" class="flex items-center justify-center gap-2 py-8 text-sm opacity-60">
      <span class="loading loading-spinner loading-xs"></span>
      <span>{{ t("config.persona.permission.loading") }}</span>
    </div>

    <div v-else-if="loadError" class="rounded-box border border-error/20 bg-error/5 p-4 text-xs text-error">
      {{ t("config.persona.permission.loadFailed", { err: loadError }) }}
    </div>

    <div v-else class="space-y-3 min-w-0 max-w-full">
      <div v-if="skillPermissionRequiresExec" class="rounded-box border border-warning/20 bg-warning/5 p-3 text-xs text-warning">
        {{ t("config.persona.permission.skillsRequireExec") }}
      </div>
      <PermissionToolTree
        :sections="sections"
        @leaf-toggle="onLeafToggle"
        @group-toggle="onGroupToggle"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { AgentPermissionMode, PermissionCatalog, PersonaProfile } from "../../../../../types/app";
import { ensurePermissionControl } from "../../../utils/persona-capability";
import {
  buildBuiltinToolGroups,
  buildMcpToolGroups,
  type PermissionLeafCategory,
  type PermissionTreeSection,
} from "../../../utils/permission-tree";
import PermissionToolTree from "../../../components/PermissionToolTree.vue";
import ConfigCard from "../../../components/ConfigCard.vue";
import SegmentedControl, { type SegmentedControlOption } from "../../../components/SegmentedControl.vue";

type PermissionUiMode = "off" | "blacklist" | "whitelist";

const props = defineProps<{
  persona: PersonaProfile;
  catalog: PermissionCatalog;
  loading?: boolean;
  loadError?: string;
}>();

const { t } = useI18n();

const control = computed(() => ensurePermissionControl(props.persona));

const listDisabled = computed(() => !control.value.enabled);

const permissionExecAllowed = computed(() => {
  const current = control.value;
  if (!current.enabled) return false;
  const execSelected = (current.builtinToolNames || []).includes("exec");
  return current.mode === "whitelist" ? execSelected : !execSelected;
});

// skill 权限依赖 exec：白名单没开 exec（或黑名单禁了 exec）时，模型取不到技能文件。
const skillPermissionRequiresExec = computed(() => control.value.enabled && !permissionExecAllowed.value);
const skillListDisabled = computed(() => listDisabled.value || skillPermissionRequiresExec.value);

const BUILTIN_TOOL_GROUP_LABEL_KEYS: Record<string, string> = {
  files: "config.persona.permission.groupFiles",
  execConfig: "config.persona.permission.groupExecConfig",
  desktop: "config.persona.permission.groupDesktop",
  web: "config.persona.permission.groupWeb",
  delegate: "config.persona.permission.groupDelegate",
  media: "config.persona.permission.groupMedia",
  other: "config.persona.permission.groupOther",
};

const sections = computed<PermissionTreeSection[]>(() => {
  const current = control.value;
  const checkedSetFor = (category: PermissionLeafCategory) =>
    new Set((current[category] || []).map((value) => String(value || "").trim()).filter(Boolean));
  const builtinChecked = checkedSetFor("builtinToolNames");
  const skillChecked = checkedSetFor("skillNames");
  const mcpChecked = checkedSetFor("mcpToolNames");
  return [
    {
      key: "builtinToolNames",
      label: t("config.persona.permission.sectionBuiltin"),
      disabled: listDisabled.value,
      groups: buildBuiltinToolGroups(
        props.catalog?.builtinTools || [],
        (name) => builtinChecked.has(name),
        (groupKey) => t(BUILTIN_TOOL_GROUP_LABEL_KEYS[groupKey] ?? "config.persona.permission.groupOther"),
      ),
      leaves: [],
    },
    {
      key: "skillNames",
      label: t("config.persona.permission.sectionSkills"),
      disabled: skillListDisabled.value,
      groups: [],
      leaves: (props.catalog?.skills || []).map((item) => {
        // 随身技能恒可用，在权限页里锁定：恒勾选、不可取消，也不参与名单勾选。
        const resident = (props.persona.residentSkillNames || []).includes(item.name);
        return {
          category: "skillNames" as const,
          name: item.name,
          displayName: item.name,
          description: item.description,
          enabled: resident || skillChecked.has(item.name),
          disabled: resident,
          locked: resident,
          badge: resident ? t("config.persona.permission.badgeResident") : undefined,
        };
      }),
    },
    {
      key: "mcpToolNames",
      label: t("config.persona.permission.sectionMcp"),
      disabled: listDisabled.value,
      groups: buildMcpToolGroups(
        props.catalog?.mcpTools || [],
        (name) => mcpChecked.has(name),
        t("config.persona.permission.groupOther"),
      ),
      leaves: [],
    },
  ];
});

function normalizeNames(names: string[]): string[] {
  return Array.from(
    new Set((names || []).map((value) => String(value || "").trim()).filter(Boolean)),
  );
}

const uiMode = computed<PermissionUiMode>(() => {
  const current = control.value;
  if (!current.enabled) return "off";
  return current.mode === "whitelist" ? "whitelist" : "blacklist";
});

const permissionModeOptions = computed<SegmentedControlOption<PermissionUiMode>[]>(() => [
  { value: "off", label: t("config.persona.permission.modeUnrestricted") },
  { value: "blacklist", label: t("config.persona.permission.modeBlacklist") },
  { value: "whitelist", label: t("config.persona.permission.modeWhitelist") },
]);

const modeHint = computed(() => {
  if (uiMode.value === "off") return t("config.persona.permission.hintOff");
  return t(
    uiMode.value === "whitelist"
      ? "config.persona.permission.hintWhitelist"
      : "config.persona.permission.hintBlacklist",
  );
});

function setUiMode(next: PermissionUiMode) {
  const current = control.value;
  if (next === "off") {
    current.enabled = false;
    return;
  }
  current.enabled = true;
  current.mode = next as AgentPermissionMode;
}

function setNamesBatch(category: PermissionLeafCategory, names: string[], checked: boolean) {
  const current = control.value;
  const next = new Set(normalizeNames(current[category]));
  for (const raw of names) {
    const trimmed = String(raw || "").trim();
    if (!trimmed) continue;
    if (checked) {
      next.add(trimmed);
    } else {
      next.delete(trimmed);
    }
  }
  current[category] = Array.from(next);
}

function onLeafToggle(payload: { category: PermissionLeafCategory; name: string; checked: boolean }) {
  setNamesBatch(payload.category, [payload.name], payload.checked);
}

function onGroupToggle(payload: { category: PermissionLeafCategory; names: string[]; checked: boolean }) {
  setNamesBatch(payload.category, payload.names, payload.checked);
}
</script>
