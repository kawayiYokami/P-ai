<template>
  <div class="w-[340px] rounded-box border border-base-300 bg-base-200/40 p-2">
    <div class="mb-1 px-1 text-xs text-base-content/50">侧栏定时器列表：每项一行</div>
    <TimerListItem v-for="timer in timers" :key="timer.id" :timer="timer" />
    <div class="mt-2 px-1 text-xs text-base-content/50">
      悬停任一行可看到「频率 / 下次触发 / 状态」；行内不出现 cron 语法。
    </div>
  </div>
</template>

<script setup lang="ts">
import TimerListItem from "./TimerListItem.vue";
import type { TimerEntry } from "../utils/timer-entry";

const now = Date.now();
const iso = (offsetMs: number) => new Date(now + offsetMs).toISOString();

const base: Omit<TimerEntry, "id" | "message"> = {
  conversationId: "conv-1",
  dispatchMode: "direct",
  enabled: true,
  runAt: iso(-86_400_000),
  cron: null,
  endAt: null,
  lastTriggeredAt: iso(-3_600_000),
  nextRunAt: null,
  createdAt: iso(-86_400_000 * 7),
};

const timers: TimerEntry[] = [
  {
    ...base,
    id: "1",
    message: "检查清华 PyPI 镜像是否已同步 p-memory 新版本",
    cron: "*/10 * * * *",
    nextRunAt: iso(10 * 60_000),
  },
  {
    ...base,
    id: "2",
    message: "收集整理火山引擎方舟 Coding Plan 的价格页",
  },
  {
    ...base,
    id: "3",
    message: "测试工具重构后功能完整性",
    enabled: false,
  },
  {
    ...base,
    id: "4",
    message: "每天早上把今天的日程整理成一封简报，发到这个会话里",
    cron: "0 9 * * 1,2,3,4,5",
    nextRunAt: iso(14 * 3_600_000),
  },
];
</script>
