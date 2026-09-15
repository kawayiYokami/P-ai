<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">视图：</span>
      <button
        type="button"
        class="btn btn-xs"
        :class="viewMode === 'composite' ? 'btn-primary' : 'btn-ghost'"
        @click="viewMode = 'composite'"
      >
        综合
      </button>
      <button
        type="button"
        class="btn btn-xs"
        :class="viewMode === 'compact' ? 'btn-primary' : 'btn-ghost'"
        @click="viewMode = 'compact'"
      >
        精简
      </button>

      <span class="ml-3 text-xs text-base-content/60">预设：</span>
      <button
        v-for="preset in presetOptions"
        :key="preset.key"
        type="button"
        class="btn btn-xs"
        :class="activePreset === preset.key ? 'btn-primary' : 'btn-ghost'"
        @click="activePreset = preset.key"
      >
        {{ preset.label }}
      </button>
    </div>

    <p class="text-xs text-base-content/50">
      综合模式按 mock 指定的层级渲染（full / sim / mini 混合）；精简模式全部转简单条目且左侧收窄。点击条目不会产生副作用。
    </p>

    <div class="flex gap-3">
      <div class="flex w-[360px] max-w-full flex-col overflow-hidden rounded-box border border-base-300 bg-base-200 pb-2">
        <template v-if="sections.length > 0">
          <div v-for="section in sections" :key="section.key">
            <div class="flex items-center gap-1.5 px-3 pb-1 pt-2 text-xs font-semibold text-base-content/50">
              <span class="truncate">{{ section.title }}</span>
              <span class="shrink-0 text-base-content/35">{{ section.entries.length }}</span>
            </div>
            <ChatConversationItem
              v-for="entry in section.entries"
              :key="entry.item.conversationId"
              :item="entry.item"
              :level="resolveLevel(entry)"
              :active-conversation-id="ACTIVE_CONVERSATION_ID"
              :user-alias="USER_ALIAS"
              user-avatar-url=""
              :persona-name-map="PERSONA_NAME_MAP"
              :persona-avatar-url-map="PERSONA_AVATAR_URL_MAP"
              :pipeline-status-by-id="PIPELINE_STATUS_BY_ID"
              :compact-indicator="isCompact"
              :show-source-badge="section.key === 'recent'"
            />
          </div>
        </template>
        <div v-else class="px-3 py-6 text-center text-sm text-base-content/50">
          没有匹配的会话
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import { simpleConversationItemLevel, type ConversationItemLevel } from "../utils/conversation-item-display";
import ChatConversationItem from "./ChatConversationItem.vue";

type DemoEntry = {
  item: ChatConversationOverviewItem;
  /** 综合模式下使用的层级；精简模式忽略，统一走 simpleConversationItemLevel */
  level: ConversationItemLevel;
};

type DemoSection = {
  key: string;
  title: string;
  entries: DemoEntry[];
};

type PresetKey = "normal" | "allStates" | "empty";

const ACTIVE_CONVERSATION_ID = "demo-active";
const USER_ALIAS = "红豆";
const PERSONA_NAME_MAP: Record<string, string> = {
  "persona-nahida": "纳西妲",
  "system-persona": "P-ai系统",
};
const PERSONA_AVATAR_URL_MAP: Record<string, string> = {};
const PIPELINE_STATUS_BY_ID: Record<string, ConversationPipelineStatus> = {
  "demo-streaming": "busy",
  "demo-error": "error",
};

const viewMode = ref<"composite" | "compact">("composite");
const activePreset = ref<PresetKey>("normal");
const isCompact = computed(() => viewMode.value === "compact");

const presetOptions: Array<{ key: PresetKey; label: string }> = [
  { key: "normal", label: "常规" },
  { key: "allStates", label: "全状态" },
  { key: "empty", label: "空" },
];

function minutesAgo(minutes: number): string {
  return new Date(Date.now() - minutes * 60_000).toISOString();
}

function preview(role: ConversationPreviewMessage["role"], textPreview: string, speakerAgentId?: string): ConversationPreviewMessage {
  return {
    messageId: `demo-preview-${Math.random().toString(36).slice(2, 8)}`,
    role,
    speakerAgentId,
    textPreview,
    createdAt: minutesAgo(1),
  };
}

function makeItem(overrides: Partial<ChatConversationOverviewItem> & { conversationId: string }): ChatConversationOverviewItem {
  return {
    title: "未命名会话",
    kind: "local_unarchived",
    messageCount: 4,
    unreadCount: 0,
    isPinned: false,
    isDraft: false,
    updatedAt: minutesAgo(30),
    lastMessageAt: minutesAgo(30),
    previewMessages: [preview("assistant", "这是演示用的会话摘要。", "persona-nahida")],
    ...overrides,
  } as ChatConversationOverviewItem;
}

