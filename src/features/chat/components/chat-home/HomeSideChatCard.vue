<template>
  <CardShell
    tone="secondary"
    :icon="MessageSquareMore"
    :label="label || t('chat.sideChat.title')"
    :avatar-src="avatarSrc"
    :busy="busy"
    :unread-count="unreadCount"
    :has-error="hasError"
    :pulsing="busy"
    interactive
    @select="emit('open')"
  >
    <span class="line-clamp-2 text-xs leading-snug text-base-content/85">{{ title }}</span>
    <ArrowUpRight class="mt-auto size-3.5 self-end text-base-content/25" aria-hidden="true" />
  </CardShell>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowUpRight, MessageSquareMore } from "@lucide/vue";
import CardShell from "./CardShell.vue";

defineProps<{
  title: string;
  /** 卡片头标；队员卡片传队员名，普通追问留空走默认「追问」 */
  label?: string;
  /** 队员头像；给了就显示圆形头像 */
  avatarSrc?: string;
  /** 调度中：转圈 */
  busy?: boolean;
  /** 未读数：绿色角标 */
  unreadCount?: number;
  /** 调度失败：红色角标 */
  hasError?: boolean;
}>();

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();
</script>
