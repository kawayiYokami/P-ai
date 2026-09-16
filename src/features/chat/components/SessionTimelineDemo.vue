<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs">
      <label class="flex items-center gap-1">
        演示宽度
        <select v-model.number="demoWidth" class="select select-bordered select-xs">
          <option :value="618">618 · 聊天窗实际</option>
          <option :value="560">560</option>
          <option :value="480">480</option>
          <option :value="400">400 · 压测</option>
        </select>
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="longList" type="checkbox" class="checkbox checkbox-xs" />
        多轮（看内部滚动）
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="clipMessages" type="checkbox" class="checkbox checkbox-xs" />
        截断到 100 字
      </label>
      <span class="text-base-content/45">时间线按钮（图标 + 「会话时间线」常驻），点击展开覆盖整个聊天区的垂直时间线</span>
    </div>

    <!-- 模拟聊天区：时间线按钮钉在右上角，展开面板几乎盖满整个区域 -->
    <div
      class="relative flex h-105 flex-col overflow-hidden rounded-box border border-base-300 bg-base-200 transition-[width] duration-200"
      :style="{ width: `min(${demoWidth}px, 100%)` }"
    >
      <div class="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden p-3">
        <div class="h-9 w-2/3 self-start rounded-xl bg-base-100/80"></div>
        <div class="h-14 w-4/5 self-end rounded-xl bg-primary/15"></div>
        <div class="h-9 w-1/2 self-start rounded-xl bg-base-100/80"></div>
        <div class="h-12 w-3/5 self-end rounded-xl bg-primary/15"></div>
      </div>

      <button
        v-if="!panelOpen"
        type="button"
        class="absolute right-2 top-2 z-30 flex items-center rounded-full p-2 pointer-events-auto"
        :class="FROST_SURFACE"
        aria-label="展开会话时间线"
        @click="panelOpen = true"
      >
        <Route class="h-4 w-4 shrink-0" />
        <span class="ml-1.5 whitespace-nowrap text-xs leading-none">会话时间线</span>
      </button>

      <Transition
        enter-active-class="transition duration-150 ease-out"
        enter-from-class="opacity-0"
        leave-active-class="transition duration-100 ease-in"
        leave-to-class="opacity-0"
      >
        <div
          v-if="panelOpen"
          class="absolute inset-0 z-20 flex items-center justify-center"
        >
          <div class="absolute inset-0 bg-black/50"></div>
          <div
            class="relative flex h-[92%] w-[92%] max-w-3xl flex-col overflow-hidden rounded-box shadow-lg"
            :class="FROST_GLASS"
          >

          <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full px-4 py-4">
            <ul class="timeline timeline-snap-icon max-md:timeline-compact timeline-vertical">
              <li v-for="(message, index) in messages" :key="message.id">
                <hr v-if="index > 0" class="bg-base-300" />
                <div class="timeline-middle">
                  <span
                    class="flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium text-base-content/80 transition-shadow"
                    :class="[
                      message.role === 'user' ? 'bg-primary/20' : 'bg-base-300',
                      index === messages.length - 1 ? 'ring-2 ring-primary' : '',
                    ]"
                  >{{ message.speaker.slice(0, 1) }}</span>
                </div>
                <div
                  class="rounded-box px-2 py-1 transition-colors hover:bg-base-200/70 hover:text-primary"
                  :class="message.role === 'user' ? 'timeline-end text-start md:mb-10' : 'timeline-start mb-10 text-start md:text-end'"
                >
                  <time class="font-mono text-xs italic opacity-50">{{ message.time }}</time>
                  <div class="text-sm font-black">{{ message.speaker }}</div>
                  <div class="text-xs">
                    <InlineMarkdownText
                      :text="message.text"
                      :limit="clipMessages ? CLIP_TEXT_LIMIT : 0"
                      :head-ratio="0.3"
                    />
                  </div>
                </div>
                <hr v-if="index < messages.length - 1" class="bg-base-300" />
              </li>
            </ul>
          </OverlayScrollArea>
          <div class="flex shrink-0 items-center justify-between gap-2 px-3 py-2">
            <span class="flex items-center gap-1 text-xs text-base-content/60">
              <CircleAlert class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              点击消息跳转到目标消息
            </span>
            <button type="button" class="btn btn-sm gap-1" @click="panelOpen = false">
              <ArrowLeft class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              返回
            </button>
          </div>
        </div>
        </div>
      </Transition>
    </div>

    <div class="text-xs text-base-content/50">
      垂直时间线直接用 daisyUI 的 <code>timeline timeline-snap-icon max-md:timeline-compact timeline-vertical</code>：
      每条消息一个 <code>li</code>，按奇偶把内容分到 <code>timeline-start</code>（<code>md:text-end</code> 右对齐）与
      <code>timeline-end</code>，中间是发言人头像，节点之间用 <code>hr</code> 连成竖轨；每条消息标注发言时间。
      正文按「每句只留 150 字：前 30% + 后 70%」截断，超长消息一眼能看到开头和落脚点；展开后是整个容器的覆盖层，背景压黑，
      卡片宽度上限 <code>max-w-3xl</code>、高度 92% 居中，内部交给
      <code>OverlayScrollArea</code> 上下滚动查看历史消息。
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowLeft, CircleAlert, Route } from "@lucide/vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import { FROST_GLASS, FROST_SURFACE } from "./session-float-styles";

