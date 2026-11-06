<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">宽度</span>
      <input v-model.number="demoWidth" type="range" min="320" max="900" step="20" class="range range-xs w-28" />
      <span class="text-xs text-base-content/50">{{ demoWidth }}px</span>
      <span class="text-xs text-base-content/40">整页宽度只影响这一栏；气泡里的折叠块、工具时间线、思维链吸顶全部是真实组件。</span>
    </div>

    <!-- ==================== 样本集 ==================== -->
    <section class="flex flex-col gap-5">
      <div v-for="sample in sampleViews" :key="sample.id" class="flex flex-col gap-1">
        <div class="text-xs text-base-content/55">{{ sample.caption }}</div>
        <div :style="{ width: `min(${demoWidth}px, 100%)` }">
          <ChatMessageItem
            v-if="sample.block"
            v-bind="chatItemProps(sample.block)"
            v-on="chatItemHandlers"
          />
          <div v-else class="rounded border border-warning/40 bg-warning/10 p-2 text-xs">
            这条样本没有派生出展示块（sourceMessageId={{ sample.id }}）。
          </div>
        </div>
      </div>
    </section>

    <!-- ==================== 流式压测 ==================== -->
    <section class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-3">
        <div class="flex flex-wrap items-end gap-3">
          <label class="flex flex-col gap-1">
            <span class="text-xs text-base-content/60">速率（TPS）</span>
            <input v-model.number="stressTps" type="number" min="1" step="1" class="input input-bordered input-xs w-28" :disabled="stressRunning" />
          </label>
          <label class="flex flex-col gap-1">
            <span class="text-xs text-base-content/60">总量（字符）</span>
            <input v-model.number="stressTotal" type="number" min="1" step="1000" class="input input-bordered input-xs w-32" :disabled="stressRunning" />
          </label>
          <button type="button" class="btn btn-xs btn-primary" @click="stressRunning ? pauseStress() : startStress()">
            {{ stressRunning ? "暂停" : (stressState === "paused" ? "继续" : "开始") }}
          </button>
          <button type="button" class="btn btn-xs" @click="resetStress">重置</button>
          <span class="text-xs text-base-content/50">状态：{{ stressStateLabel }}</span>
        </div>

        <div class="grid grid-cols-2 gap-2 text-xs md:grid-cols-4">
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">已推 / 总量</div>
            <div class="font-mono" data-stress-metric="pushed">{{ stressPushedChars }} / {{ normalizedStressTotal }}</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">平均帧率</div>
            <div class="font-mono" data-stress-metric="fps">{{ stressAvgFps }}</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">最长单帧</div>
            <div class="font-mono" data-stress-metric="max-frame">{{ stressMaxFrameMs }} ms</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">已用时</div>
            <div class="font-mono" data-stress-metric="elapsed">{{ stressElapsedLabel }}</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">掉帧（&gt;50ms）</div>
            <div class="font-mono" data-stress-metric="jank">{{ stressJankFrames }}</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">实测吞吐</div>
            <div class="font-mono" data-stress-metric="throughput">{{ stressActualTps }} TPS</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">进度</div>
            <div class="font-mono" data-stress-metric="progress">{{ stressProgressPercent }}%</div>
          </div>
          <div class="rounded border border-base-300 px-2 py-1">
            <div class="text-base-content/50">收口</div>
            <div class="font-mono" data-stress-metric="state">{{ stressState }}</div>
          </div>
        </div>

        <div class="text-xs text-base-content/50">
          推的是真实增量路径：每帧往 `contentBlocks` 的正文里追加字符，再由真实的
          ChatMessageItem 重新派生、重新解析 markdown 渲染；不是推完再一次性显示。
          加速跑法：地址带 <code>?stress=auto</code> 时页面加载即开跑，供无头取数。
        </div>

        <OverlayScrollArea scroller-class="max-h-[26rem] pr-2">
          <div :style="{ width: `min(${demoWidth}px, 100%)` }">
            <ChatMessageItem
              v-if="stressBlock"
              v-bind="chatItemProps(stressBlock)"
              v-on="chatItemHandlers"
            />
          </div>
        </OverlayScrollArea>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, ref, type ShallowRef } from "vue";
