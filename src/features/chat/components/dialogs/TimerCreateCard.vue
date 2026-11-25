<template>
  <dialog ref="dialogRef" class="modal" @close="handleDialogClose" @cancel.prevent="handleDialogClose">
    <div class="modal-box w-11/12 max-w-lg p-0">
      <div class="flex items-center justify-between gap-3 border-b border-base-300 px-5 py-4">
        <h3 class="min-w-0 truncate text-base font-semibold">{{ dialogTitle }}</h3>
        <div class="flex shrink-0 items-center gap-3">
          <!-- 启用与否是这条定时器的总开关，放在标题行；底部只留操作按钮 -->
          <label
            v-if="isEditMode"
            class="flex items-center gap-2 text-xs text-base-content/70"
            :class="busy ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
          >
            {{ t("chat.timerCreate.enable") }}
            <input
              type="checkbox"
              class="toggle toggle-sm toggle-success"
              :checked="!!timer?.enabled"
              :disabled="busy"
              @change="handleToggleEnabled"
            />
          </label>
          <button
            type="button"
            class="btn btn-ghost btn-sm btn-circle"
            :disabled="busy"
            :title="t('common.close')"
            @click="requestClose"
          >
            <AppIcon name="close" class="h-4 w-4" />
          </button>
        </div>
      </div>

      <form @submit.prevent="handleSubmit">
        <OverlayScrollArea scroller-class="max-h-[68vh] overflow-y-auto">
          <div class="space-y-4 px-5 py-4">
          <div
            v-if="errorText"
            class="rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error whitespace-pre-wrap break-all"
          >
            {{ errorText }}
          </div>

          <div class="space-y-4">
            <label class="block space-y-2">
              <span class="block text-sm font-medium">{{ t("chat.timerCreate.messageLabel") }}</span>
              <textarea
                v-model="message"
                class="textarea textarea-bordered min-h-40 w-full resize-none"
                rows="6"
                :placeholder="t('chat.timerCreate.messagePlaceholder')"
                :disabled="busy"
              ></textarea>
            </label>

            <div class="block space-y-2">
              <span class="block text-sm font-medium">{{ t("chat.timerCreate.conversationLabel") }}</span>
              <!-- 会话内打开的新建：目标已由宿主定为当前会话，不给可点控件，免得让人以为还得设一遍 -->
              <div
                v-if="lockedConversationId"
                class="rounded-box border border-base-300 bg-base-200/60 px-3 py-2 text-sm text-base-content/80"
              >
                {{ t("chat.timerCreate.conversationLockedCurrent") }}
              </div>
              <ConversationPickerDropdown
                v-else
                v-model="conversationId"
                :conversations="conversations"
                :disabled="busy"
              />
              <span class="block text-xs text-base-content/55">
                {{ lockedConversationId ? t("chat.timerCreate.conversationLockedHint") : t("chat.timerCreate.conversationHint") }}
              </span>
            </div>

            <div class="space-y-2">
              <span class="block text-sm font-medium">{{ t("chat.timerCreate.dispatchModeLabel") }}</span>
              <div class="flex flex-wrap items-center gap-4 text-sm">
                <label class="flex cursor-pointer items-center gap-2">
                  <input
                    v-model="dispatchMode"
                    class="radio radio-sm"
                    type="radio"
                    value="delegate"
                    :disabled="busy"
                  />
                  <span>{{ t("chat.timerCreate.dispatchModes.delegate") }}</span>
                </label>
                <label class="flex items-center gap-2" :class="isGlobalTarget ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'">
                  <input
                    v-model="dispatchMode"
                    class="radio radio-sm"
                    type="radio"
                    value="direct"
                    :disabled="busy || isGlobalTarget"
                  />
                  <span>{{ t("chat.timerCreate.dispatchModes.direct") }}</span>
                </label>
              </div>
              <span v-if="isGlobalTarget" class="block text-xs text-base-content/55">
                {{ t("chat.timerCreate.directDisabledHint") }}
              </span>
            </div>
          </div>

          <div class="space-y-4 border-t border-base-300 pt-4">
            <div class="grid gap-3 sm:grid-cols-2">
              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("chat.timerCreate.runAtLabel") }}</span>
                <input
                  v-model="runAt"
                  class="input input-bordered w-full"
                  type="datetime-local"
                  :disabled="busy"
                />
                <span class="block text-xs text-base-content/55">{{ t("chat.timerCreate.runAtHint") }}</span>
              </label>
              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("chat.timerCreate.endAtLabel") }}</span>
                <input
                  v-model="endAt"
                  class="input input-bordered w-full"
                  type="datetime-local"
                  :disabled="busy"
                />
                <span class="block text-xs text-base-content/55">{{ t("chat.timerCreate.endAtHint") }}</span>
              </label>
            </div>

            <label class="block space-y-2">
              <span class="block text-sm font-medium">{{ t("chat.timerCreate.scheduleModeLabel") }}</span>
              <SegmentedControl
                v-model="scheduleMode"
                :options="scheduleModeOptions"
                :disabled="busy"
                size="sm"
              />
            </label>

            <div v-if="scheduleMode === 'interval'" class="space-y-3">
              <CronPicker v-model="cron" :disabled="busy" />
            </div>
          </div>
          </div>
        </OverlayScrollArea>

        <div class="flex flex-wrap items-center justify-between gap-2 border-t border-base-300 px-5 py-3">
          <div class="flex items-center gap-2">
            <button
              v-if="isEditMode"
              type="button"
              class="btn btn-sm btn-error"
              :disabled="busy"
              @click="handleDelete"
            >
              {{ t("chat.timerCreate.delete") }}
            </button>
          </div>
          <div class="flex items-center gap-2">
            <button type="button" class="btn btn-sm btn-ghost" :disabled="busy" @click="requestClose">
              {{ t("common.cancel") }}
            </button>
            <button
              type="submit"
              class="btn btn-sm btn-primary"
              :disabled="busy || !canSubmit"
              :title="canSubmit ? undefined : t('chat.timerCreate.messageRequired')"
            >
              {{ busy ? t("common.saving") : t("common.save") }}
            </button>
          </div>
        </div>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="requestClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import CronPicker from "../../../shared/components/CronPicker.vue";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConversationPickerDropdown from "../../../shared/components/ConversationPickerDropdown.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import SegmentedControl from "../../../config/components/SegmentedControl.vue";