const demoWidth = ref(618);
const longList = ref(false);
const clipMessages = ref(true);
const panelOpen = ref(false);

type TimelineMessage = { id: string; role: "user" | "assistant"; speaker: string; time: string; text: string };

const baseMessages: Array<Omit<TimelineMessage, "id">> = [
  {
    role: "user",
    speaker: "红豆",
    time: "11:57",
    text: "优化前端。悬浮按钮上下都常驻；新消息预览如果曾经滚动到过最下、又没有发起新调度，就不预览、变成滚到最下居中；时间线按钮悬停的时候展开一点点，出现图标加「会话时间线」，点击之后展开一个几乎覆盖整个 chatview 的 timeline，内部有滚动区可以上下滚动查看历史消息。",
  },
  {
    role: "assistant",
    speaker: "纳西妲",
    time: "11:58",
    text: "前面两处我直接落在会话悬浮操作区里：下排工作区 bar 与上排时间线按钮不再随滚动位置显隐，只要会话支持悬浮区就一直在位；新消息预览则新增一条判断，曾经到过底部、此后又没有新一轮调度时，不再预览正文，只留一个居中的「回到底部」圆钮。",
  },
  {
    role: "user",
    speaker: "红豆",
    time: "13:20",
    text: "时间线用 daisyUI 那种 timeline 的写法，每条消息一个节点，中间图标、两侧交替放内容，连线用 hr 串起来；每条消息都要把发言时间标出来，长消息不用全放，能认出是哪一句就够了。",
  },
  {
    role: "assistant",
    speaker: "纳西妲",
    time: "13:21",
    text: "那就照这个结构重排：奇偶交替分到 timeline-start 与 timeline-end，timeline-start 在 md 以上右对齐，节点换成发言人头像；每条消息保留 100 字，具体是开头 30 字加结尾 70 字，中间用省略号接起来，这样一眼能看到开头和落脚点。",
  },
  {
    role: "assistant",
    speaker: "纳西妲",
    time: "13:26",
    text: "展开面板几乎盖满聊天区，内部滚动交给 OverlayScrollArea；节点头像按当前所处的那一轮高亮，越往下越接近最新的回答，跳转时把视口对齐到该节点顶部即可。",
  },
];

const messages = computed<TimelineMessage[]>(() => {
  const expandGroup = (group: number) =>
    baseMessages.map((message, index) => ({ ...message, id: `${group}-${index}` }));
  return longList.value
    ? Array.from({ length: 4 }, (_, group) => expandGroup(group)).flat()
    : expandGroup(0);
});

// 每句只留 150 字：前 30% + 后 70%，切点落在行内段边界，不切断行内代码
const CLIP_TEXT_LIMIT = 150;
</script>
