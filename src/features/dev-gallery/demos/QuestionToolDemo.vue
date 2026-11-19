<template>
  <div class="space-y-3 pt-2">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">预设：</span>
      <button type="button" class="btn btn-xs" :class="demoQuestionToolItems.length > 0 ? 'btn-primary' : 'btn-ghost'" @click="setDemoQuestionToolPreset('pending')">3 题待答（#2/#5/#7）</button>
      <button type="button" class="btn btn-xs" :class="demoQuestionToolItems.length === 0 ? 'btn-primary' : 'btn-ghost'" @click="setDemoQuestionToolPreset('empty')">全部发完（空态）</button>
      <button type="button" class="btn btn-xs btn-ghost" @click="resetDemoQuestionTool">重置</button>
      <span v-if="demoQuestionToolLastSubmit" class="text-xs text-success">已发送 {{ demoQuestionToolLastSubmitLabel }}<template v-if="demoQuestionToolItems.length > 0">，还剩 {{ demoQuestionToolItems.length }} 题</template></span>
    </div>
    <div v-if="demoQuestionToolPanelItems.length > 0" class="flex justify-center">
      <ChatQuestionPanel
        key="demo-question-tool"
        :items="demoQuestionToolPanelItems"
        v-model="demoQuestionToolAnswers"
        submit-mode="manual"
        class="flex-1"
        @submit="onDemoQuestionToolSubmit"
        @reject="onDemoQuestionToolReject"
      />
    </div>
    <div v-else class="rounded-lg border border-dashed border-base-300 p-6 text-center text-sm text-base-content/60">
      当前会话没有问题：面板不渲染，输入面板恢复原样
    </div>
    <div v-if="demoQuestionToolLastSubmit" class="mockup-code max-h-64 overflow-auto text-xs">
      <pre class="whitespace-pre-wrap break-all"><code>{{ JSON.stringify(demoQuestionToolLastSubmit, undefined, 2) }}</code></pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import ChatQuestionPanel from "../../chat/components/ChatQuestionPanel.vue";
import {
  buildConversationQuestionItems,
  pruneConversationQuestionAnswers,
  type ConversationQuestionView,
} from "../../chat/utils/conversation-question";

/** 模拟后端 `question_list` 返回：编号不连续（#2/#5/#7），验证面板按实际编号显示。 */
const demoQuestionToolInitialItems: ConversationQuestionView[] = [
  {
    number: 2,
    title: "要不要现在发布？",
    background: "改动已经全绿，但 release tag 还没写。",
    options: ["现在发布", "再等等"],
  },
  { number: 5, title: "用哪个分支？", options: ["main", "release"] },
  { number: 7, title: "回滚由谁执行？", options: ["我自己来", "交给值班"] },
];
const demoQuestionToolItems = ref<ConversationQuestionView[]>([...demoQuestionToolInitialItems]);
const demoQuestionToolAnswers = ref<Record<string, { optionId: string; label: string; comment: string }>>({});
const demoQuestionToolLastSubmit = ref<Array<{ id: string; optionId: string; label: string; comment: string }> | null>(null);

/** 走真实映射，和 ChatView 是同一份实现，不手写静态复现。 */
const demoQuestionToolPanelItems = computed(() =>
  buildConversationQuestionItems(demoQuestionToolItems.value),
);

/** 逐题提交时每次只有一题，展示编号比展示条数更有信息量。 */
const demoQuestionToolLastSubmitLabel = computed(() =>
  (demoQuestionToolLastSubmit.value ?? [])
    .map((answer) => `#${answer.id} ${answer.label}`)
    .join("、"),
);

function resetDemoQuestionTool() {
  demoQuestionToolItems.value = [...demoQuestionToolInitialItems];
  demoQuestionToolAnswers.value = {};
  demoQuestionToolLastSubmit.value = null;
}

function setDemoQuestionToolPreset(preset: "pending" | "empty") {
  demoQuestionToolAnswers.value = {};
  demoQuestionToolLastSubmit.value = null;
  demoQuestionToolItems.value = preset === "empty" ? [] : [...demoQuestionToolInitialItems];
}

/** 演示后端答题的效果：该编号从列表移除，答完全部就回到空态。 */
function onDemoQuestionToolSubmit(
  payload: Array<{ id: string; optionId: string; label: string; comment: string }>,
) {
  demoQuestionToolLastSubmit.value = payload;
  const answered = new Set(payload.map((answer) => Number(answer.id)));
  demoQuestionToolItems.value = demoQuestionToolItems.value.filter(
    (item) => !answered.has(item.number),
  );
  demoQuestionToolAnswers.value = {};
}

/** 演示拒绝回答：只把被拒的题移除，不产生任何"已发送"记录，其余题的勾选保留。 */
function onDemoQuestionToolReject(number: number) {
  demoQuestionToolItems.value = demoQuestionToolItems.value.filter(
    (item) => item.number !== number,
  );
  demoQuestionToolAnswers.value = pruneConversationQuestionAnswers(
    demoQuestionToolItems.value,
    demoQuestionToolAnswers.value,
  );
}
</script>