import { confirmDialog } from "../../../shared/services/confirm-dialog";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import type { UnarchivedConversationSummary } from "../../../../types/app";
import type { TimerDispatchMode, TimerEntry } from "../../utils/timer-entry";

type ScheduleMode = "once" | "interval";

const props = withDefaults(defineProps<{
  open: boolean;
  mode?: "create" | "edit";
  timer?: TimerEntry | null;
  /** 宿主给的默认目标会话：聊天窗口传当前会话，设置页留空即全局定时器 */
  defaultConversationId?: string;
}>(), {
  mode: "create",
  timer: null,
  defaultConversationId: "",
});

const emit = defineEmits<{
  (e: "close"): void;
  (e: "saved", timer: TimerEntry): void;
  (e: "deleted", timerId: string): void;
  /** 只拨了启用开关：宿主应刷新这一条，但别关弹窗 */
  (e: "enabledChange", timer: TimerEntry): void;
}>();

const { t } = useI18n();

const dialogRef = ref<HTMLDialogElement | null>(null);
const busy = ref(false);
const errorText = ref("");

const message = ref("");
const conversationId = ref("");
const conversations = ref<UnarchivedConversationSummary[]>([]);
const dispatchMode = ref<TimerDispatchMode>("delegate");
const runAt = ref("");
const endAt = ref("");

const scheduleMode = ref<ScheduleMode>("once");
const cron = ref("");

const isEditMode = computed(() => props.mode === "edit" && !!props.timer);
const dialogTitle = computed(() =>
  isEditMode.value ? t("chat.timerCreate.editTitle") : t("chat.timerCreate.title"),
);

/** 空目标会话＝全局定时器：没有前台会话可激活，后端只接受委托 */
const isGlobalTarget = computed(() => !conversationId.value.trim());

/** 会话内打开的新建：目标就是当前会话，界面不给改；编辑模式仍允许换目标 */
const lockedConversationId = computed(() => (
  isEditMode.value ? "" : String(props.defaultConversationId || "").trim()
));

const canSubmit = computed(() => message.value.trim().length > 0);

const scheduleModeOptions = computed(() => [
  { value: "once" as const, label: t("chat.timerCreate.scheduleModes.once") },
  { value: "interval" as const, label: t("chat.timerCreate.scheduleModes.interval") },
]);

watch(isGlobalTarget, (global) => {
  if (global) dispatchMode.value = "delegate";
});

/** 切到定时且还没有表达式时，先给一个看得懂的默认值 */
watch(scheduleMode, (mode) => {
  if (mode !== "interval") {
    return;
  }
  if (!cron.value.trim()) {
    cron.value = "0 9 * * *";
  }
});

/** 本地 RFC3339 → `datetime-local` 需要的 `YYYY-MM-DDTHH:mm` */
function toDateTimeLocalValue(value?: string | null): string {
  const raw = String(value ?? "").trim();
  if (!raw) return "";
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return "";
  const pad = (input: number) => String(input).padStart(2, "0");
  return `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}T${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`;
}

/** `datetime-local` → 带时区的 RFC3339；留空返回 null，表示沿用后端默认 */
function fromDateTimeLocalValue(value: string): string | null {
  const raw = String(value || "").trim();
  if (!raw) return null;
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return null;
  return parsed.toISOString();
}