import type { AssistantStreamBlock, ChatMessage, ChatMessageBlock } from "../../../types/app";
import ChatMessageItem from "./ChatMessageItem.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import { useChatMessageBlocks } from "../composables/use-chat-turns";

const demoWidth = ref(620);

const DEMO_CONVERSATION_ID = "demo-thinking-and-tools";

const AVATAR_LEADER = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><rect width='32' height='32' rx='16' fill='%237c9cd8'/></svg>";
const AVATAR_MEMBER = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><rect width='32' height='32' rx='16' fill='%23d8a07c'/></svg>";

const personaNameMap: Record<string, string> = {
  "agent-sandrone": "桑多涅",
  "agent-xiaoheita": "小黑塔",
  "system-persona": "pai system",
};

const personaAvatarUrlMap: Record<string, string> = {
  "agent-sandrone": AVATAR_LEADER,
  "agent-xiaoheita": AVATAR_MEMBER,
};

// ==================== 消息 → 展示块：走真实的派生链路 ====================

function useDemoBlocks(messageList: ShallowRef<ChatMessage[]>) {
  return useChatMessageBlocks({
    allMessages: messageList,
    activeChatApiConfig: computed(() => null),
    currentConversationId: computed(() => DEMO_CONVERSATION_ID),
    perfDebug: false,
    perfNow: () => performance.now(),
  });
}

function demoAssistantMessage(
  id: string,
  contentBlocks: AssistantStreamBlock[],
  meta: Record<string, unknown> = {},
): ChatMessage {
  return {
    id,
    role: "assistant",
    createdAt: new Date(Date.now() - 3 * 60 * 1000).toISOString(),
    speakerAgentId: "agent-sandrone",
    // 正文一律从 contentBlocks 取，parts 留一封同样内容的文本 part，与后端落盘形态一致。
    parts: [{ type: "text", text: "" }],
    contentBlocks,
    providerMeta: { dispatchElapsedMs: 12840, ...meta },
  };
}

// ---- 样本 1：跑完的一轮，长思维链 + 三种工具形态 + 正文 ----

const REASONING_LONG = [
  "先把边界量清楚再动手。用户在意的不是「有没有拦住」，而是「拦住之后那句话怎么落进会话里」。",
  "",
  "1. 先确认停止命令到底能不能知道当前会话是谁：`requested_conversation_id` 从 `StopChatRequest.session.conversationId` 来。",
  "2. 再看调度收口那条路能不能承担清标记：它要先过 `conversation_is_stopped` 的三条判定。",
  "3. 如果三条判定里任何一条不成立，`handle_conversation_stopped` 就不会执行，清理也就不会发生。",
  "",
  "**关键分歧**：用户点停止之后紧接着发新消息，这一批会先命中 `conversation_has_pending_queue_events` 分支，直接转去处理队列。",
  "也就是说收口能不能清，取决于用户停手之后有没有马上再发话——这不该是清理的判据。",
  "",
  "再往下拆，三条判定分别是：",
  "",
  "- 会话运行态必须是 `Idle`：`AssistantStreaming` 与 `OrganizingContext` 都不算停下",
  "- 待处理队列必须为空",
  "- 不能有还没消费的引导事件",
  "",
  "这三条同时成立才叫「真的停了」。用户刚点完停止的那一瞬间，运行态还在 `AssistantStreaming` 上，",
  "三条里第一条就不成立，所以那一刻收口这段根本不会走到。",
  "",
  "再想一层：清标记这件事本身不改停止流程的任何其他行为。工具句柄、流式内容落盘、会话运行态，一样都不能碰。",
  "所以落点只能在停止命令收尾那段，且必须挂在「真的打断到了东西」的分支里。",
  "",
  "还有一个反例要排除：迟到到达的停止请求。句柄表里已经没有可中止的对象，也没有请求号，`aborted` 为假。",
  "这时候成员那一轮其实已经跑完了，账该由正常路径去销，不能替它提前销账。",
  "",
  "顺带要确认新加的这一步不会把别的会话带进去：",
  "",
  "1. 只读这一次停止对应的那个会话的元数据，不扫会话列表",
  "2. 不是队员会话就直接返回，队长会话上没有 `team.membership`",
  "3. 本来就不欠汇报时也不写，避免用户每点一次停止就白改一次元数据",
  "",
  "最后是验证口径：不为测试造壳，读源码确认到接线这一档，跑编译与两条针对性用例。",
  "真机上点一次停止的手感留给用户验，我这边只保证静态与单测两档。",
  "",
  "结论：只在 `aborted == true` 时清，清的是这一次停止对应的那个会话自己的标记与锚点。",
].join("\n");

