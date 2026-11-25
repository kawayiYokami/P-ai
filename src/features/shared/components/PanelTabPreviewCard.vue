<template>
  <!--
    溢出菜单里的一张会话预览卡，两层结构：
    外层（base-300 半透明）放图标与标题，内层（base-100 + 边框）放摘要。
    固定 250×200，内容多少都不变——同一行卡片必须对齐，不靠内容撑高。
  -->
  <div
    class="group/preview relative flex h-[200px] w-[250px] cursor-pointer select-none flex-col gap-2 rounded-box bg-base-300/45 p-3 transition-colors"
    :class="[
      active ? 'ring-1 ring-base-content/20' : 'hover:bg-base-300/65',
      tab.disabled ? 'pointer-events-none opacity-45' : '',
    ]"
    :title="tab.title || tab.label"
    @click="emit('select', tab.key)"
    @contextmenu="emit('contextmenu', tab, $event)"
  >
    <!-- 关闭按钮钉在右上角，不占任何一行的宽度 -->
    <button
      v-if="tab.closeable && !tab.disabled"
      type="button"
      class="btn btn-ghost btn-xs btn-circle absolute right-1.5 top-1.5 size-6 min-h-6 opacity-60 hover:opacity-100"
      :title="closeTitle"
      @click.stop="emit('close', tab.key)"
    >
      <AppIcon name="close" class="size-3.5" />
    </button>

    <!-- 外层：图标紧挨标题，标题占满剩余宽度并给关闭按钮让位 -->
    <div class="flex min-w-0 shrink-0 items-start gap-1.5 pr-6">
      <span class="relative mt-0.5 flex size-5 shrink-0 items-center justify-center">
        <img
          v-if="tab.iconSrc"
          :src="tab.iconSrc"
          alt=""
          class="size-5 rounded-full object-cover"
        />
        <AppIcon
          v-else-if="tab.icon"
          :name="tab.icon"
          class="size-4 text-base-content/60"
          aria-hidden="true"
        />
        <span
          v-if="tab.statusDotClass"
          :class="tab.statusDotClass"
          :title="tab.statusTitle"
          class="absolute -right-0.5 -bottom-0.5 size-2 rounded-full ring-1 ring-base-200"
        />
      </span>
      <div
        class="line-clamp-2 min-w-0 flex-1 text-sm font-semibold leading-snug break-words"
        :class="active ? 'text-base-content' : 'text-base-content/85'"
      >{{ tab.cardTitle || tab.label }}</div>
      <!-- 转圈放在标题后面：叠在图标上会盖住图标本身 -->
      <AppLoading v-if="tab.busy" size="xs" class="mt-1 shrink-0 text-base-content/60" />
    </div>

    <!-- 内层：摘要按聊天气泡排（用户靠右、助手靠左），圆角取「外层圆角 − 外层内边距」 -->
    <div
      class="min-h-0 flex-1 overflow-hidden rounded-[calc(var(--radius-box)-0.5rem)] border border-base-300/70 bg-base-100 px-2 py-1.5"
    >
      <template v-if="tab.previewMessages && tab.previewMessages.length > 0">
        <div
          v-for="(message, index) in tab.previewMessages"
          :key="index"
          class="chat"
          :class="message.role === 'user' ? 'chat-end' : 'chat-start'"
        >
          <div
            class="chat-bubble text-xs leading-snug break-words"
            :class="message.role === 'user' ? 'chat-bubble-primary' : ''"
          >{{ message.textPreview }}</div>
        </div>
      </template>
      <!-- 没有摘要时给骨架：和真实气泡同一套排布与尺寸（用户靠右、助手靠左），
           不写「暂无摘要」，也不带加载动画 -->
      <div v-else class="flex flex-col" aria-hidden="true">
        <div class="chat chat-end">
          <div class="chat-bubble h-9 w-3/5 !bg-base-content/10"></div>
        </div>
        <div class="chat chat-start">
          <div class="chat-bubble h-20 w-4/5 !bg-base-content/10"></div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import AppLoading from "./AppLoading.vue";
import type { PanelTabStripItem } from "./panel-tab-strip.types";

defineProps<{
  tab: PanelTabStripItem;
  active: boolean;
  closeTitle: string;
}>();

const emit = defineEmits<{
  (e: "select", key: string): void;
  (e: "close", key: string): void;
  (e: "contextmenu", tab: PanelTabStripItem, event: MouseEvent): void;
}>();
</script>
