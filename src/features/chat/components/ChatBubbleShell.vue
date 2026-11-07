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
            <span
              v-if="streaming"
              class="ecall-chat-bubble-meta ecall-chat-bubble-streaming-meta"
              :class="{ 'ecall-chat-bubble-streaming-meta-themed': isDark }"
            >
              <AppLoading size="0.62rem" />
              {{ streamingText || "正在生成" }}
            </span>
            <span v-else-if="meta" class="ecall-chat-bubble-meta">{{ meta }}</span>
          </div>

          <div v-show="showActivity && !!$slots.activity" class="ecall-chat-bubble-activity">
            <slot name="activity" />
          </div>
        </div>
      </div>

      <div v-show="showActivity && !!$slots['activity-panel']" class="ecall-chat-bubble-activity-panel">
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
import AppLoading from "../../shared/components/AppLoading.vue";

const props = withDefaults(defineProps<{
  tone?: "assistant" | "user" | "system";
  name: string;
  meta?: string;
  avatarUrl?: string;
  streaming?: boolean;
  streamingText?: string;
  /** 当前是否深色主题。深色下状态胶囊改用主题色，浅色保持中性 */
  isDark?: boolean;
  separated?: boolean;
  wide?: boolean;
  bubbleBackground?: boolean;
  contentEmpty?: boolean;
  showActivity?: boolean;
}>(), {
  tone: "assistant",
  meta: "",
  avatarUrl: "",
  streaming: false,
  streamingText: "",
  isDark: false,
  separated: false,
  wide: false,
  bubbleBackground: false,
  contentEmpty: false,
  showActivity: true,
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
  color: color-mix(in srgb, var(--color-base-content) 80%, transparent);
  font-size: var(--app-text-xs-size);
  font-weight: 400;
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
  /* 头部是基线对齐的，这层里装了非文字的加载指示器，基线会取到它底边，把整段状态文字顶高；
     让它自己按行居中，文字才会和左边的名字站在同一条线上 */
  align-self: center;
  /* 行高必须钉成 1。这层带上下 padding，如果继承正文的 1.2 行高，盒高会到 17.6px，
     比名字的 15.6px 更高，反而成了头部这一行里最高的一项——align-self:center 再没有余量可居中，
     结果顶边对齐名字、底边多出 2px，看起来就是「胶囊往下坠」。钉成 1 之后盒高 15.2px，
     低于名字，名字重新成为行高基准，胶囊上下各留 0.2px，才是真的居中 */
  line-height: 1;
  gap: 0.2rem;
  /* 状态用中性胶囊：彩色只留给工具图标，这里一旦上主色就会比说话人名字还亮，主次颠倒。
     文字取前景色 72%，和头像行同层，深浅主题都靠明暗自适应 */
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  background: color-mix(in oklab, var(--color-base-content) 8%, transparent);
  color: color-mix(in oklab, var(--color-base-content) 72%, transparent);
  font-size: var(--app-text-caption-size);
}

/* 深色主题的状态胶囊改用主题色。深色底的 primary 是针对 #2b2b2b 单独提亮过的一档
   （#319eea，4.63:1），做胶囊文字本身够用；浅色主题不动，保持中性，彩色只留给工具图标。
   用双类写死特异性，不依赖这条规则排在 .ecall-chat-bubble-streaming-meta 之后 */
.ecall-chat-bubble-streaming-meta.ecall-chat-bubble-streaming-meta-themed {
  background: color-mix(in oklab, var(--color-primary) 14%, transparent);
  color: var(--color-primary);
}

/* 气泡头的状态点比正文小一号，尺寸由 AppLoading 的 size 传（约 0.62rem），不再用类压 */

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