const SAMPLE_DONE_TEXT = [
  "结论先放这里：收口那条路靠不住，落点必须在停止命令里。",
  "",
  "改动落在三处，停止命令收尾只加一行：",
  "",
  "```rust",
  "let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;",
  "if aborted {",
  "    if let Some(conversation_id) = requested_conversation_id.as_deref() {",
  "        team_clear_member_pending_report_on_user_interrupt(state, conversation_id);",
  "    }",
  "}",
  "```",
  "",
  "| 时机 | 触发点 | 清标记后是否补发汇报 |",
  "| --- | --- | --- |",
  "| 队员已汇报 | `builtin_team_report` | 否（本人已发） |",
  "| 队长再次发话 | `team_mark_member_pending_report` | 否（换锚点重记） |",
  "| 自动转发完成 | `team_forward_member_stop_report` | 是（转发后清） |",
  "| 用户打断 | `stop_chat_message_inner` | 否（用户自己不要了） |",
  "",
  "失败态有独立通知（`team_notify_leader_member_failed`），所以打断与失败都不再补一封，避免队长一件事收到两封信。",
].join("\n");

const SAMPLE_DONE: ChatMessage = demoAssistantMessage("demo-done", [
  { reasoning: REASONING_LONG, tools: [], text: "" },
  {
    reasoning: "",
    tools: [
      {
        toolCallId: "call-get-session",
        name: "get_session",
        argsText: "",
        resultText: "当前会话：demo-thinking-and-tools，消息 42 条，最后活动 3 分钟前",
        status: "done",
      },
    ],
    text: "先确认这是哪个会话，再决定读哪一段。",
  },
  {
    reasoning: "",
    tools: [
      {
        toolCallId: "call-exec",
        name: "exec",
        argsText: "cd /repo/p-ai\nrg -n \"pending_report\" src-tauri/src -g '*.rs'\n",
        resultText: [
          "12 matches",
          "builtin_team.rs:720  fn team_mark_member_pending_report",
          "builtin_team.rs:752  fn team_clear_member_pending_report",
          "builtin_team.rs:781  fn team_clear_member_pending_report_on_user_interrupt",
          "core_commands.rs:2397  team_clear_member_pending_report_on_user_interrupt(state, conversation_id);",
        ].join("\n"),
        status: "done",
      },
      {
        toolCallId: "call-read",
        name: "read",
        argsText: [
          "{",
          "  \"absolute_path\": \"/repo/src-tauri/src/features/chat/model_runtime/tools_and_builtin/builtin_team.rs\",",
          "  \"offset\": 745,",
          "  \"limit\": 40",
          "}",
        ].join("\n"),
        resultText: [
          "/// 清「待汇报」标记。四个时机共用这一个入口：队员已汇报、队长再次发话、自动转发完成、",
          "/// 用户打断（见 `team_clear_member_pending_report_on_user_interrupt`）。",
          "fn team_clear_member_pending_report(state: &AppState, member_conversation_id: &str) {",
          "    let member_conversation_id = member_conversation_id.trim();",
          "    if member_conversation_id.is_empty() {",
          "        return;",
          "    }",
          "    let result = state_update_conversation_metadata_cached(state, member_conversation_id, |conversation| {",
          "        if let Some(membership) = conversation.team.membership.as_mut() {",
          "            membership.pending_report = false;",
          "            membership.pending_report_anchor = None;",
          "        }",
          "        Ok(())",
          "    });",
          "    if let Err(err) = result {",
          "        runtime_log_warn(format!(",
          "            \"[队伍] 失败，任务=清除待汇报标记，conversation_id={member_conversation_id}，error={err}\"",
          "        ));",
          "    }",
          "}",
        ].join("\n"),
        status: "done",
      },
      {
        toolCallId: "call-wait",
        name: "wait",
        argsText: "",
        status: "done",
      },
    ],
    text: SAMPLE_DONE_TEXT,
  },
]);

