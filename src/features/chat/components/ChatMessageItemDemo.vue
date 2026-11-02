<template>
  <div class="relative flex h-[900px] flex-col overflow-hidden rounded-box border border-base-300 bg-base-100">
    <div class="flex flex-wrap items-center gap-2 border-b border-base-300 px-3 py-2">
      <span class="text-xs text-base-content/60">宽度</span>
      <input v-model.number="demoWidth" type="range" min="320" max="720" step="20" class="range range-xs w-28" />
      <span class="text-xs text-base-content/50">{{ demoWidth }}px</span>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
      <div :style="{ width: `min(${demoWidth}px, 100%)` }">
        <ChatMessageItem
          v-for="item in blocks"
          :key="item.id"
          active-conversation-id="demo-conversation"
          :block="item"
          :selection-key="item.id"
          :selection-mode-enabled="false"
          :selected="false"
          :chatting="false"
          :busy="false"
          :frozen="false"
          user-alias="红豆"
          user-avatar-url=""
          :persona-name-map="personaNameMap"
          :persona-avatar-url-map="personaAvatarUrlMap"
          :markdown-is-dark="false"
          playing-audio-id=""
          :active-turn-user="false"
          :compact-with-previous="false"
          :can-regenerate="false"
          :can-confirm-plan="false"
          @assistant-link-click="() => {}"
          @open-image-preview="() => {}"
          @copy-message="() => {}"
          @copy-message-image-done="() => {}"
          @copy-message-image-failed="() => {}"
          @toggle-audio-playback="() => {}"
          @recall-turn="() => {}"
          @create-conversation-branch-from-turn="() => {}"
          @regenerate-turn="() => {}"
          @confirm-plan="() => {}"
          @enter-selection-mode="() => {}"
          @toggle-message-selected="() => {}"
          @activity-toggle="() => {}"
          @reasoning-stuck-change="() => {}"
        />
      </div>
    </div>

    <div class="border-t border-base-300 px-3 py-2 text-xs text-base-content/60">
      前五条都是同一种 ChatMessageItem：普通 assistant 消息、队伍邮件（服务端默认形态，收起）、
      同一封邮件点开后、队伍通知、队员停下后自动投回来的单行产出。
      队伍邮件喂的是服务端真实拼出来的 markdown——标题进 `&lt;summary&gt;`，状态行在折叠体内，
      渲染层不做任何特殊处理，气泡形态与第一条完全一致；单行产出没有邮件头也不折叠，
      正文就是那一行原文。署名与头像靠 speakerAgentId 查 personaNameMap；
      通知喂的是后端内置系统人格的默认名，查不到会退回裸 id，所以这个映射必须喂全。
      后两条是工具状态专项：第六条在 executing_tool 阶段带一个 doing 工具，头部应报具体工具名；
      第七条把三种工具并列——无参数有结果（get_session）、有参数有结果（read）、无参数无结果（wait），
      点开活动面板后：前两条各带结果，get_session 没有参数方块；wait 连箭头都没有。
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { ChatMessageBlock } from "../../../types/app";
import ChatMessageItem from "./ChatMessageItem.vue";

const demoWidth = ref(560);

const AVATAR_LEADER = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><rect width='32' height='32' rx='16' fill='%237c9cd8'/></svg>";
const AVATAR_MEMBER = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><rect width='32' height='32' rx='16' fill='%23d8a07c'/></svg>";

const personaNameMap: Record<string, string> = {
  "agent-sandrone": "桑多涅",
  "agent-xiaoheita": "小黑塔",
  // 后端内置系统人格的默认名（default_system_persona().name）；
  // 真实环境里取的是那份 agents.json 里存的名字，这里只是把默认值喂全
  "system-persona": "pai system",
};

const personaAvatarUrlMap: Record<string, string> = {
  "agent-sandrone": AVATAR_LEADER,
  "agent-xiaoheita": AVATAR_MEMBER,
};

function baseBlock(overrides: Partial<ChatMessageBlock>): ChatMessageBlock {
  return {
    id: `demo-${overrides.id || "block"}`,
    role: "assistant",
    speakerAgentId: "agent-sandrone",
    createdAt: new Date(Date.now() - 3 * 60 * 1000).toISOString(),
    text: "",
    images: [],
    audios: [],
    attachmentFiles: [],
    toolCallCount: 0,
    lastToolName: "",
    toolCalls: [],
    activityItems: [],
    activityReasoningCharCount: 0,
    activityToolCountsByName: {},
    activityRunning: false,
    activityStatus: "idle",
    ...overrides,
  };
}

const MAIL_TITLE = "把登录鉴权重做成 session 表";
const MAIL_STATE_LINE = "（队长把这条线的状态设为：进行中）";

