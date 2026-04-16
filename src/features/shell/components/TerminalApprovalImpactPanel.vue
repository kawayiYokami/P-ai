<script setup lang="ts">
import { computed } from "vue";

type ImpactItem = {
  path: string;
  adds: number;
  removes: number;
  kind: "update" | "add" | "delete" | "other";
};

const props = defineProps<{
  approvalKind?: string;
  command?: string;
  reviewOpinion?: string;
  impactSummary: ImpactItem[];
  patchKinds?: Array<"update" | "add" | "delete" | "other">;
}>();

type RiskLevel = "high" | "medium" | "low" | "info";

const normalizedCommand = computed(() => String(props.command || "").trim());
const normalizedApprovalKind = computed(() => String(props.approvalKind || "").trim());
const normalizedPatchKinds = computed(() => Array.isArray(props.patchKinds) ? props.patchKinds : []);
const normalizedReviewOpinion = computed(() => String(props.reviewOpinion || "").trim());
const hasAiReviewSummary = computed(() =>
  normalizedApprovalKind.value.startsWith("ai_tool_review")
  && !!normalizedReviewOpinion.value,
);

const riskAssessment = computed(() => {
  const command = normalizedCommand.value.toLowerCase();
  const hasAddPatch = normalizedPatchKinds.value.includes("add");
  const hasDeletePatch = normalizedPatchKinds.value.includes("delete");
  const hasShellRedirection = />|>>/.test(command);
  const hasShellWriteIntent =
    hasShellRedirection
    || /\b(printf|echo|tee|touch|truncate)\b/.test(command)
    || /\b(set-content|add-content|out-file|new-item|copy-item|move-item|rename-item|remove-item)\b/.test(command);
  if (!command) {
    if (normalizedApprovalKind.value === "apply_patch_workspace_write") {
      const patchNotes = [
        hasAddPatch ? "包含新增文件。" : "",
        hasDeletePatch ? "包含删除文件。" : "",
      ].filter(Boolean).join(" ");
      return {
        level: "medium" as RiskLevel,
        label: "中风险",
        reason: `会直接修改工作区文件。${patchNotes}`.trim(),
      };
    }
    if (normalizedApprovalKind.value === "read_file_preview") {
      return {
        level: "low" as RiskLevel,
        label: "低风险",
        reason: "当前请求主要是读取内容，不直接写入。",
      };
    }
    return {
      level: "info" as RiskLevel,
      label: "待判断",
      reason: "这次审批没有提供可分析的命令文本。",
    };
  }

  if (/\b(rm|del|erase|format|mkfs|dd|shutdown|reboot)\b/.test(command) || /\b(remove-item|reg delete|diskpart)\b/.test(command)) {
    return {
      level: "high" as RiskLevel,
      label: "高风险",
      reason: "包含删除、格式化或系统级破坏指令。",
    };
  }

  if (hasShellWriteIntent || /\b(move|mv|copy|cp|xcopy|robocopy|mkdir|new-item|set-content|add-content|out-file|rename|ren)\b/.test(command) || /\b(git clean|git reset|git checkout)\b/.test(command)) {
    return {
      level: "medium" as RiskLevel,
      label: "中风险",
      reason: "包含文件写入、覆盖、重定向输出、移动或批量改动行为。",
    };
  }

  if (/\b(cat|type|dir|ls|pwd|rg|findstr|git status|git diff|get-childitem)\b/.test(command)) {
    return {
      level: "low" as RiskLevel,
      label: "低风险",
      reason: "看起来是查询、读取或检查型命令。",
    };
  }

  return {
    level: "info" as RiskLevel,
    label: "待判断",
    reason: "命令不在常见规则内，建议结合影响范围再确认。",
  };
});

const riskClassMap: Record<RiskLevel, string> = {
  high: "badge-error",
  medium: "badge-warning",
  low: "badge-success",
  info: "badge-ghost",
};

const impactKindLabelMap: Record<ImpactItem["kind"], string> = {
  update: "更新",
  add: "新增",
  delete: "删除",
  other: "影响",
};

const impactKindClassMap: Record<ImpactItem["kind"], string> = {
  update: "badge-info",
  add: "badge-success",
  delete: "badge-error",
  other: "badge-ghost",
};
</script>

<template>
  <div class="mt-3 space-y-3">
    <div v-if="!hasAiReviewSummary" class="rounded-box border border-base-300 bg-base-200/50 px-3 py-3">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-xs font-medium text-base-content/60">危险等级评估</span>
        <span class="badge badge-sm" :class="riskClassMap[riskAssessment.level]">{{ riskAssessment.label }}</span>
      </div>
      <div class="mt-2 text-sm text-base-content/80">
        {{ riskAssessment.reason }}
      </div>
    </div>

    <div v-if="impactSummary.length > 0">
      <div>影响范围：</div>
      <div class="mt-2 space-y-2">
        <div
          v-for="item in impactSummary"
          :key="item.path"
          class="flex flex-wrap items-center gap-2 text-xs"
        >
          <span class="badge badge-sm" :class="impactKindClassMap[item.kind]">{{ impactKindLabelMap[item.kind] }}</span>
          <span class="font-mono text-base-content/80">{{ item.path }}</span>
          <span v-if="item.adds > 0" class="text-success">+{{ item.adds }}</span>
          <span v-if="item.removes > 0" class="text-error">-{{ item.removes }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
