<template>
  <div class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100">
    <!-- 控制项：只改渲染条件，不改数据 -->
    <div class="flex flex-wrap items-center gap-3 border-b border-base-300 px-4 py-2 text-xs">
      <label class="flex items-center gap-1.5">
        <span class="text-base-content/60">状态</span>
        <select v-model="variantKey" class="select select-bordered select-xs">
          <option v-for="variant in variants" :key="variant.key" :value="variant.key">{{ variant.label }}</option>
        </select>
      </label>
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
      <div class="overflow-hidden rounded-box border border-base-300 bg-base-200" :style="{ width: `${rowWidth}px` }">
        <ChatConversationItem
          :item="current.item"
          :level="resolveLevel(current)"
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
    </div>

    <p class="border-t border-base-300 px-4 py-2 text-xs leading-5 text-base-content/60">
      这一页只放一条会话条目卡，用上面的状态切换看它各种形态：未读、当前选中、mini 行、待定问题数。
      喂的是 mock 数据，点击条目、右键菜单、改名、归档等交互一律被丢弃，不写库。
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import { simpleConversationItemLevel, type ConversationItemLevel } from "../utils/conversation-item-display";
import ChatConversationItem from "./ChatConversationItem.vue";

type DemoVariant = {
  key: string;
  label: string;
  item: ChatConversationOverviewItem;
  /** 综合模式下使用的层级；精简模式忽略，统一走 simpleConversationItemLevel */
  level: ConversationItemLevel;
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

const variants: DemoVariant[] = [
  {
    key: "unread",
    label: "有未读",
    level: "full",
    item: makeItem({
      conversationId: "demo-item-unset-unread",
      title: "有未读",
      unreadCount: 3,
      updatedAt: minutesAgo(45),
      lastMessageAt: minutesAgo(45),
      previewMessages: [preview("我看下 monitor.changed 的触发链路。")],
    }),
  },
  {
    key: "read",
    label: "无未读",
    level: "full",
    item: makeItem({
      conversationId: "demo-item-unset-read",
      title: "无未读",
      updatedAt: minutesAgo(40),
      lastMessageAt: minutesAgo(40),
    }),
  },
  {
    key: "active",
    label: "当前选中（未读恒不显示）",
    level: "full",
    item: makeItem({
      conversationId: ACTIVE_CONVERSATION_ID,
      title: "当前选中会话（未读恒不显示）",
      // 当前会话恒不显示未读，这里故意给值，验证它不会漏出来
      unreadCount: 2,
      updatedAt: minutesAgo(1),
      lastMessageAt: minutesAgo(1),
    }),
  },
  {
    key: "mini",
    label: "mini 行（没有第二行）",
    level: "mini",
    item: makeItem({
      conversationId: "demo-item-mini",
      title: "mini 行（没有第二行）",
      updatedAt: minutesAgo(60 * 24 * 30),
      lastMessageAt: minutesAgo(60 * 24 * 30),
      previewMessages: [],
    }),
  },
  // 待定问题数：与未读并列但含义不同（它数的是等你回答的问题），
  // 「有数才出现、0 与缺失一律不占位」是一条硬门，逐个形态都能切到。
  {
    key: "pending-only",
    label: "待定 2 + 无未读",
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
    key: "pending-unread",
    label: "待定 3 + 未读 3（两个数字必须分得开）",
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
    key: "pending-zero",
    label: "待定 0 + 未读 5（0 不渲染）",
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
    key: "pending-cap",
    label: "待定 100 → 99+",
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

const variantKey = ref(variants[0]?.key ?? "");
const current = computed(() => variants.find((variant) => variant.key === variantKey.value) ?? variants[0]!);

function resolveLevel(variant: DemoVariant): ConversationItemLevel {
  if (shape.value === "compact") return simpleConversationItemLevel(variant.item);
  return variant.level;
}
</script>
