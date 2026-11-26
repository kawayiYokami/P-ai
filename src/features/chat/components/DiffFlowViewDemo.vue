<template>
  <div class="flex flex-col gap-4">
    <GalleryControls>
      <span class="text-xs text-base-content/60">宽度</span>
      <input v-model.number="demoWidth" type="range" min="260" max="720" step="20" class="range range-xs w-28" />
      <span class="text-xs text-base-content/50">{{ demoWidth }}px</span>
      <template #hint>
        点左侧方格跳到对应改动段，滚动时高亮跟着走。一段一项、按改动顺序平铺，同一文件改两次就是两项；点每段的标题行可以折叠该段。不套卡片、段间不加分隔线。diff 复用 TerminalApprovalPatchSample 渲染。标题行右侧的按钮是「打开文件」，跳到文件阅读面板定位到该文件。
      </template>
    </GalleryControls>

    <div
      class="relative flex h-[720px] flex-col overflow-hidden rounded-box border border-base-300 bg-base-200"
      :style="{ width: `min(${demoWidth}px, 100%)` }"
    >
      <DiffFlowView :items="items" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import DiffFlowView from "./DiffFlowView.vue";
import GalleryControls from "../../dev-gallery/GalleryControls.vue";
import type { DiffFlowItem } from "./diff-flow-types";

const demoWidth = ref(340);

const items: DiffFlowItem[] = [
  {
    key: "1:tool_impls.rs",
    path: "easy_call_ai/src-tauri/src/features/chat/model_runtime/tools_and_builtin/tool_impls.rs",
    added: 16,
    removed: 16,
    lines: [
      "@@ -31,16 +31,16 @@",
      "-ProviderToolDefinition::new(",
      '-    "automation",',
      '-    "会话自动化入口：`get` 拿目录、令牌、端点与当前清单；`reload` 重读清单并起停。",',
      "-    serde_json::json!({",
      '-        "type": "object",',
      '-        "properties": {',
      '-            "command": {',
      '-                "type": "string",',
      '-                "enum": ["get", "reload"],',
      '-                "description": "get = 拿本会话的脚本目录、激活令牌、激活端点与当前清单；reload = 改完 automations.json 后完整重读并起停。"',
      "-            }",
      "-        },",
      '-        "required": ["command"],',
      '-        "additionalProperties": false',
      "-    }),",
      "-)",
      "+ProviderToolDefinition::new(",
      '+    "automation",',
      '+    "PAI 会话自动化工具。在后台托管周期轮询或常驻运行的脚本与命令，充当本会话的事件哨兵，满足条件时主动向本会话投递消息。`get` 获取专属脚本目录、当前清单与执行指南；`reload` 在修改清单后重载起停任务。",',
      "+    serde_json::json!({",
      '+        "type": "object",',
      '+        "properties": {',
      '+            "command": {',
      '+                "type": "string",',
      '+                "enum": ["get", "reload"],',
      '+                "description": "get = 拿脚本目录、当前清单、状态与指南；reload = 修改 automations.json 后重载起停。"',
      "+            }",
      "+        },",
      '+        "required": ["command"],',
      '+        "additionalProperties": false',
      "+    }),",
      "+)",
    ],
  },
  {
    key: "2:builtin_automation.rs",
    path: "easy_call_ai/src-tauri/src/features/chat/model_runtime/tools_and_builtin/builtin_automation.rs",
    added: 3,
    removed: 1,
    lines: [
      "@@ -212,4 +212,6 @@",
      "     let manifest = automation_read_config(&config_path)?;",
      "-    if manifest.is_none() { return Ok(outcome); }",
      "+    let Some(manifest) = manifest else {",
      "+        return Ok(outcome);",
      "+    };",
      "     automation_reconcile(state, &manifest.tasks, &mut outcome)?;",
      "     Ok(outcome)",
    ],
  },
  {
    // 同一文件的第二次改动：不聚合，单独占一项，序号接着往后排
    key: "3:tool_impls.rs",
    path: "easy_call_ai/src-tauri/src/features/chat/model_runtime/tools_and_builtin/tool_impls.rs",
    added: 2,
    removed: 0,
    lines: [
      "@@ -78,3 +78,5 @@",
      "     let outcome = automation_reconcile_all(state)?;",
      "+    runtime_log_info!(\"[自动化] 清单重载完成，任务数 {}\", outcome.tasks);",
      "+    conversation_automation_broadcast_updated(app_handle, &conversation_id);",
      "     Ok(outcome)",
    ],
  },
  {
    key: "4:automation_commit_msg.txt",
    path: "easy_call_ai/.pai/temp/automation_commit_msg.txt",
    added: 6,
    removed: 0,
    lines: [
      "@@ -0,0 +1,6 @@",
      "+feat(automation): 会话自动化清单支持 description 与前端读取",
      "+",
      "+- 清单任务新增 description 字段，只用于 describe 回显，不参与调度",
      "+- 新增 automation_list_tasks 命令，前端按会话读取任务列表",
      "+- 清单重读后广播 easy-call:automation-updated，带上 conversationId",
      "+- 允许清单带额外字段，不再 deny_unknown_fields",
    ],
  },
];
</script>
