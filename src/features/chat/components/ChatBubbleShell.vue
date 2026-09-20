<template>
  <div :class="['ecall-chat-bubble-shell', `ecall-chat-bubble-tone-${tone}`, { 'ecall-chat-bubble-separated': separated, 'ecall-chat-bubble-wide': wide, 'ecall-chat-bubble-no-avatar': !avatarUrl }]">
    <template v-if="tone === 'user'">
      <div class="ecall-chat-bubble-body">
        <div class="ecall-chat-bubble-surface" :style="surfaceStyle">
          <slot />
        </div>

        <div v-if="$slots.footer" class="ecall-chat-bubble-footer">
          <slot name="footer" />
        </div>
      </div>
    </template>

    <template v-else>
      <div class="ecall-chat-bubble-head">
        <div v-if="avatarUrl" class="ecall-chat-bubble-avatar" :title="name">
          <img :src="avatarUrl" :alt="name" />
        </div>

        <div class="ecall-chat-bubble-main">
          <div class="ecall-chat-bubble-header">
            <span class="ecall-chat-bubble-name">{{ name }}</span>
            <span v-if="streaming" class="ecall-chat-bubble-meta ecall-chat-bubble-streaming-meta">
              <span class="loading loading-spinner loading-xs" aria-hidden="true"></span>
              {{ streamingText || "正在生成" }}
            </span>
            <span v-else-if="meta" class="ecall-chat-bubble-meta">{{ meta }}</span>
          </div>

          <div v-if="$slots.activity" class="ecall-chat-bubble-activity">
            <slot name="activity" />
          </div>
        </div>
      </div>

      <div v-if="$slots['activity-panel']" class="ecall-chat-bubble-activity-panel">
        <slot name="activity-panel" />
      </div>

      <div class="ecall-chat-bubble-body">
        <div class="ecall-chat-bubble-surface" :style="surfaceStyle">
          <slot />
        </div>

        <div v-if="$slots.footer" class="ecall-chat-bubble-footer">
          <slot name="footer" />
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, type StyleValue } from "vue";

const props = withDefaults(defineProps<{
  tone?: "assistant" | "user" | "system";
  name: string;
  meta?: string;
  avatarUrl?: string;
  streaming?: boolean;
  streamingText?: string;
  separated?: boolean;
  wide?: boolean;
  bubbleBackground?: boolean;
  contentEmpty?: boolean;
}>(), {
  tone: "assistant",
  meta: "",
  avatarUrl: "",
  streaming: false,
  streamingText: "",
  separated: false,
  wide: false,
  bubbleBackground: false,
  contentEmpty: false,
});

const surfaceStyle = computed<StyleValue | undefined>(() => {
  if (props.contentEmpty) {
    return {
      backgroundColor: "transparent",
      padding: 0,
    };
  }
  if (props.tone === "user") {
    return {
      borderRadius: "var(--radius-box, 1rem)",
      backgroundColor: "var(--color-base-300)",
      padding: "0.68rem 1rem",
    };
  }
  if (!props.bubbleBackground) return undefined;
  return {
    borderRadius: "var(--radius-box, 1rem)",
    backgroundColor: "var(--color-base-100)",
    padding: "0.68rem 1rem",
  };
});
</script>

<style scoped>
/* ========== 表格骨架：助手/系统 = 头像列 + 内容列；user = 右对齐、无头像 ========== */
.ecall-chat-bubble-shell {
  --ecall-bubble-avatar-size: 2rem;
  --ecall-bubble-gap: 0.55rem;
  --ecall-bubble-max-width: 100%;
  position: relative;
  width: 100%;
}

.ecall-chat-bubble-tone-user {
  --ecall-bubble-max-width: 42rem;
  display: flex;
  justify-content: flex-end;
}

.ecall-chat-bubble-wide {
  --ecall-bubble-max-width: 100%;
}

/* 连续消息分隔线：沿气泡区域顶边 */
.ecall-chat-bubble-separated::before {
  position: absolute;
  top: -0.5rem;
  left: 0;
  width: min(var(--ecall-bubble-max-width), 100%);
  height: 1px;
  background: color-mix(in srgb, var(--color-base-content) 14%, transparent);
  content: "";
  pointer-events: none;
  transform: scaleY(0.5);
  transform-origin: center;
}

.ecall-chat-bubble-tone-user.ecall-chat-bubble-separated::before {
  left: auto;
  right: 0;
}

/* ---------- 表格：头像列 + 内容列，正文与 footer 跨全宽，天然对齐头像左缘 ---------- */
.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) {
  display: grid;
  grid-template-columns: var(--ecall-bubble-avatar-size) minmax(0, 1fr);
  grid-template-areas:
    "avatar main"
    "activity-panel activity-panel"
    "body body";
  column-gap: var(--ecall-bubble-gap);
  align-items: start;
}

