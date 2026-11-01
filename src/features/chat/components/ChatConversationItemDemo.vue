<template>
  <div class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100">
    <!-- 控制项：只改渲染条件，不改数据 -->
    <div class="flex flex-wrap items-center gap-3 border-b border-base-300 px-4 py-2 text-xs">
      <label class="flex items-center gap-1.5">
        <span class="text-base-content/60">行宽</span>
        <select v-model.number="rowWidth" class="select select-bordered select-xs">
          <option :value="360">360（画廊默认）</option>
          <option :value="292">292（真实侧栏）</option>
          <option :value="260">260（极窄）</option>
        </select>
      </label>
      <label class="flex items-center gap-1.5">
        <span class="text-base-content/60">形态</span>
        <select v-model="shape" class="select select-bordered select-xs">
          <option value="composite">综合（full / mini）</option>
          <option value="compact">精简（自动分层）</option>
        </select>
      </label>
      <label class="flex items-center gap-1.5">
        <input v-model="showSourceBadge" type="checkbox" class="checkbox checkbox-xs" />
        <span class="text-base-content/60">来源徽章</span>
      </label>
    </div>

    <div class="p-3">
      <div
        class="flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-200 pb-2"
        :style="{ width: `${rowWidth}px` }"
      >
        <template v-for="group in groups" :key="group.key">
          <div class="flex items-center gap-1.5 px-3 pb-1 pt-2 text-xs font-semibold text-base-content/50">
            <span class="truncate">{{ group.title }}</span>
            <span class="shrink-0 text-base-content/35">{{ group.rows.length }}</span>
          </div>
          <div v-for="row in group.rows" :key="row.item.conversationId" :class="row.hook">
            <ChatConversationItem
              :item="row.item"
              :level="resolveLevel(row)"
              :active-conversation-id="ACTIVE_CONVERSATION_ID"
              :user-alias="USER_ALIAS"
              user-avatar-url=""
              :persona-name-map="PERSONA_NAME_MAP"
              :persona-avatar-url-map="PERSONA_AVATAR_URL_MAP"
              :pipeline-status-by-id="EMPTY_PIPELINE_STATUS"
              :compact-indicator="shape === 'compact'"
              :show-source-badge="showSourceBadge"
            />
          </div>
        </template>
      </div>
    </div>

    <p class="border-t border-base-300 px-4 py-2 text-xs leading-5 text-base-content/60">
      喂的是 mock 数据：会话标题、摘要、未读数、进度状态、待定问题数都由本页给定，点击条目、右键菜单、改名、归档等交互一律被丢弃，不写库。
      上半组是形态：无状态有未读 / 无状态无未读 / 有状态有未读 / 阻塞 + 99 / 完成 / 待办 / 当前选中会话 / mini 行。
      中间组是待定问题数：只给待定、待定 + 未读、三件同框、待定 0（不渲染）、100 显示 99+。
      下半组是撞色对照：生产版（间距 12 + 未读一圈与行底色同值的缝）、只拉间距、只加缝、缝改用 base-100（白）。
      除第一行外，其余对照样式都只在本页生效。
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import { simpleConversationItemLevel, type ConversationItemLevel } from "../utils/conversation-item-display";
import ChatConversationItem from "./ChatConversationItem.vue";

type DemoRow = {
  item: ChatConversationOverviewItem;
  /** 综合模式下使用的层级；精简模式忽略，统一走 simpleConversationItemLevel */
  level: ConversationItemLevel;
  /** 只在本页生效的对照样式类 */
  hook?: string;
};

const ACTIVE_CONVERSATION_ID = "demo-item-active";
const USER_ALIAS = "红豆";
const PERSONA_NAME_MAP: Record<string, string> = { "persona-nahida": "纳西妲" };
const PERSONA_AVATAR_URL_MAP: Record<string, string> = {};
const EMPTY_PIPELINE_STATUS: Record<string, ConversationPipelineStatus> = {};

const rowWidth = ref(360);
const shape = ref<"composite" | "compact">("composite");
const showSourceBadge = ref(true);