// ---- 样本 2：正在跑（executing_tool）----

const SAMPLE_RUNNING: ChatMessage = demoAssistantMessage(
  "demo-running",
  [
    {
      reasoning: [
        "先把类型检查跑一遍，再挑一条最相关的测试跑。",
        "",
        "- `pnpm typecheck` 覆盖 Vue 与 TypeScript",
        "- `cargo check` 单独跑，缓存已经热了",
        "",
        "两件互不依赖，可以一起发。",
      ].join("\n"),
      tools: [],
      text: "",
    },
    {
      reasoning: "",
      tools: [
        {
          toolCallId: "call-running",
          name: "exec",
          argsText: "cd /repo/p-ai\npnpm typecheck && cargo check --manifest-path src-tauri/Cargo.toml\n",
          status: "doing",
        },
      ],
      text: "",
    },
  ],
  { _streaming: true, _schedulingState: "executing_tool" },
);

// ---- 样本 3：失败中断（错误结果 + 正文截断）----

const SAMPLE_FAILED: ChatMessage = demoAssistantMessage(
  "demo-failed",
  [
    {
      reasoning: "先读那条 spec，确认它到底断言了什么再改实现。",
      tools: [],
      text: "",
    },
    {
      reasoning: "",
      tools: [
        {
          toolCallId: "call-error",
          name: "read",
          argsText: JSON.stringify(
            { absolute_path: "/repo/src/features/chat/components/playwright.missing.spec.ts" },
            null,
            2,
          ),
          resultText: "错误：文件不存在：/repo/src/features/chat/components/playwright.missing.spec.ts",
          status: "done",
        },
      ],
      text: "路径写错了，先把目标文件确认清楚再继续，这里先停在",
    },
  ],
  { dispatchElapsedMs: 4210 },
);

// ---- 样本 4：思维链超过 8000 字符，走纯文本降级 ----

const REASONING_HUGE = (() => {
  const paragraph = [
    "我在核对每一个调用点之间的先后关系：停止请求可能先于句柄登记到达，也可能晚于轮次收尾到达，",
    "这两种情况下后端能拿到的证据完全不同，前者只有请求号，后者什么都没有，第三种情况是句柄还在但身份对不上，",
    "这时候后端既不能猜也不能放过，只能把这一条原样放回表里，等下一次请求带上正确身份再来。",
  ].join("");
  const lines: string[] = [
    "这一条专门把思维链撑到 8000 字符以上：渲染层在超过这个长度时会自动降级成纯文本，",
    "避免几十万字符在正则解析和虚拟节点树上阻塞主线程。下面是逐段的核对过程。",
    "",
  ];
  for (let index = 1; index <= 60; index += 1) {
    lines.push(`${index}. ${paragraph}`);
    lines.push("");
  }
  return lines.join("\n");
})();

const SAMPLE_HUGE_REASONING: ChatMessage = demoAssistantMessage("demo-huge-reasoning", [
  { reasoning: REASONING_HUGE, tools: [], text: "核对完了，结论与上面那条样本一致。" },
]);

// ---- 样本 5：没有思维链，只有工具（即答 · 工具执行毕）----

