<template>
  <div class="space-y-3 pt-2">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">预设：</span>
      <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'single' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'single'">shell单行</button>
      <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'singleLong' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'singleLong'">shell 10 行</button>
      <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'ten' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'ten'">patch 10 行</button>
      <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'multi' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'multi'">多条 3 题</button>
      <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'custom' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'custom'">多选项</button>
      <button type="button" class="btn btn-xs btn-ghost" @click="resetDemoQuestionAnswers">重置</button>
      <span v-if="demoQuestionLastSubmit" class="text-xs text-success">已提交 {{ demoQuestionLastSubmit.length }} 题</span>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <label class="flex cursor-pointer items-center gap-1.5 text-xs">
        <input type="checkbox" class="checkbox checkbox-xs" v-model="demoWithWorkspace">
        <span>演示目录全权（可记忆工作区）</span>
      </label>
      <span v-if="demoWithWorkspace" class="text-xs text-base-content/50">右上角会出现“目录全权”</span>
    </div>
    <div class="flex justify-center">
      <ChatQuestionPanel
        :key="demoQuestionPreset + String(demoWithWorkspace)"
        :items="demoQuestionItems"
        v-model="demoQuestionAnswers"
        class="flex-1"
        @submit="onDemoQuestionSubmit"
        @approve-for-workspace="onDemoQuestionApproveForWorkspace"
      />
    </div>
    <div v-if="demoQuestionLastWorkspaceApproved" class="alert alert-success py-2 text-xs">
      <span>已演示触发目录全权：{{ demoQuestionLastWorkspaceApproved }}</span>
    </div>
    <div v-if="demoQuestionLastSubmit" class="mockup-code max-h-64 overflow-auto text-xs">
      <pre class="whitespace-pre-wrap break-all"><code>{{ JSON.stringify(demoQuestionLastSubmit, undefined, 2) }}</code></pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import ChatQuestionPanel from "../../chat/components/ChatQuestionPanel.vue";
import type { QuestionItem as ChatQuestionItem } from "../../chat/components/ChatQuestionPanel.vue";

const demoQuestionPreset = ref<"single" | "singleLong" | "multi" | "ten" | "custom">("single");
const demoWithWorkspace = ref(true);
const demoQuestionLastWorkspaceApproved = ref("");
const demoQuestionAnswers = ref<Record<string, { optionId: string; label: string; comment: string }>>({});
const demoQuestionLastSubmit = ref<Array<{ id: string; optionId: string; label: string; comment: string }> | null>(null);
const demoQuestionItems = computed<ChatQuestionItem[]>(() => {
  if (demoQuestionPreset.value === "single") {
    return [
      {
        id: "q-single",
        title: "是否允许执行此命令？",
        description: "单行 shell 命令示例",
        previewText: "$ pnpm exec eslint src --ext .ts,.tsx --max-warnings 0 --format stylish --cache --cache-location .eslintcache",
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "singleLong") {
    const tenShell = [
      "$ pnpm exec eslint src --ext .ts,.tsx --max-warnings 0",
      "$ pnpm typecheck",
      "$ pnpm build",
      "$ cargo check --manifest-path src-tauri/Cargo.toml",
      "$ cargo test --manifest-path src-tauri/Cargo.toml",
      "$ pnpm exec vitest run --reporter=basic",
      "$ git status --short",
      "$ git diff --stat",
      "$ pnpm changelog:build",
      "$ pnpm tauri build --debug",
    ].join("\n");
    return [
      {
        id: "q-single-long",
        title: "是否允许执行这组命令？（10 行 shell）",
        description: "10 行 shell 命令，超过 6 行应在内容区内滚动",
        previewText: tenShell,
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "custom") {
    return [
      {
        id: "q-custom",
        title: "请选择处理方式",
        description: "演示多行选项架构：直接选项为纯按钮，补充选项为输入框+按钮联体。",
        previewText: "你可以扩展任意数量的选项，AI提问也能复用同一张卡。",
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "direct-a", label: "直接同意", kind: "direct" },
          { id: "with-a", label: "同意并补充", kind: "withInput", placeholder: "补充说明（可选）" },
          { id: "with-b", label: "拒绝并说明原因", kind: "withInput", placeholder: "请填写拒绝原因（必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "ten") {
    const tenPatch = [
      "*** Begin Patch",
      "*** Update File: src/features/chat/components/ChatQuestionPanel.vue",
      "@@ -1,10 +1,15 @@",
      ...Array.from({ length: 10 }, (_, i) => `- 旧行 ${i + 1}: const a${i} = ${i}`),
      ...Array.from({ length: 10 }, (_, i) => `+ 新行 ${i + 1}: const b${i} = ${i * 2}`),
      "*** End Patch",
    ].join("\n");
    return [
      {
        id: "q-ten",
        title: "10 行 diff 演示",
        description: "超过 6 行的 diff，应在内容区内滚动并保留红绿高亮。",
        previewText: tenPatch,
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  return [
    {
      id: "q-1",
      title: "是否允许改写 src/App.vue ?",
      description: "删除 12 行，新增 8 行",
      previewText: "*** Begin Patch\n*** Update File: src/App.vue\n@@\n- 旧行\n+ 新行",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
    {
      id: "q-2",
      title: "是否允许删除 src/old.ts ?",
      description: "该文件已无引用，删除后需回归 typecheck。",
      previewText: "DELETE src/old.ts (42 行)",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
    {
      id: "q-3",
      title: "是否允许新增 src/utils/util.ts ?",
      description: "新增通用工具，约 60 行。",
      previewText: "*** Add File: src/utils/util.ts\n+ export function helper() {}",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
  ];
});

function resetDemoQuestionAnswers() {
  demoQuestionAnswers.value = {};
  demoQuestionLastSubmit.value = null;
  demoQuestionLastWorkspaceApproved.value = "";
}

function onDemoQuestionSubmit(payload: Array<{ id: string; optionId: string; label: string; comment: string }>) {
  demoQuestionLastSubmit.value = payload;
}

function onDemoQuestionApproveForWorkspace(requestId: string) {
  demoQuestionLastWorkspaceApproved.value = String(requestId || "").trim() || "unknown";
}

watch(demoQuestionPreset, () => {
  resetDemoQuestionAnswers();
});
watch(demoWithWorkspace, () => {
  resetDemoQuestionAnswers();
});
</script>
