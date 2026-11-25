<template>
  <div class="space-y-4">
    <GalleryControls>
      <span class="text-xs text-base-content/70">形态</span>
      <button
        v-for="option in options"
        :key="option.key"
        type="button"
        class="btn btn-ghost btn-xs"
        :class="active === option.key ? 'btn-primary' : ''"
        @click="show(option.key)"
      >
        {{ option.label }}
      </button>
      <span class="text-xs text-base-content/50">弹窗以 modal 覆盖整页，这是它在应用里的真实观感</span>
    </GalleryControls>

    <div class="text-xs text-base-content/60">
      会话下拉走 `conversation.overview.list`，纯浏览器环境拿不到，下拉里只剩「全局」——这不影响弹窗其余部分。
    </div>

    <TimerCreateCard
      :open="open"
      :mode="mode"
      :timer="timer"
      :default-conversation-id="defaultConversationId"
      @close="open = false"
      @saved="open = false"
      @enabled-change="handleEnabledChange"
      @deleted="open = false"
    />

    <button type="button" class="btn btn-sm btn-primary" @click="show(active)">
      重新打开弹窗
    </button>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref } from "vue";
import TimerCreateCard from "./TimerCreateCard.vue";
import GalleryControls from "../../../dev-gallery/GalleryControls.vue";
import type { TimerEntry } from "../../utils/timer-entry";

const open = ref(true);
const mode = ref<"create" | "edit">("create");
const timer = ref<TimerEntry | null>(null);
const active = ref("create-cron");
/** 模拟「在某个会话里打开新建」：宿主会把它当前会话传进来 */
const defaultConversationId = ref("");

const now = new Date();
const iso = new Date(now.getTime() - 60_000).toISOString();

const sampleTimer: TimerEntry = {
  id: "42",
  message: "每天早上把今天的日程整理成一封简报，发到这个会话里。",
  conversationId: null,
  dispatchMode: "delegate",
  enabled: true,
  runAt: iso,
  cron: "0 9 * * 1,2,3,4,5",
  endAt: null,
  lastTriggeredAt: null,
  nextRunAt: null,
  createdAt: iso,
};

const options = [
  { key: "create-cron", label: "新建 · 定时" },
  { key: "create-once", label: "新建 · 一次性" },
  { key: "create-in-conversation", label: "新建 · 会话内" },
  { key: "edit-cron", label: "编辑 · 已有 cron" },
];

/** 拨启用开关：demo 里只更新本地这条，不关弹窗 */
function handleEnabledChange(updated: TimerEntry) {
  timer.value = updated;
}

async function show(key: string) {
  active.value = key;
  // 先关再开：弹窗只在 open 由 false 变 true 时重置表单，同刻切换形态必须走这一遭
  open.value = false;
  await nextTick();
  defaultConversationId.value = key === "create-in-conversation" ? "conv-20261125-demo" : "";
  if (key === "edit-cron") {
    mode.value = "edit";
    timer.value = { ...sampleTimer };
  } else {
    mode.value = "create";
    timer.value = null;
  }
  open.value = true;
}
</script>