function minutesAgo(minutes: number): string {
  return new Date(Date.now() - minutes * 60_000).toISOString();
}

function preview(textPreview: string): ConversationPreviewMessage {
  return {
    messageId: `demo-item-preview-${Math.random().toString(36).slice(2, 8)}`,
    role: "assistant",
    speakerAgentId: "persona-nahida",
    textPreview,
    createdAt: minutesAgo(1),
  };
}

function makeItem(overrides: Partial<ChatConversationOverviewItem> & { conversationId: string }): ChatConversationOverviewItem {
  return {
    title: "未命名会话",
    kind: "local_unarchived",
    conversationKind: "chat",
    messageCount: 6,
    unreadCount: 0,
    isPinned: false,
    isDraft: false,
    updatedAt: minutesAgo(30),
    lastMessageAt: minutesAgo(30),
    previewMessages: [preview("这是演示用的会话摘要。")],
    ...overrides,
  } as ChatConversationOverviewItem;
}

const rows: DemoRow[] = [
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-unset-unread",
      title: "无状态 + 有未读",
      unreadCount: 3,
      updatedAt: minutesAgo(45),
      lastMessageAt: minutesAgo(45),
      previewMessages: [preview("我看下 monitor.changed 的触发链路。")],
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-unset-read",
      title: "无状态 + 无未读",
      updatedAt: minutesAgo(40),
      lastMessageAt: minutesAgo(40),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-doing-unread-divider",
      title: "有状态 + 有未读（进行）",
      progressState: "in_progress",
      evidence: "已开始改颜色映射与三处落点",
      unreadCount: 12,
      updatedAt: minutesAgo(3),
      lastMessageAt: minutesAgo(3),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-blocked-unread",
      title: "有状态 + 有未读（阻塞）",
      progressState: "blocked",
      evidence: "缺设计稿确认，接口字段也还没定",
      unreadCount: 99,
      updatedAt: minutesAgo(6),
      lastMessageAt: minutesAgo(6),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-done-read",
      title: "有状态 + 无未读（完成）",
      progressState: "done",
      evidence: "用户说「可以了，发吧」",
      updatedAt: minutesAgo(30),
      lastMessageAt: minutesAgo(30),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-item-todo-read",
      title: "有状态 + 无未读（待办）",
      progressState: "todo",
      unreadCount: 0,
      updatedAt: minutesAgo(20),
      lastMessageAt: minutesAgo(20),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: ACTIVE_CONVERSATION_ID,
      title: "当前选中会话（未读恒不显示）",
      progressState: "in_review",
      evidence: "改动已提交，等你过一遍",
      // 当前会话恒不显示未读，这里故意给值，验证它不会漏出来
      unreadCount: 2,
      updatedAt: minutesAgo(1),
      lastMessageAt: minutesAgo(1),
    }),
  },
  {
    level: "mini",
    item: makeItem({
      conversationId: "demo-item-mini",
      title: "mini 行（没有第二行，不显示状态）",
      progressState: "blocked",
      evidence: "这条证据不该出现在 mini 行上",
      updatedAt: minutesAgo(60 * 24 * 30),
      lastMessageAt: minutesAgo(60 * 24 * 30),
      previewMessages: [],
    }),
  },
];