function resetForm() {
  const timer = props.timer;
  errorText.value = "";
  if (isEditMode.value && timer) {
    message.value = timer.message;
    conversationId.value = timer.conversationId ?? "";
    dispatchMode.value = timer.dispatchMode;
    runAt.value = toDateTimeLocalValue(timer.runAt);
    endAt.value = toDateTimeLocalValue(timer.endAt);
    const existingCron = String(timer.cron ?? "").trim();
    cron.value = existingCron;
    scheduleMode.value = existingCron ? "interval" : "once";
    return;
  }
  message.value = "";
  // 新建时目标默认取宿主当前会话；没有会话上下文（设置页）就是全局定时器
  conversationId.value = String(props.defaultConversationId || "").trim();
  dispatchMode.value = conversationId.value ? "direct" : "delegate";
  runAt.value = toDateTimeLocalValue(new Date().toISOString());
  endAt.value = "";
  scheduleMode.value = "once";
  cron.value = "";
}

/** 目标会话下拉的数据源：未归档会话列表；取不到就留空，下拉里只剩「全局」 */
async function loadConversations() {
  try {
    const items = await invokeTauri<UnarchivedConversationSummary[]>("conversation.overview.list");
    conversations.value = Array.isArray(items) ? items : [];
  } catch {
    conversations.value = [];
  }
}

function syncDialog() {
  const dialog = dialogRef.value;
  if (!dialog) return;
  if (props.open) {
    if (!dialog.open) dialog.showModal();
    return;
  }
  if (dialog.open) dialog.close();
}

watch(() => props.open, (open) => {
  if (open) {
    resetForm();
    void loadConversations();
  }
  syncDialog();
}, { immediate: true });
watch(dialogRef, syncDialog);

function handleDialogClose() {
  if (!props.open) return;
  emit("close");
}

function requestClose() {
  if (busy.value) return;
  emit("close");
}

/**
 * 解析出要提交的 cron。
 * 定时器后端只认 cron 字符串，固定间隔必须换算；换算不出来的组合直接报错，
 * 不做近似，避免「设了每 2 天却在第 2、4、6… 天触发」这种静默改变语义。
 */
function resolveCronExpression(): { value: string } | { error: string } {
  if (scheduleMode.value !== "interval") {
    return { value: "" };
  }
  const normalized = cron.value.trim();
  if (!normalized) {
    return { error: t("chat.timerCreate.validation.cronRequired") };
  }
  return { value: normalized };
}

function buildInput() {
  // 后端 update 把 null/缺省视为「保持原值」，只有空串才是明确的「清空」。
  // 因此这里统一送字符串：用户清空的 cron / 截止时间 / 目标会话才会真正被清掉。
  const cronResult = resolveCronExpression();
  if ("error" in cronResult) {
    return null;
  }
  return {
    message: message.value.trim(),
    conversationId: conversationId.value.trim(),
    dispatchMode: dispatchMode.value,
    runAt: fromDateTimeLocalValue(runAt.value) ?? "",
    cron: cronResult.value,
    endAt: fromDateTimeLocalValue(endAt.value) ?? "",
  };
}

async function handleSubmit() {
  if (busy.value) return;
  if (!canSubmit.value) {
    errorText.value = t("chat.timerCreate.messageRequired");
    return;
  }
  const input = buildInput();
  if (!input) {
    return;
  }
  busy.value = true;
  errorText.value = "";
  try {
    if (isEditMode.value && props.timer) {
      const updated = await invokeTauri<TimerEntry>("timer.update", {
        input: { timerId: props.timer.id, ...input },
      });
      emit("saved", updated);
      emit("close");
      return;
    }
    const created = await invokeTauri<TimerEntry>("timer.create", { input });
    emit("saved", created);
    emit("close");
  } catch (error) {
    errorText.value = `${isEditMode.value ? t("chat.timerCreate.updateFailed") : t("chat.timerCreate.createFailed")}: ${toErrorMessage(error)}`;
  } finally {
    busy.value = false;
  }
}

/** 启用 / 停用：只改 enabled，其他字段原样保留 */
async function handleToggleEnabled() {
  if (busy.value || !props.timer) return;
  busy.value = true;
  errorText.value = "";
  try {
    const updated = await invokeTauri<TimerEntry>("timer.update", {
      input: { timerId: props.timer.id, enabled: !props.timer.enabled },
    });
    emit("enabledChange", updated);
  } catch (error) {
    errorText.value = `${t("chat.timerCreate.toggleFailed")}: ${toErrorMessage(error)}`;
  } finally {
    busy.value = false;
  }
}

async function handleDelete() {
  if (busy.value || !props.timer) return;
  const confirmed = await confirmDialog({
    message: t("config.timer.deleteConfirm"),
    confirmText: t("common.delete"),
    cancelText: t("common.cancel"),
  });
  if (!confirmed) return;
  busy.value = true;
  errorText.value = "";
  try {
    await invokeTauri("timer.delete", { input: { timerId: props.timer.id } });
    emit("deleted", props.timer.id);
    emit("close");
  } catch (error) {
    errorText.value = `${t("chat.timerCreate.deleteFailed")}: ${toErrorMessage(error)}`;
  } finally {
    busy.value = false;
  }
}
</script>