const SAMPLE_TOOLS_ONLY: ChatMessage = demoAssistantMessage("demo-tools-only", [
  {
    reasoning: "",
    tools: [
      {
        toolCallId: "call-list",
        name: "exec",
        argsText: "ls /repo/src/features/chat/components | grep -i demo",
        resultText: [
          "ChatConversationItemDemo.vue",
          "ChatHomePanelDemo.vue",
          "ChatMessageItemDemo.vue",
          "ChatThinkingAndToolsDemo.vue",
        ].join("\n"),
        status: "done",
      },
    ],
    text: "",
  },
]);

const sampleMessages: ChatMessage[] = [
  SAMPLE_DONE,
  SAMPLE_RUNNING,
  SAMPLE_FAILED,
  SAMPLE_HUGE_REASONING,
  SAMPLE_TOOLS_ONLY,
];

const sampleCaptions: Record<string, string> = {
  "demo-done": "样本 1｜跑完的一轮：长思维链 → 无参数有结果的 get_session → 有参数有结果（多行 shell / 嵌套 JSON 参数 + 长路径）的 exec 与 read → 无参数无结果的 wait → 正文含代码块与表格。点开折叠块可看到三种工具形态；展开思维链条目后正文进限高滚动区（真实组件用 max-h-80，即 20rem / 320px）。",
  "demo-running": "样本 2｜正在跑：`_schedulingState=executing_tool` 且带一个 doing 工具，头部应报具体工具名（正在执行 exec），不是笼统的「正在执行工具」。",
  "demo-failed": "样本 3｜失败中断：工具结果是错误文本、正文停在半句、这一轮没有收口。错误不额外造样式——真实组件就是这么直出结果的。",
  "demo-huge-reasoning": "样本 4｜思维链超过 8000 字符：收起态裁四行、展开后整段渲染，看长文本下的排版与开合手感。",
  "demo-tools-only": "样本 5｜没有思维链、只有工具：折叠块头部应显示「即答 · 工具执行毕」，不带「已思考」字数。",
};

const { allMessageBlocks: sampleBlocks } = useDemoBlocks(shallowRef<ChatMessage[]>(sampleMessages));

const sampleViews = computed(() => {
  const blocks = sampleBlocks.value;
  return sampleMessages.map((message) => ({
    id: message.id,
    caption: sampleCaptions[message.id] || message.id,
    block: blocks.find((block) => block.sourceMessageId === message.id) || null,
  }));
});

// ==================== 流式压测 ====================

/** 压测用的正文语料：每段是一段完整的 markdown，重复拼接后仍然是合法文档。 */
const STRESS_CORPUS = [
  "## 这一段是压测语料\n\n",
  "正文里同时铺**加粗**、`行内代码`、以及一段引用：\n\n",
  "> 缩进与换行都要走一遍解析，不然压出来的只是纯文本拼接的速度。\n\n",
  "- 无序列表第一项\n",
  "- 无序列表第二项，带 `code` 片段\n",
  "  - 嵌套一层\n",
  "  - 再嵌套一层\n\n",
  "1. 有序列表第一项\n",
  "2. 有序列表第二项\n\n",
  "```rust\nfn tick(now: f64) -> f64 {\n    now * 1.5\n}\n```\n\n",
  "| 指标 | 含义 |\n| --- | --- |\n| 帧间隔 | 两个 rAF 回调之间的真实间隔 |\n| 最长单帧 | 一轮里最大的那个间隔 |\n\n",
  "段落之间要留空行，这样 markdown 才会把它切成两个段落，而不是揉成一行继续往下排。",
  "这一句后面没有换行，用来压长段落的折行计算。\n\n",
].join("");

const DEFAULT_TPS = 300;
const DEFAULT_TOTAL = 200_000;

const stressTps = ref(DEFAULT_TPS);
const stressTotal = ref(DEFAULT_TOTAL);
const stressRunning = ref(false);
const stressState = ref<"idle" | "running" | "paused" | "done">("idle");
const stressPushedChars = ref(0);
const stressFrameCount = ref(0);
const stressMaxFrameMs = ref(0);
const stressJankFrames = ref(0);
const stressProgressPercent = ref(0);
const stressElapsedMs = ref(0);
const stressActiveMs = ref(0);