// 阻塞红与未读红撞色：同一行两块红只隔一个 gap 时会被读成一个数词。
// 这里并排四种切法，第一行就是生产在用的（给未读加行底色的缝 + 拉开间距），其余只在本页生效。
const clashRows: DemoRow[] = [
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-clash-prod",
      title: "生产版：间距 12 + 未读加一圈行底色缝",
      progressState: "blocked",
      evidence: "缺设计稿确认，接口字段也还没定",
      unreadCount: 99,
      updatedAt: minutesAgo(6),
      lastMessageAt: minutesAgo(6),
    }),
  },
  {
    level: "full",
    hook: "demo-no-ring",
    item: makeItem({
      conversationId: "demo-clash-a",
      title: "A：只拉间距（12），不加缝",
      progressState: "blocked",
      unreadCount: 99,
      updatedAt: minutesAgo(7),
      lastMessageAt: minutesAgo(7),
    }),
  },
  {
    level: "full",
    hook: "demo-gap2",
    item: makeItem({
      conversationId: "demo-clash-b",
      title: "B：只加缝（8 间距不变）",
      progressState: "blocked",
      unreadCount: 99,
      updatedAt: minutesAgo(8),
      lastMessageAt: minutesAgo(8),
    }),
  },
  {
    level: "full",
    hook: "demo-ring100",
    item: makeItem({
      conversationId: "demo-clash-a-plus-b2",
      title: "对照：缝用 base-100（白），会读成白边",
      progressState: "blocked",
      unreadCount: 99,
      updatedAt: minutesAgo(9),
      lastMessageAt: minutesAgo(9),
    }),
  },
  {
    level: "full",
    hook: "demo-accent",
    item: makeItem({
      conversationId: "demo-clash-c",
      title: "C：阻塞改紫（accent）",
      progressState: "blocked",
      unreadCount: 99,
      updatedAt: minutesAgo(10),
      lastMessageAt: minutesAgo(10),
    }),
  },
];

// 待定问题数：与未读并列但含义不同（它数的是等你回答的问题），且后端查询尚未接上，
// 所以这里专门做一组假数据，验证「有数才出现、0 与缺失一律不占位」这条硬门。
const pendingRows: DemoRow[] = [
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-pending-only",
      title: "待定 2 + 无未读",
      questionCount: 2,
      updatedAt: minutesAgo(4),
      lastMessageAt: minutesAgo(4),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-pending-unread",
      title: "待定 3 + 未读 3（两个数字，形状必须分得开）",
      questionCount: 3,
      unreadCount: 3,
      updatedAt: minutesAgo(5),
      lastMessageAt: minutesAgo(5),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-pending-state-unread",
      title: "状态 + 待定 1 + 未读 99（三件同框）",
      progressState: "blocked",
      evidence: "缺设计稿确认，接口字段也还没定",
      questionCount: 1,
      unreadCount: 99,
      updatedAt: minutesAgo(6),
      lastMessageAt: minutesAgo(6),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-pending-zero",
      title: "待定 0 + 未读 5（0 不渲染，红圆保持原样）",
      questionCount: 0,
      unreadCount: 5,
      updatedAt: minutesAgo(7),
      lastMessageAt: minutesAgo(7),
    }),
  },
  {
    level: "full",
    item: makeItem({
      conversationId: "demo-pending-cap",
      title: "待定 100 → 99+",
      questionCount: 100,
      updatedAt: minutesAgo(8),
      lastMessageAt: minutesAgo(8),
    }),
  },
];

const groups: Array<{ key: string; title: string; rows: DemoRow[] }> = [
  { key: "basic", title: "状态 × 未读：左边状态，右边未读", rows },
  { key: "pending", title: "待定问题数：有数才出现，0 与缺失不占位", rows: pendingRows },
  { key: "clash", title: "阻塞红 vs 未读红", rows: clashRows },
];

function resolveLevel(row: DemoRow): ConversationItemLevel {
  if (shape.value === "compact") return simpleConversationItemLevel(row.item);
  return row.level;
}
</script>

<style scoped>
/* 下面四条只在本页生效，用来并排对比撞色的几种切法，不参与生产渲染。 */

/* A 的反面：把间距退回改动前的 8px */
.demo-gap2 :deep(.ecall-conv-tail) {
  column-gap: 0.5rem;
}

/* 只拉间距、不加缝 */
.demo-no-ring :deep(.ecall-conv-unread) {
  box-shadow: none;
}

/* 缝改用 base-100（白），说明它为什么不适合 */
.demo-ring100 :deep(.ecall-conv-unread) {
  box-shadow: 0 0 0 2px var(--color-base-100);
}

/* C：阻塞改用 accent 紫 */
.demo-accent :deep(.ecall-progress-badge) {
  background-color: var(--color-accent);
  color: var(--color-accent-content);
}
</style>