.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) .ecall-chat-bubble-head {
  display: contents;
}

.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) .ecall-chat-bubble-avatar {
  grid-area: avatar;
  margin: 0.25rem 0;
}

.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) .ecall-chat-bubble-main {
  grid-area: main;
  padding: 0.25rem 0;
}

.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) .ecall-chat-bubble-activity-panel {
  grid-area: activity-panel;
}

.ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user) .ecall-chat-bubble-body {
  box-sizing: border-box;
  grid-area: body;
  width: min(var(--ecall-bubble-max-width), 100%);
  max-width: 100%;
}

/* 无头像：不占头像列 */
.ecall-chat-bubble-shell.ecall-chat-bubble-no-avatar:not(.ecall-chat-bubble-tone-user),
.ecall-chat-bubble-shell-no-avatar:not(.ecall-chat-bubble-tone-user) {
  grid-template-columns: minmax(0, 1fr);
  grid-template-areas:
    "main"
    "activity-panel"
    "body";
}

/* 宽屏：正文与 footer 让开头像列、对齐名字左缘；窄屏（手机 / 侧栏）仍贴头像左缘用满整宽 */
@container (min-width: 800px) {
  .ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user):not(.ecall-chat-bubble-no-avatar) {
    grid-template-areas:
      "avatar main"
      ". activity-panel"
      ". body";
  }

  /* 右侧补上与头像列等宽的空当，正文与思考 / 工具面板左右留白对称 */
  .ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user):not(.ecall-chat-bubble-no-avatar) .ecall-chat-bubble-body,
  .ecall-chat-bubble-shell:not(.ecall-chat-bubble-tone-user):not(.ecall-chat-bubble-no-avatar) .ecall-chat-bubble-activity-panel {
    padding-right: calc(var(--ecall-bubble-avatar-size) + var(--ecall-bubble-gap));
  }
}

/* ---------- user：右对齐，无头像 ---------- */
.ecall-chat-bubble-tone-user .ecall-chat-bubble-body {
  width: auto;
  max-width: min(var(--ecall-bubble-max-width), 100%);
  align-items: flex-end;
}

.ecall-chat-bubble-tone-user .ecall-chat-bubble-footer {
  flex-direction: row-reverse;
}

/* ---------- 共通 ---------- */
.ecall-chat-bubble-body {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.25rem;
}

.ecall-chat-bubble-avatar {
  display: inline-flex;
  flex: 0 0 var(--ecall-bubble-avatar-size);
  width: var(--ecall-bubble-avatar-size);
  height: var(--ecall-bubble-avatar-size);
  margin-top: 0.18rem;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-radius: 999px;
  background: var(--color-neutral);
  color: var(--color-neutral-content);
  font-size: var(--app-text-sm-size);
  font-weight: 650;
  line-height: 1;
}

.ecall-chat-bubble-avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.ecall-chat-bubble-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
}

.ecall-chat-bubble-header,
.ecall-chat-bubble-footer {
  display: inline-flex;
  max-width: 100%;
  align-items: baseline;
  gap: 0.45rem;
}

.ecall-chat-bubble-name {
  min-width: 0;
  overflow: hidden;
  color: color-mix(in srgb, var(--color-base-content) 86%, transparent);
  font-size: var(--app-text-xs-size);
  font-weight: 560;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ecall-chat-bubble-meta {
  color: color-mix(in srgb, var(--color-base-content) 55%, transparent);
  font-size: var(--app-text-xs-size);
  line-height: 1.2;
}

.ecall-chat-bubble-footer {
  color: color-mix(in srgb, var(--color-base-content) 42%, transparent);
  font-size: var(--app-text-xs-size);
  line-height: 1.2;
}

.ecall-chat-bubble-streaming-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  color: var(--color-primary);
  font-size: var(--app-text-caption-size);
}

.ecall-chat-bubble-streaming-meta .loading {
  width: 0.62rem;
  height: 0.62rem;
}

.ecall-chat-bubble-surface {
  width: fit-content;
  max-width: 100%;
  color: var(--color-base-content);
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size));
  line-height: 1.5;
}

.ecall-chat-bubble-wide .ecall-chat-bubble-surface {
  width: 100%;
}

.ecall-chat-bubble-activity {
  width: 100%;
}

/* 展开区与气泡左缘对齐，无额外偏移 */

.ecall-chat-bubble-footer {
  min-height: 1.25rem;
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms ease;
}

.ecall-chat-bubble-shell:hover .ecall-chat-bubble-footer,
.ecall-chat-bubble-shell:focus-within .ecall-chat-bubble-footer {
  opacity: 1;
  pointer-events: auto;
}
</style>