const normalSections: DemoSection[] = [
  {
    key: "pinned",
    title: "置顶",
    entries: [
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-pinned",
          title: "PAI 发布流程核对清单",
          isPinned: true,
          workspaceRootPath: "E:/github/easy_call_ai",
          workspaceLabel: "easy_call_ai",
          updatedAt: minutesAgo(180),
          lastMessageAt: minutesAgo(180),
          previewMessages: [preview("user", "把发布前要跑的检查列一遍", "user-persona")],
        }),
      },
    ],
  },
  {
    key: "recent",
    title: "最近会话",
    entries: [
      {
        level: "full",
        item: makeItem({
          conversationId: ACTIVE_CONVERSATION_ID,
          title: "左栏切换与搜索入口改造",
          workspaceRootPath: "E:/github/easy_call_ai",
          workspaceLabel: "easy_call_ai",
          updatedAt: minutesAgo(2),
          lastMessageAt: minutesAgo(2),
          previewMessages: [preview("assistant", "分段控件的滑动指示块已经加上。", "persona-nahida")],
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-unread",
          title: "排查后台任务页加载失败",
          unreadCount: 3,
          updatedAt: minutesAgo(45),
          lastMessageAt: minutesAgo(45),
          previewMessages: [preview("assistant", "我看下 monitor.changed 的触发链路。", "persona-nahida")],
        }),
      },
      {
        level: "sim",
        item: makeItem({
          conversationId: "demo-sim",
          title: "同人格旧会话（简单条目）",
          updatedAt: minutesAgo(90),
          lastMessageAt: minutesAgo(90),
          previewMessages: [preview("user", "继续上面的问题", "user-persona")],
        }),
      },
    ],
  },
  {
    key: "workspace:easy_call_ai",
    title: "easy_call_ai",
    entries: [
      {
        level: "sim",
        item: makeItem({
          conversationId: "demo-ws-1",
          title: "梳理 Changelog 生成流程",
          workspaceRootPath: "E:/github/easy_call_ai",
          workspaceLabel: "easy_call_ai",
          updatedAt: minutesAgo(240),
          lastMessageAt: minutesAgo(240),
        }),
      },
      {
        level: "mini",
        item: makeItem({
          conversationId: "demo-ws-2",
          title: "清理未使用的组件导入",
          workspaceRootPath: "E:/github/easy_call_ai",
          workspaceLabel: "easy_call_ai",
          updatedAt: minutesAgo(60 * 24 * 20),
          lastMessageAt: minutesAgo(60 * 24 * 20),
          previewMessages: [],
        }),
      },
    ],
  },
  {
    key: "workspace:docs",
    title: "文档仓库",
    entries: [
      {
        level: "sim",
        item: makeItem({
          conversationId: "demo-ws-3",
          title: "整理萌娘百科缓存页",
          workspaceRootPath: "E:/github/paimonhome",
          workspaceLabel: "文档仓库",
          updatedAt: minutesAgo(60 * 30),
          lastMessageAt: minutesAgo(60 * 30),
        }),
      },
    ],
  },
];

const allStateSections: DemoSection[] = [
  {
    key: "recent",
    title: "会话状态总览",
    entries: [
      {
        level: "full",
        item: makeItem({
          conversationId: ACTIVE_CONVERSATION_ID,
          title: "当前选中（active）",
          updatedAt: minutesAgo(1),
          lastMessageAt: minutesAgo(1),
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-unread-2",
          title: "有未读（unread）",
          unreadCount: 12,
          updatedAt: minutesAgo(8),
          lastMessageAt: minutesAgo(8),
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-streaming",
          title: "流式中（assistant_streaming）",
          runtimeState: "assistant_streaming",
          updatedAt: minutesAgo(3),
          lastMessageAt: minutesAgo(3),
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-error",
          title: "管线失败（pipeline error）",
          updatedAt: minutesAgo(20),
          lastMessageAt: minutesAgo(20),
          previewMessages: [preview("tool", "命令执行失败：exit code 1")],
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-system",
          title: "系统通知会话",
          isSystemNotificationConversation: true,
          workspaceLabel: "P-ai系统",
          updatedAt: minutesAgo(120),
          lastMessageAt: minutesAgo(120),
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-draft",
          title: "会话草稿（draft）",
          isDraft: true,
          updatedAt: minutesAgo(0),
          lastMessageAt: minutesAgo(0),
          previewMessages: [],
        }),
      },
      {
        level: "full",
        item: makeItem({
          conversationId: "demo-remote",
          title: "远程联系人会话",
          kind: "remote_im_contact",
          channelId: "channel-demo",
          channelName: "研发频道",
          updatedAt: minutesAgo(15),
          lastMessageAt: minutesAgo(15),
          previewMessages: [preview("assistant", "这边已经收到，稍后同步进度。", "persona-nahida")],
        }),
      },
      {
        level: "mini",
        item: makeItem({
          conversationId: "demo-stale",
          title: "很久未用（mini，摘要折叠）",
          updatedAt: minutesAgo(60 * 24 * 40),
          lastMessageAt: minutesAgo(60 * 24 * 40),
          previewMessages: [],
        }),
      },
    ],
  },
];

const sections = computed<DemoSection[]>(() => {
  if (activePreset.value === "empty") return [];
  return activePreset.value === "allStates" ? allStateSections : normalSections;
});

function resolveLevel(entry: DemoEntry): ConversationItemLevel {
  if (isCompact.value) return simpleConversationItemLevel(entry.item);
  return entry.level;
}
</script>