const normalizedStressTotal = computed(() => {
  const total = Number(stressTotal.value);
  if (!Number.isFinite(total) || total <= 0) return DEFAULT_TOTAL;
  return Math.floor(total);
});

const normalizedStressTps = computed(() => {
  const tps = Number(stressTps.value);
  if (!Number.isFinite(tps) || tps <= 0) return DEFAULT_TPS;
  return tps;
});

const stressStateLabel = computed(() => {
  switch (stressState.value) {
    case "running":
      return "正在推";
    case "paused":
      return "已暂停";
    case "done":
      return "已推满";
    default:
      return "未开始";
  }
});

const stressAvgFps = computed(() => {
  const ms = stressActiveMs.value;
  if (ms <= 0 || stressFrameCount.value <= 0) return "—";
  return (stressFrameCount.value / (ms / 1000)).toFixed(1);
});

const stressActualTps = computed(() => {
  const ms = stressActiveMs.value;
  if (ms <= 0) return "—";
  return Math.round(stressPushedChars.value / (ms / 1000));
});

const stressElapsedLabel = computed(() => {
  const ms = stressElapsedMs.value;
  if (ms <= 0) return "0.0s";
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = ((ms % 60_000) / 1000).toFixed(1);
  return `${minutes}m${seconds}s`;
});

// 压测状态：一条正在流式的消息，每帧只往 contentBlocks 的正文里追加字符，
// 派生（useChatMessageBlocks）与渲染（ChatMessageItem）全部走真实路径。
const stressMessages = shallowRef<ChatMessage[]>([
  demoAssistantMessage("demo-stress", [{ reasoning: "", tools: [], text: "" }], {
    _streaming: true,
    _schedulingState: "streaming_text",
  }),
]);

const { allMessageBlocks: stressBlocks } = useDemoBlocks(stressMessages);
const stressBlock = computed<ChatMessageBlock | null>(() => stressBlocks.value[0] || null);

let stressSource = "";
let stressRafId = 0;
let stressSegmentStartedAt = 0;
let stressBaseElapsedMs = 0;
let stressLastFrameAt = 0;

function buildStressSource(total: number): string {
  if (total <= 0) return "";
  const repeatCount = Math.ceil(total / STRESS_CORPUS.length) + 1;
  return STRESS_CORPUS.repeat(repeatCount);
}

function writeStressText(text: string, streaming: boolean) {
  const base = stressMessages.value[0];
  stressMessages.value = [
    {
      ...base,
      contentBlocks: [{ reasoning: "", tools: [], text }],
      providerMeta: {
        ...(base.providerMeta || {}),
        _streaming: streaming,
        _schedulingState: streaming ? "streaming_text" : "idle",
      },
    },
  ];
}

function stressTick() {
  if (!stressRunning.value) return;
  // 时间表按墙钟走：rAF 的时间戳在重负载下会被渲染队列拖慢，
  // 拿它排表等于让帧钟自己证明自己「跑满了 300 TPS」，读数不成立。
  const now = performance.now();
  const elapsedMs = stressBaseElapsedMs + (now - stressSegmentStartedAt);
  stressElapsedMs.value = elapsedMs;
  if (stressLastFrameAt > 0) {
    const frameMs = now - stressLastFrameAt;
    stressActiveMs.value += frameMs;
    stressFrameCount.value += 1;
    if (frameMs > stressMaxFrameMs.value) stressMaxFrameMs.value = frameMs;
    if (frameMs > 50) stressJankFrames.value += 1;
  }
  stressLastFrameAt = now;

  const total = normalizedStressTotal.value;
  // 这一帧该推到哪个字符，而不是「每帧固定推几个」：
  // 帧被拖长时一次补上欠下的量，吞吐才等于设定的 TPS，卡顿只体现在单帧耗时上。
  const dueChars = Math.min(total, Math.floor((elapsedMs / 1000) * normalizedStressTps.value));
  if (dueChars > stressPushedChars.value) {
    stressPushedChars.value = dueChars;
    writeStressText(stressSource.slice(0, dueChars), dueChars < total);
  }
  stressProgressPercent.value = total > 0
    ? Math.min(100, Math.round((stressPushedChars.value / total) * 1000) / 10)
    : 0;

  if (stressPushedChars.value >= total) {
    finishStress();
    return;
  }
  stressRafId = window.requestAnimationFrame(stressTick);
}

