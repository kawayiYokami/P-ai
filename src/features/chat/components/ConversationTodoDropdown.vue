<template>
  <div
    v-if="hasActiveOrPendingTodo"
    ref="shellRef"
    class="pointer-events-none flex justify-center pb-1"
  >
    <div
      class="todo-shell bg-base-100/70 shadow backdrop-blur-md group pointer-events-auto"
      :class="{ 'todo-shell-expanded': expanded }"
      :aria-label="t('config.task.fields.todo')"
      @click.stop
      @mousedown.stop
      @mouseleave="expanded = false"
    >
      <button
        type="button"
        class="todo-header hover:bg-base-content/10"
        :aria-expanded="expanded"
        @click="toggleExpanded"
      >
        <span class="shrink-0" :class="activeTodoStatus === 'in_progress' ? 'todo-dot-active' : 'todo-dot-idle'"></span>
        <span class="min-w-0 flex-1 truncate text-left text-sm">{{ activeConversationTodoDisplay }}</span>
        <span
          v-if="todos.length > 1"
          class="badge badge-ghost badge-sm shrink-0 font-normal tabular-nums"
        >{{ completedCount }}/{{ todos.length }}</span>
        <AppIcon name="chevron-down"
          class="h-3.5 w-3.5 shrink-0 text-base-content/50 transition-transform duration-300"
        />
      </button>
      <div class="todo-body">
        <button
          v-if="expanded"
          type="button"
          class="todo-collapse-btn opacity-0 transition-opacity duration-200 group-hover:opacity-60 hover:opacity-100"
          :aria-label="t('chat.todoCollapse')"
          @click="toggleExpanded"
        >
          <AppIcon name="chevron-up" class="h-4 w-4" />
        </button>
        <div class="todo-body-inner">
          <div class="flex items-center justify-between gap-3 text-xs text-base-content/60">
            <span class="font-medium">{{ t("config.task.fields.todo") }}</span>
            <span class="tabular-nums">{{ t("chat.todoProgress", { done: completedCount, total: todos.length }) }}</span>
          </div>
          <progress
            class="progress progress-primary h-1.5"
            :value="completedCount"
            :max="todos.length"
          ></progress>
          <ul class="mt-1 flex flex-col gap-1.5">
            <li
              v-for="(item, index) in todos"
              :key="`${item.status}-${index}-${item.content}`"
              class="flex items-start gap-2.5"
              :title="item.content"
            >
              <span
                class="todo-dot mt-2 shrink-0"
                :class="todoStatusDotClass(item.status)"
              ></span>
              <span
                class="min-w-0 flex-1 wrap-break-word text-sm leading-6"
                :class="todoTextClass(item.status)"
              >{{ item.content }}</span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";

interface NormalizedTodo {
  content: string;
  status: "pending" | "in_progress" | "completed";
}

const props = defineProps<{
  todos: NormalizedTodo[];
  personaName: string;
}>();

const { t } = useI18n();

const shellRef = ref<HTMLElement | null>(null);
const expanded = ref(false);

function toggleExpanded() {
  expanded.value = !expanded.value;
}

function handlePointerDownOutside(event: PointerEvent) {
  if (!expanded.value) return;
  const target = event.target as Node | null;
  if (target && !shellRef.value?.contains(target)) {
    expanded.value = false;
  }
}

onMounted(() => document.addEventListener("pointerdown", handlePointerDownOutside));
onBeforeUnmount(() => document.removeEventListener("pointerdown", handlePointerDownOutside));

const hasActiveOrPendingTodo = computed(() =>
  props.todos.some((item) => item.status === "pending" || item.status === "in_progress"),
);

const activeTodoIndex = computed(() => {
  const inProgressIndex = props.todos.findIndex((item) => item.status === "in_progress");
  if (inProgressIndex >= 0) return inProgressIndex;
  const pendingIndex = props.todos.findIndex((item) => item.status === "pending");
  if (pendingIndex >= 0) return pendingIndex;
  return props.todos.length ? 0 : -1;
});

const activeTodoStatus = computed(() => {
  const index = activeTodoIndex.value;
  if (index < 0) return "pending";
  return props.todos[index]?.status ?? "pending";
});

const completedCount = computed(() => props.todos.filter((item) => item.status === "completed").length);

const activeConversationTodo = computed(() => {
  const index = activeTodoIndex.value;
  if (index < 0) return "";
  return String(props.todos[index]?.content || "").trim();
});

const activeConversationTodoDisplay = computed(() => {
  const todo = activeConversationTodo.value;
  if (!todo) return "";
  const name = String(props.personaName || "").trim();
  return name
    ? t("chat.todoIntentionWithPersona", { name, todo })
    : t("chat.todoIntention", { todo });
});

function todoStatusDotClass(status: NormalizedTodo["status"]): string {
  if (status === "completed") return "bg-success";
  if (status === "in_progress") return "bg-primary shadow-[0_0_0_3px] shadow-primary/20";
  return "bg-base-300";
}

function todoTextClass(status: NormalizedTodo["status"]): string {
  if (status === "completed") return "text-base-content/50 line-through";
  if (status === "in_progress") return "text-base-content font-semibold";
  return "text-base-content";
}
</script>

<style scoped>
/* 折叠态：一行小条（固定行高），贴底圆角 */
.todo-shell {
  width: 100%;
  max-width: min(88vw, 30rem);
  display: grid;
  grid-template-rows: 2.5rem 0fr;
  border-radius: 0 0 var(--radius-field) var(--radius-field);
  overflow: hidden;
  transition:
    grid-template-rows 340ms cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 340ms cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 340ms ease;
}

/* 展开态：小条行高归零消失，大卡片全圆角 */
.todo-shell-expanded {
  grid-template-rows: 0rem 1fr;
  border-radius: var(--radius-box, 1rem);
}

.todo-header {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 0.5rem;
  padding: 0 0.5rem 0 0.75rem;
  text-align: left;
  cursor: pointer;
  min-height: 0;
  overflow: hidden;
  transition: background-color 180ms ease;
}

.todo-collapse-btn {
  position: absolute;
  top: 0.3rem;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.15rem 0.6rem;
  cursor: pointer;
  color: var(--color-base-content);
  border-radius: 9999px;
  transition: background-color 180ms ease;
}

.todo-body {
  position: relative;
  min-height: 0;
  overflow: hidden;
}

.todo-body-inner {
  max-height: min(55vh, 26rem);
  overflow-y: auto;
  padding: 0.5rem 0.75rem 0.75rem;
}

.todo-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
}

.todo-dot-active {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
  background: var(--color-primary);
  animation: todo-dot-pulse 1.8s ease-in-out infinite;
}

.todo-dot-idle {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
  background: var(--color-base-content);
  opacity: 0.4;
}

@keyframes todo-dot-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 0 color-mix(in oklab, var(--color-primary) 45%, transparent);
  }
  60% {
    box-shadow: 0 0 0 6px transparent;
  }
}
</style>