const blocks = computed<ChatMessageBlock[]>(() => [
  // 对照组：同一个人格的普通消息，用来确认队伍消息没有多出任何独有的头部件
  baseBlock({
    id: "plain",
    text: "先看一遍现有中间件被哪些路由引用，再决定动哪几个。老的中间件先留着，等新链路跑通再删。",
  }),
  // 队伍邮件：服务端拼好的 markdown，原样渲染——标题进 <summary>，正文在折叠体内，默认收起
  baseBlock({
    id: "mail",
    providerMeta: {
      messageKind: "team_message",
      teamMessage: true,
      teamMailTitle: MAIL_TITLE,
    },
    text: `<details>\n<summary>${MAIL_TITLE}</summary>\n${MAIL_STATE_LINE}\n\n先看一遍现有中间件被哪些路由引用，再决定动哪几个。老的中间件先留着，等新链路跑通再删。\n</details>`,
  }),
  // 同一封邮件点开后的样子：正文在折叠体内照样按 markdown 渲染
  baseBlock({
    id: "mail-open",
    providerMeta: {
      messageKind: "team_message",
      teamMessage: true,
      teamMailTitle: MAIL_TITLE,
    },
    text: `<details open>\n<summary>${MAIL_TITLE}</summary>\n${MAIL_STATE_LINE}\n\n先看一遍现有中间件被哪些路由引用，再决定动哪几个。\n\n1. 列出引用点\n2. 老的先留着，新链路跑通再删\n</details>`,
  }),
  // 队伍通知：没有邮件头，只有一段正文
  baseBlock({
    id: "notice",
    speakerAgentId: "system-persona",
    providerMeta: { messageKind: "team_notice", teamMessage: true },
    text: "你已经完成了你的职责，队长决定把你移出队伍。此后你是一个普通的追问会话，不用再等队长的指示。",
  }),
  // 队员停下后自动投给队长的单行产出：没有邮件头、不折叠，正文就是那一行原文。
  // 折叠的意义是「标题在外、内容点开看」，一行字折起来只会把同一句话写两遍。
  baseBlock({
    id: "mail-single-line",
    providerMeta: { messageKind: "team_message", teamMessage: true },
    text: "收到，李工在线，直接发言通道正常。",
  }),
  // 卡片头：工具执行中。后端把 _schedulingState 留在 executing_tool，
  // 头部应报到具体工具名（这里应是「正在执行：阅读 playwright.spec.ts」），不是笼统的「正在执行工具」
  baseBlock({
    id: "tool-doing",
    isStreaming: true,
    providerMeta: { _schedulingState: "executing_tool" },
    toolCallCount: 1,
    lastToolName: "read",
    toolCalls: [{
      name: "read",
      argsText: JSON.stringify({ absolute_path: "/repo/src/playwright.spec.ts" }),
      status: "doing",
    }],
    activityItems: [{
      kind: "tool",
      id: "tool-doing-1",
      toolCallId: "call_doing",
      name: "read",
      argsText: JSON.stringify({ absolute_path: "/repo/src/playwright.spec.ts" }),
      status: "doing",
    }],
    // 展开态不读 activityItems，改从 contentBlocks 重新派生，喂同一份数据保证两态一致
    contentBlocks: [{
      reasoning: "",
      text: "",
      tools: [{
        toolCallId: "call_doing",
        name: "read",
        argsText: JSON.stringify({ absolute_path: "/repo/src/playwright.spec.ts" }),
        status: "doing",
      }],
    }],
    activityRunning: true,
    activityStatus: "running_tool",
  }),
  // 参数区对照：无参数工具不显示参数区（也不给箭头），有参数工具照旧。
  // 这里同时点开两条，确认无参数的只露结果、有参数的照旧是「参数 --- 结果」
  baseBlock({
    id: "tool-args-compare",
    text: "先查会话，再读文件。",
    toolCallCount: 2,
    lastToolName: "read",
    toolCalls: [
      { name: "get_session", argsText: "", status: "done" },
      { name: "read", argsText: JSON.stringify({ absolute_path: "/repo/src/a.ts" }), status: "done" },
    ],
    activityItems: [
      { kind: "tool", id: "compare-1", toolCallId: "call_noargs", name: "get_session", argsText: "", resultText: "共 3 个会话", status: "done" },
      { kind: "tool", id: "compare-2", toolCallId: "call_args", name: "read", argsText: JSON.stringify({ absolute_path: "/repo/src/a.ts" }), resultText: "文件内容", status: "done" },
      { kind: "tool", id: "compare-3", toolCallId: "call_empty", name: "wait", argsText: "", status: "done" },
    ],
    contentBlocks: [{
      reasoning: "",
      text: "",
      tools: [
        { toolCallId: "call_noargs", name: "get_session", argsText: "", resultText: "共 3 个会话", status: "done" },
        { toolCallId: "call_args", name: "read", argsText: JSON.stringify({ absolute_path: "/repo/src/a.ts" }), resultText: "文件内容", status: "done" },
        { toolCallId: "call_empty", name: "wait", argsText: "", status: "done" },
      ],
    }],
  }),
]);
</script>