function startStress() {
  if (stressRunning.value) return;
  const total = normalizedStressTotal.value;
  if (stressPushedChars.value >= total) {
    resetStress();
  }
  if (!stressSource || stressSource.length < total) {
    stressSource = buildStressSource(total);
  }
  stressRunning.value = true;
  stressState.value = "running";
  stressSegmentStartedAt = performance.now();
  stressBaseElapsedMs = stressElapsedMs.value;
  stressLastFrameAt = 0;
  if (stressPushedChars.value === 0) {
    writeStressText("", true);
  }
  stressRafId = window.requestAnimationFrame(stressTick);
}

function pauseStress() {
  if (!stressRunning.value) return;
  stressRunning.value = false;
  stressState.value = "paused";
  if (stressRafId) {
    window.cancelAnimationFrame(stressRafId);
    stressRafId = 0;
  }
  // 暂停不计入用时，也不计入帧统计。
  stressLastFrameAt = 0;
}

function finishStress() {
  stressRunning.value = false;
  stressState.value = "done";
  if (stressRafId) {
    window.cancelAnimationFrame(stressRafId);
    stressRafId = 0;
  }
  stressLastFrameAt = 0;
  writeStressText(stressSource.slice(0, stressPushedChars.value), false);
}

function resetStress() {
  stressRunning.value = false;
  if (stressRafId) {
    window.cancelAnimationFrame(stressRafId);
    stressRafId = 0;
  }
  stressState.value = "idle";
  stressPushedChars.value = 0;
  stressFrameCount.value = 0;
  stressMaxFrameMs.value = 0;
  stressJankFrames.value = 0;
  stressProgressPercent.value = 0;
  stressElapsedMs.value = 0;
  stressActiveMs.value = 0;
  stressBaseElapsedMs = 0;
  stressSegmentStartedAt = 0;
  stressLastFrameAt = 0;
  stressSource = "";
  writeStressText("", true);
}

onMounted(() => {
  // 无头取数用：地址带 ?stress=auto 时页面加载就按默认档（300 TPS / 200K）开跑。
  const params = new URLSearchParams(window.location.search);
  if (params.get("stress") === "auto") {
    window.setTimeout(() => startStress(), 600);
  }
});

onBeforeUnmount(() => {
  if (stressRafId) window.cancelAnimationFrame(stressRafId);
});

// ==================== 真实组件的 props / 事件 ====================

const noop = () => {};

function chatItemProps(block: ChatMessageBlock) {
  return {
    activeConversationId: DEMO_CONVERSATION_ID,
    block,
    selectionKey: block.id,
    selectionModeEnabled: false,
    selected: false,
    chatting: false,
    busy: false,
    frozen: false,
    userAlias: "红豆",
    userAvatarUrl: "",
    personaNameMap,
    personaAvatarUrlMap,
    markdownIsDark: false,
    playingAudioId: "",
    activeTurnUser: false,
    compactWithPrevious: false,
    canRegenerate: false,
    canConfirmPlan: false,
  };
}

const chatItemHandlers = {
  assistantLinkClick: noop,
  openImagePreview: noop,
  copyMessage: noop,
  copyMessageImageDone: noop,
  copyMessageImageFailed: noop,
  toggleAudioPlayback: noop,
  recallTurn: noop,
  createConversationBranchFromTurn: noop,
  regenerateTurn: noop,
  confirmPlan: noop,
  enterSelectionMode: noop,
  toggleMessageSelected: noop,
  activityToggle: noop,
};
</script>
