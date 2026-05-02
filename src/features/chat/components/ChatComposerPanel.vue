<template>
  <div>
    <ChatQueuePreview
      :queue-events="visibleQueueEvents"
      :session-state="sessionState"
      @recall-to-input="handleRecallToInput"
      @mark-guided="markGuided"
    />

    <div
      v-if="linkOpenErrorText"
      class="alert alert-warning mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-24 overflow-auto"
    >
      <span>{{ linkOpenErrorText }}</span>
    </div>
    <div
      v-if="selectionModeEnabled"
      class="rounded-box border border-base-300 bg-base-100 px-3 py-3"
    >
      <div class="text-xs opacity-70">已选择 {{ selectedMessageCount }} 条消息</div>
      <div class="mt-3 flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="btn btn-sm"
          :disabled="selectedMessageCount === 0"
          @click="emit('selectionActionBranch')"
        >
          创造会话分支
        </button>
        <button
          type="button"
          class="btn btn-sm"
          :class="{ 'btn-primary': selectionDeliverCardOpen }"
          :disabled="selectedMessageCount === 0 || selectionDeliverTargetOptions.length === 0"
          @click="openSelectionDeliverCard"
        >
          转发到会话
        </button>
        <button
          type="button"
          class="btn btn-sm"
          :class="{ 'btn-primary': selectionDelegateCardOpen }"
          :disabled="selectedMessageCount === 0 || delegateDepartmentOptions.length === 0"
          @click="openSelectionDelegateCard"
        >
          发起委托
        </button>
        <button
          type="button"
          class="btn btn-sm"
          :disabled="selectedMessageCount === 0"
          @click="emit('selectionActionCopy')"
        >
          复制
        </button>
        <button
          type="button"
          class="btn btn-sm"
          :class="{ 'btn-primary': selectionShareCardOpen }"
          :disabled="selectedMessageCount === 0"
          @click="openSelectionShareCard"
        >
          分享
        </button>
        <button
          type="button"
          class="btn btn-sm btn-ghost ml-auto"
          @click="handleExitSelectionMode"
        >
          取消
        </button>
      </div>
      <div
        v-if="selectionDeliverCardOpen"
        class="mt-3 rounded-box border border-base-300 bg-base-200/50 px-3 py-3"
      >
        <div class="text-sm font-medium">转发到会话</div>
        <div class="mt-1 text-xs opacity-70">会把当前选中的原消息插入到目标会话末尾；如果目标会话正在流式输出，会直接失败。</div>
        <select
          v-model="selectionDeliverTargetConversationId"
          class="select select-bordered select-sm mt-3 w-full"
          :disabled="selectionDeliverTargetOptions.length === 0"
        >
          <option
            v-for="item in selectionDeliverTargetOptions"
            :key="item.conversationId"
            :value="item.conversationId"
          >
            {{ selectionDeliverOptionLabel(item) }}
          </option>
        </select>
        <div class="mt-3 flex items-center justify-end gap-2">
          <button
            type="button"
            class="btn btn-sm"
            @click="closeSelectionDeliverCard"
          >
            取消
          </button>
          <button
            type="button"
            class="btn btn-sm btn-primary"
            :disabled="!selectionDeliverTargetConversationId"
            @click="confirmSelectionDeliver"
          >
            确定转发
          </button>
        </div>
      </div>
      <div
        v-if="selectionDelegateCardOpen"
        class="mt-3 rounded-box border border-base-300 bg-base-200/50 px-3 py-3"
      >
        <div class="flex items-center justify-between gap-3">
          <div>
            <div class="text-sm font-medium">发起异步委托</div>
            <div class="mt-1 text-xs opacity-70">选中消息会作为纯文本背景发送给子代理。</div>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <span class="text-sm opacity-70">快捷委托</span>
            <button type="button" class="btn btn-sm" @click="applyDelegateReviewPreset">审查</button>
            <button type="button" class="btn btn-sm btn-ghost" @click="clearSelectionDelegateFields">清空</button>
          </div>
        </div>
        <div v-if="recentDelegateRequests.length > 0" class="mt-3 flex flex-wrap gap-2">
          <button
            v-for="item in recentDelegateRequests"
            :key="item.id"
            type="button"
            class="btn btn-xs max-w-full justify-start"
            :title="item.question"
            @click="applyRecentDelegateRequest(item)"
          >
            <span class="max-w-52 truncate">{{ item.label }}</span>
          </button>
        </div>
        <select v-model="selectionDelegateDepartmentId" class="select select-bordered select-sm mt-3 w-full">
          <option v-for="department in delegateDepartmentOptions" :key="department.id" :value="department.id">
            {{ selectionDelegateDepartmentLabel(department) }}
          </option>
        </select>
        <textarea
          v-model="selectionDelegateBackground"
          class="textarea textarea-bordered mt-3 min-h-16 w-full resize-y text-sm"
          placeholder="补充背景（可选）"
        ></textarea>
        <textarea
          v-model="selectionDelegateQuestion"
          class="textarea textarea-bordered mt-2 min-h-20 w-full resize-y text-sm"
          placeholder="要子代理查清的问题"
        ></textarea>
        <textarea
          v-model="selectionDelegateFocus"
          class="textarea textarea-bordered mt-2 min-h-20 w-full resize-y text-sm"
          placeholder="搜索/调查重点（可选）"
        ></textarea>
        <div class="mt-3 flex items-center justify-end gap-2">
          <button type="button" class="btn btn-sm" @click="closeSelectionDelegateCard">取消</button>
          <button
            type="button"
            class="btn btn-sm btn-primary"
            :disabled="!canSubmitSelectionDelegate"
            @click="confirmSelectionDelegate"
          >
            发起委托
          </button>
        </div>
      </div>
      <div
        v-if="selectionShareCardOpen"
        class="mt-3 rounded-box border border-base-300 bg-base-200/50 px-3 py-3"
      >
        <div class="text-sm font-medium">分享</div>
        <div class="mt-1 text-xs opacity-70">把当前选中的消息导出为可分享文件。</div>
        <div class="mt-3 flex flex-wrap items-center gap-2">
          <button type="button" class="btn btn-sm btn-primary" @click="confirmSelectionShare('png')">导出图片</button>
          <button type="button" class="btn btn-sm" @click="confirmSelectionShare('html')">导出 HTML</button>
          <button type="button" class="btn btn-sm btn-ghost ml-auto" @click="closeSelectionShareCard">取消</button>
        </div>
      </div>
    </div>
    <template v-else>
    <div v-if="clipboardImages.length > 0 || queuedAttachmentNotices.length > 0" class="mb-2 flex flex-wrap gap-1">
      <div v-for="(img, idx) in clipboardImages" :key="`${img.mime}-${idx}`" class="badge badge-ghost gap-1 py-3">
        <ImageIcon v-if="isImageMime(img.mime)" class="h-3.5 w-3.5" />
        <FileText v-else-if="isPdfMime(img.mime)" class="h-3.5 w-3.5" />
        <ImageIcon v-else class="h-3.5 w-3.5" />
        <span class="text-[11px]">{{ isPdfMime(img.mime) ? `PDF ${idx + 1}` : t("chat.image", { index: idx + 1 }) }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="emit('removeClipboardImage', idx)">
          <X class="h-3 w-3" />
        </button>
      </div>
      <div
        v-for="(file, idx) in queuedAttachmentNotices"
        :key="file.id"
        class="badge badge-ghost gap-1 py-3"
      >
        <FileText class="h-3.5 w-3.5" />
        <span class="text-[11px]">{{ file.fileName }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="emit('removeQueuedAttachmentNotice', idx)">
          <X class="h-3 w-3" />
        </button>
      </div>
    </div>
    <div v-if="transcribing" class="mb-1 text-[11px] opacity-80 flex items-center gap-1">
      <span class="loading loading-spinner loading-sm"></span>
      <span>{{ t("chat.transcribing") }}</span>
    </div>
    <div v-if="selectedMentions.length > 0" class="mb-2 flex flex-wrap gap-1">
      <span
        v-for="item in selectedMentions"
        :key="item.agentId"
        class="badge gap-1 bg-base-300 px-3 py-3 text-sm text-base-content border-transparent"
      >
        <span class="max-w-24 truncate leading-none">@{{ item.agentName }}</span>
        <button
          type="button"
          class="ml-0.5 inline-flex h-5 w-5 items-center justify-center rounded-full text-base-content transition hover:bg-error hover:text-error-content"
          :disabled="chatting || frozen"
          @click.stop="removeSelectedMention(item.agentId)"
        >
          <X class="h-3 w-3" />
        </button>
      </span>
    </div>
    <div v-if="selectedInstructionPrompts.length > 0" class="mb-2 flex flex-wrap gap-1">
      <div
        v-for="item in selectedInstructionPrompts"
        :key="item.id"
        class="badge badge-outline gap-1 py-3"
      >
        <Layers2 class="h-3.5 w-3.5" />
        <span class="max-w-48 truncate text-[11px]" :title="item.prompt">{{ item.prompt }}</span>
        <button class="btn btn-ghost btn-sm btn-square" :disabled="chatting || frozen" @click="removeSelectedInstructionPreset(item.id)">
          <X class="h-3 w-3" />
        </button>
      </div>
    </div>
    <div ref="composerRootRef" class="flex flex-col">
      <div v-if="instructionPanelOpen" class="flex flex-wrap content-start gap-2 max-h-48 overflow-y-auto">
        <button
          v-for="(item, index) in normalizedInstructionPresets"
          :key="item.id"
          type="button"
          class="btn btn-sm min-h-0 max-w-full justify-start normal-case px-3"
          :class="instructionFocusIndex === index ? 'btn-primary' : 'btn-ghost'"
          :title="item.prompt"
          @click="applyInstructionPreset(item)"
        >
          <span class="block max-w-64 truncate text-left text-sm sm:max-w-80">{{ item.prompt }}</span>
        </button>
        <div v-if="normalizedInstructionPresets.length === 0" class="w-full px-2 py-3 text-sm opacity-60">
          {{ t("chat.noInstructionPresets") }}
        </div>
      </div>
      <div class="relative">
        <textarea
          ref="chatInputRef"
          v-model="localChatInput"
          class="w-full textarea resize-none overflow-y-auto chat-input-no-focus scrollbar-gutter-stable min-h-8"
          rows="1"
          :disabled="frozen"
          :placeholder="chatInputPlaceholder"
          @input="handleChatInputInput"
          @keydown="handleChatInputKeydown"
        ></textarea>
      </div>
      <Teleport to="body">
        <div
          v-if="mentionPanelOpen"
          class="fixed z-1200"
          :style="mentionPanelStyle"
        >
          <div class="dropdown-content mt-2 w-max max-w-[min(80vw,20rem)] overflow-hidden rounded-box border border-base-300 bg-base-100 p-1 shadow-xl">
            <ul class="flex flex-col gap-1">
              <li
                v-for="(item, index) in filteredMentionOptions"
                :key="`${item.agentId}:${item.departmentId}`"
              >
                <button
                  type="button"
                  class="flex min-h-0 w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors hover:bg-base-200/80"
                  :class="mentionFocusIndex === index ? 'bg-base-200' : ''"
                  @click="applyMention(item)"
                >
                  <div class="indicator shrink-0">
                    <span
                      v-if="isMentionSelected(item.agentId)"
                      class="indicator-item inline-flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[9px] font-bold text-primary-content"
                    >
                      @
                    </span>
                    <div class="avatar">
                      <div class="w-7 rounded-full">
                        <img
                          v-if="item.avatarUrl"
                          :src="item.avatarUrl"
                          :alt="item.agentName"
                          class="w-7 h-7 rounded-full object-cover"
                        />
                        <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-[10px]">
                          {{ avatarInitial(item.agentName) }}
                        </div>
                      </div>
                    </div>
                  </div>
                  <span class="min-w-0 flex-1 truncate pr-0.5 text-sm leading-5">{{ item.agentName }}</span>
                </button>
              </li>
            </ul>
            <div v-if="filteredMentionOptions.length === 0" class="px-2.5 py-2 text-sm opacity-60">
              {{ t("chat.noMentionCandidates") }}
            </div>
          </div>
        </div>
      </Teleport>
      <div class="mt-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-2">
          <button
            class="btn btn-sm btn-circle btn-ghost shrink-0"
            :disabled="chatting || frozen"
            :title="t('chat.command')"
            @click="toggleInstructionPanel"
          >
            <Layers2 class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-circle btn-ghost shrink-0"
            :disabled="chatting || frozen"
            :title="t('chat.attach')"
            @click="emit('pickAttachments')"
          >
            <Paperclip class="h-3.5 w-3.5" />
          </button>
          <button
            class="btn btn-sm btn-circle shrink-0"
            :class="recording ? 'btn-error' : 'btn-ghost'"
            :disabled="!canRecord || chatting || frozen"
            :title="recording ? t('chat.recording', { seconds: Math.max(1, Math.round(recordingMs / 1000)) }) : t('chat.holdRecord', { hotkey: recordHotkey })"
            @mousedown.prevent="emit('startRecording')"
            @mouseup.prevent="emit('stopRecording')"
            @mouseleave.prevent="recording && emit('stopRecording')"
            @touchstart.prevent="emit('startRecording')"
            @touchend.prevent="emit('stopRecording')"
          >
            <Mic class="h-3.5 w-3.5" />
          </button>
          <select
            class="select select-ghost select-sm h-8 min-h-8 w-44 max-w-44"
            :value="selectedChatModelId"
            :disabled="chatting || frozen || normalizedChatModelOptions.length === 0"
            title="首要模型"
            @change="handleChatModelChange"
          >
            <option
              v-for="item in normalizedChatModelOptions"
              :key="item.id"
              :value="item.id"
            >
              {{ item.name }}
            </option>
          </select>
        </div>
        <div class="flex items-center gap-2">
          <span
            v-if="planModeEnabled"
            class="badge badge-sm badge-info shrink-0 select-none"
            :title="`Shift+Tab ${t('chat.plan.mode')}`"
          >
            {{ t("chat.plan.mode") }}
          </span>
          <button
            class="btn btn-sm btn-circle shrink-0"
            :class="showStopAction ? 'btn-error' : 'btn-success'"
            :disabled="frozen || busy || (showStopAction && !!stopChatDisabled)"
            :title="showStopAction ? `${t('chat.stop')} / ${t('chat.stopReplying')}` : t('chat.send')"
            @click="showStopAction ? emit('stopChat') : handleSendChat()"
          >
            <Square v-if="showStopAction" class="h-3.5 w-3.5 fill-current" />
            <Send v-else class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
    </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FileText, Image as ImageIcon, Layers2, Mic, Paperclip, Send, Square, X } from "lucide-vue-next";
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMentionTarget, PromptCommandPreset, SkillListResult } from "../../../types/app";
import { invokeTauri } from "../../../services/tauri-api";
import ChatQueuePreview from "./ChatQueuePreview.vue";
import { useChatQueue } from "../composables/use-chat-queue";

type BinaryAttachment = { mime: string; bytesBase64: string };
type QueuedAttachmentNotice = { id: string; fileName: string; relativePath: string; mime: string };
type ConversationDepartmentOption = {
  id: string;
  name: string;
  ownerAgentId?: string;
  ownerName: string;
  providerName?: string;
  modelName?: string;
};

const props = defineProps<{
  selectionModeEnabled: boolean;
  selectedMessageCount: number;
  chatInput: string;
  instructionPresets: PromptCommandPreset[];
  mentionOptions: ChatMentionTarget[];
  selectedMentions: ChatMentionTarget[];
  chatInputPlaceholder: string;
  clipboardImages: BinaryAttachment[];
  queuedAttachmentNotices: QueuedAttachmentNotice[];
  linkOpenErrorText: string;
  chatErrorText: string;
  transcribing: boolean;
  canRecord: boolean;
  recording: boolean;
  recordingMs: number;
  recordHotkey: string;
  selectedChatModelId: string;
  chatModelOptions: ApiConfigItem[];
  planModeEnabled: boolean;
  chatting: boolean;
  frontendRoundPhase?: "idle" | "queued" | "waiting" | "streaming";
  busy: boolean;
  stopChatDisabled?: boolean;
  frozen: boolean;
  showSideConversationList: boolean;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  createConversationDepartmentOptions: ConversationDepartmentOption[];
  defaultCreateConversationDepartmentId: string;
}>();

const emit = defineEmits<{
  (e: "exitSelectionMode"): void;
  (e: "selectionActionBranch"): void;
  (e: "selectionActionForward", targetConversationId: string): void;
  (e: "selectionActionDelegate", payload: { departmentId: string; presetId: string; background: string; question: string; focus: string }): void;
  (e: "selectionActionCopy"): void;
  (e: "selectionActionShare", format: "html" | "png"): void;
  (e: "update:chatInput", value: string): void;
  (e: "update:selectedInstructionPrompts", value: PromptCommandPreset[]): void;
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", agentId: string): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "startRecording"): void;
  (e: "stopRecording"): void;
  (e: "pickAttachments"): void;
  (e: "update:selectedChatModelId", value: string): void;
  (e: "update:planModeEnabled", value: boolean): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
}>();

const { t } = useI18n();
const { queueEvents, sessionState, recallQueueEvent, markGuided } = useChatQueue();

const visibleQueueEvents = computed(() => {
  const activeConversationId = String(props.activeConversationId || "").trim();
  if (!activeConversationId) return [];
  return queueEvents.value.filter(
    (event) => String(event.conversationId || "").trim() === activeConversationId,
  );
});

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;
const USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY = "easy_call.user_async_delegate_recent.v1";
const USER_ASYNC_DELEGATE_RECENT_LIMIT = 3;
const DELEGATE_REVIEW_FALLBACK_BACKGROUND = [
  "请严格遵守内置 code-review skill 执行审查。",
  "",
  "核心要求：",
  "- 审查当前工作区代码改动，而不是审查单条工具调用。",
  "- 先拿到明确 diff，再做缺陷判断。",
  "- 只报告真实、可复现、会影响正确性/稳定性/安全性的缺陷。",
  "- 不要把风格建议、命名偏好或无法从 diff 证明的推测当成缺陷。",
  "",
  "补充输出约束：当前任务是用户手动发起的异步委托，结果会直接写回原会话；请用自然语言报告审查结论，不要输出工具审查 JSON，除非用户明确要求 JSON。",
].join("\n");
let delegateReviewBackgroundCache = "";

const composerRootRef = ref<HTMLDivElement | null>(null);
const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const chatInputHistoryApplying = ref(false);
const resizeInputRaf = ref(0);
const instructionPanelOpen = ref(false);
const instructionFocusIndex = ref(0);
const selectedInstructionPrompts = ref<PromptCommandPreset[]>([]);
const mentionPanelOpen = ref(false);
const mentionQuery = ref("");
const mentionFocusIndex = ref(0);
const mentionRange = ref<{ start: number; end: number } | null>(null);
const selectionDeliverCardOpen = ref(false);
const selectionDeliverTargetConversationId = ref("");
const selectionDelegateCardOpen = ref(false);
const selectionShareCardOpen = ref(false);
const selectionDelegateDepartmentId = ref("");
const selectionDelegatePresetId = ref("review");
const selectionDelegateBackground = ref("");
const selectionDelegateQuestion = ref("");
const selectionDelegateFocus = ref("");
type RecentDelegateRequest = {
  id: string;
  label: string;
  departmentId: string;
  presetId: string;
  background: string;
  question: string;
  focus: string;
};
const recentDelegateRequests = ref<RecentDelegateRequest[]>([]);
const mentionPanelStyle = ref<Record<string, string>>({
  left: "0px",
  top: "0px",
  transform: "translateY(calc(-100% - 8px))",
});

const normalizedInstructionPresets = computed(() =>
  (Array.isArray(props.instructionPresets) ? props.instructionPresets : [])
    .map((item) => ({
      id: String(item?.id || "").trim(),
      name: String(item?.prompt || item?.name || "").trim(),
      prompt: String(item?.prompt || item?.name || "").trim(),
    }))
    .filter((item) => !!item.id && !!item.prompt),
);
const normalizedChatModelOptions = computed(() =>
  (Array.isArray(props.chatModelOptions) ? props.chatModelOptions : [])
    .map((item) => ({
      id: String(item?.id || "").trim(),
      name: String(item?.name || "").trim(),
    }))
    .filter((item) => !!item.id && !!item.name),
);

const showStopAction = computed(() =>
  props.chatting || ["queued", "waiting", "streaming"].includes(String(props.frontendRoundPhase || "idle")),
);
const selectedMentions = computed(() =>
  (Array.isArray(props.selectedMentions) ? props.selectedMentions : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      departmentId: String(item?.departmentId || "").trim(),
      departmentName: String(item?.departmentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.departmentId && !!item.agentName),
);
const filteredMentionOptions = computed(() => {
  return (Array.isArray(props.mentionOptions) ? props.mentionOptions : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      departmentId: String(item?.departmentId || "").trim(),
      departmentName: String(item?.departmentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.departmentId && !!item.agentName);
});
const selectionDeliverTargetOptions = computed(() =>
  (Array.isArray(props.unarchivedConversationItems) ? props.unarchivedConversationItems : [])
    .filter((item) => String(item.conversationId || "").trim() !== String(props.activeConversationId || "").trim())
    .map((item) => ({
      conversationId: String(item.conversationId || "").trim(),
      title: String(item.title || "").trim() || "未命名会话",
      departmentName: String(item.departmentName || "").trim() || undefined,
      runtimeState: item.runtimeState,
    }))
    .filter((item) => !!item.conversationId),
);
const activeConversationAgentId = computed(() => {
  const activeConversationId = String(props.activeConversationId || "").trim();
  if (!activeConversationId) return "";
  return String(
    (props.unarchivedConversationItems || []).find(
      (item) => String(item.conversationId || "").trim() === activeConversationId,
    )?.agentId || "",
  ).trim();
});
const delegateDepartmentOptions = computed(() =>
  (Array.isArray(props.createConversationDepartmentOptions) ? props.createConversationDepartmentOptions : [])
    .map((item) => ({
      id: String(item.id || "").trim(),
      name: String(item.name || "").trim() || String(item.id || "").trim(),
      ownerAgentId: String(item.ownerAgentId || "").trim() || undefined,
      ownerName: String(item.ownerName || "").trim(),
      providerName: String(item.providerName || "").trim() || undefined,
      modelName: String(item.modelName || "").trim() || undefined,
    }))
    .filter((item) => {
      if (!item.id) return false;
      const activeAgentId = activeConversationAgentId.value;
      return !activeAgentId || item.ownerAgentId !== activeAgentId;
    }),
);
const preferredDelegateDepartmentId = computed(() => {
  return String(delegateDepartmentOptions.value[0]?.id || "").trim();
});
const canSubmitSelectionDelegate = computed(() =>
  props.selectedMessageCount > 0
  && delegateDepartmentOptions.value.some(
    (department) => department.id === String(selectionDelegateDepartmentId.value || "").trim(),
  )
  && !!String(selectionDelegateQuestion.value || "").trim(),
);
const planModeToggleAllowed = computed(() => !props.chatting && !props.frozen);

function selectionDeliverOptionLabel(item: {
  title: string;
  departmentName?: string;
  runtimeState?: ChatConversationOverviewItem["runtimeState"];
}): string {
  const parts = [String(item.title || "").trim() || "未命名会话"];
  const departmentName = String(item.departmentName || "").trim();
  if (departmentName) parts.push(departmentName);
  if (item.runtimeState === "assistant_streaming") parts.push("流式中");
  if (item.runtimeState === "organizing_context") parts.push("整理中");
  return parts.join(" / ");
}

function openSelectionDeliverCard() {
  if (selectionDeliverTargetOptions.value.length === 0) return;
  closeSelectionDelegateCard();
  closeSelectionShareCard();
  const currentTargetConversationId = String(selectionDeliverTargetConversationId.value || "").trim();
  const hasValidTarget = selectionDeliverTargetOptions.value.some(
    (item) => item.conversationId === currentTargetConversationId,
  );
  if (!currentTargetConversationId || !hasValidTarget) {
    selectionDeliverTargetConversationId.value = selectionDeliverTargetOptions.value[0]?.conversationId || "";
  }
  selectionDeliverCardOpen.value = true;
}

function closeSelectionDeliverCard() {
  selectionDeliverCardOpen.value = false;
}

function confirmSelectionDeliver() {
  const targetConversationId = String(selectionDeliverTargetConversationId.value || "").trim();
  if (!targetConversationId) return;
  closeSelectionDeliverCard();
  emit("selectionActionForward", targetConversationId);
}

function selectionDelegateDepartmentLabel(item: ConversationDepartmentOption): string {
  const parts = [String(item.name || "").trim() || String(item.id || "").trim()];
  const ownerName = String(item.ownerName || "").trim();
  if (ownerName) parts.push(ownerName);
  const modelName = String(item.modelName || "").trim();
  if (modelName) parts.push(modelName);
  return parts.join(" / ");
}

function delegateReviewPreset() {
  return {
    presetId: "review",
    background: delegateReviewBackgroundCache || DELEGATE_REVIEW_FALLBACK_BACKGROUND,
    question: "请结合选中消息，按内置 code-review 审查规则检查当前工作区代码改动。",
    focus: "先确认明确 diff，再只报告真实、可复现、会影响正确性/稳定性/安全性的缺陷；没有确认到缺陷时直接说明未发现。",
  };
}

async function loadDelegateReviewBackground(): Promise<string> {
  if (delegateReviewBackgroundCache.trim()) return delegateReviewBackgroundCache;
  try {
    const result = await invokeTauri<SkillListResult>("mcp_list_skills");
    const skill = (result.skills || []).find((item) => String(item.name || "").trim() === "code-review");
    const content = String(skill?.content || "").trim();
    if (content) {
      delegateReviewBackgroundCache = [
        "请严格遵守以下 code-review skill 内容：",
        "",
        content,
        "",
        "补充输出约束：当前任务是用户手动发起的异步委托，结果会直接写回原会话；请用自然语言报告审查结论，不要输出工具审查 JSON，除非用户明确要求 JSON。",
      ].join("\n");
      return delegateReviewBackgroundCache;
    }
  } catch (error) {
    console.error("[用户异步委托][前端] 读取 code-review skill 失败", error);
  }
  delegateReviewBackgroundCache = DELEGATE_REVIEW_FALLBACK_BACKGROUND;
  return delegateReviewBackgroundCache;
}

function normalizeRecentDelegateRequest(raw: unknown): RecentDelegateRequest | null {
  const item = raw as Partial<RecentDelegateRequest> | null;
  if (!item) return null;
  const departmentId = String(item.departmentId || "").trim();
  const question = String(item.question || "").trim();
  const focus = String(item.focus || "").trim();
  if (!departmentId || !question) return null;
  const presetId = String(item.presetId || "review").trim() || "review";
  const label = String(item.label || question).trim() || question;
  return {
    id: String(item.id || `${departmentId}:${presetId}:${question}`).trim(),
    label,
    departmentId,
    presetId,
    background: String(item.background || "").trim(),
    question,
    focus,
  };
}

function loadRecentDelegateRequests() {
  try {
    const raw = window.localStorage.getItem(USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    const normalized: RecentDelegateRequest[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
      const request = normalizeRecentDelegateRequest(item);
      if (!request) continue;
      const key = `${request.departmentId}\n${request.presetId}\n${request.background}\n${request.question}\n${request.focus}`;
      if (seen.has(key)) continue;
      seen.add(key);
      normalized.push(request);
      if (normalized.length >= USER_ASYNC_DELEGATE_RECENT_LIMIT) break;
    }
    recentDelegateRequests.value = normalized;
  } catch {
    recentDelegateRequests.value = [];
  }
}

function saveRecentDelegateRequests() {
  try {
    window.localStorage.setItem(USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY, JSON.stringify(recentDelegateRequests.value));
  } catch {
    // ignore persistence failures
  }
}

function rememberDelegateRequest(raw: Omit<RecentDelegateRequest, "id" | "label">) {
  const request = normalizeRecentDelegateRequest({
    ...raw,
    id: `${Date.now()}:${raw.departmentId}`,
    label: raw.question,
  });
  if (!request) return;
  const key = `${request.departmentId}\n${request.presetId}\n${request.background}\n${request.question}\n${request.focus}`;
  recentDelegateRequests.value = [
    request,
    ...recentDelegateRequests.value.filter((item) =>
      `${item.departmentId}\n${item.presetId}\n${item.background}\n${item.question}\n${item.focus}` !== key
    ),
  ].slice(0, USER_ASYNC_DELEGATE_RECENT_LIMIT);
  saveRecentDelegateRequests();
}

async function applyDelegateReviewPreset() {
  const preset = delegateReviewPreset();
  selectionDelegatePresetId.value = preset.presetId;
  selectionDelegateBackground.value = preset.background;
  selectionDelegateQuestion.value = preset.question;
  selectionDelegateFocus.value = preset.focus;
  selectionDelegateBackground.value = await loadDelegateReviewBackground();
}

function clearSelectionDelegateFields() {
  selectionDelegatePresetId.value = "review";
  selectionDelegateBackground.value = "";
  selectionDelegateQuestion.value = "";
  selectionDelegateFocus.value = "";
}

function applyRecentDelegateRequest(item: RecentDelegateRequest) {
  const departmentStillExists = delegateDepartmentOptions.value.some((department) => department.id === item.departmentId);
  if (departmentStillExists) {
    selectionDelegateDepartmentId.value = item.departmentId;
  }
  selectionDelegatePresetId.value = item.presetId || "review";
  selectionDelegateBackground.value = item.background;
  selectionDelegateQuestion.value = item.question;
  selectionDelegateFocus.value = item.focus;
}

function openSelectionDelegateCard() {
  closeSelectionDeliverCard();
  closeSelectionShareCard();
  const preferredDepartmentId = preferredDelegateDepartmentId.value;
  if (preferredDepartmentId) {
    selectionDelegateDepartmentId.value = preferredDepartmentId;
  }
  selectionDelegateCardOpen.value = true;
}

function closeSelectionDelegateCard() {
  selectionDelegateCardOpen.value = false;
}

function openSelectionShareCard() {
  if (props.selectedMessageCount <= 0) return;
  closeSelectionDeliverCard();
  closeSelectionDelegateCard();
  selectionShareCardOpen.value = true;
}

function closeSelectionShareCard() {
  selectionShareCardOpen.value = false;
}

function confirmSelectionShare(format: "html" | "png") {
  closeSelectionShareCard();
  emit("selectionActionShare", format);
}

function confirmSelectionDelegate() {
  if (!canSubmitSelectionDelegate.value) return;
  const payload = {
    departmentId: String(selectionDelegateDepartmentId.value || "").trim(),
    presetId: String(selectionDelegatePresetId.value || "review").trim() || "review",
    background: String(selectionDelegateBackground.value || "").trim(),
    question: String(selectionDelegateQuestion.value || "").trim(),
    focus: String(selectionDelegateFocus.value || "").trim(),
  };
  rememberDelegateRequest(payload);
  closeSelectionDelegateCard();
  emit("selectionActionDelegate", payload);
}

function handleExitSelectionMode() {
  closeSelectionDeliverCard();
  closeSelectionDelegateCard();
  closeSelectionShareCard();
  emit("exitSelectionMode");
}

function loadChatInputHistory() {
  try {
    const raw = window.localStorage.getItem(CHAT_INPUT_HISTORY_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    const normalized: string[] = [];
    const seen = new Set<string>();
    for (const item of parsed) {
      const text = String(item || "").trim();
      if (!text || seen.has(text)) continue;
      seen.add(text);
      normalized.push(text);
      if (normalized.length >= CHAT_INPUT_HISTORY_LIMIT) break;
    }
    chatInputHistory.value = normalized;
  } catch {
    chatInputHistory.value = [];
  }
}

function saveChatInputHistory() {
  try {
    window.localStorage.setItem(CHAT_INPUT_HISTORY_STORAGE_KEY, JSON.stringify(chatInputHistory.value));
  } catch {
    // ignore persistence failures
  }
}

function pushChatInputHistory(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  chatInputHistory.value = [text, ...chatInputHistory.value.filter((item) => item !== text)].slice(0, CHAT_INPUT_HISTORY_LIMIT);
  saveChatInputHistory();
  chatInputHistoryCursor.value = -1;
  chatInputHistoryDraft.value = "";
}

function emitSelectedInstructionPrompts() {
  emit("update:selectedInstructionPrompts", selectedInstructionPrompts.value);
}

function openInstructionPanel() {
  instructionPanelOpen.value = true;
  if (instructionFocusIndex.value >= normalizedInstructionPresets.value.length) {
    instructionFocusIndex.value = Math.max(0, normalizedInstructionPresets.value.length - 1);
  }
}

function closeInstructionPanel() {
  instructionPanelOpen.value = false;
}

function closeMentionPanel() {
  mentionPanelOpen.value = false;
  mentionQuery.value = "";
  mentionFocusIndex.value = 0;
  mentionRange.value = null;
}

function refreshMentionPanelPosition() {
  const el = chatInputRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  mentionPanelStyle.value = {
    left: `${Math.round(rect.left)}px`,
    top: `${Math.round(rect.top)}px`,
    transform: "translateY(calc(-100% - 8px))",
  };
}

function toggleInstructionPanel() {
  if (instructionPanelOpen.value) {
    closeInstructionPanel();
    return;
  }
  openInstructionPanel();
}

function applyInstructionPreset(item: PromptCommandPreset | undefined) {
  if (!item) return;
  if (!selectedInstructionPrompts.value.some((entry) => entry.id === item.id)) {
    selectedInstructionPrompts.value = [...selectedInstructionPrompts.value, item];
    emitSelectedInstructionPrompts();
  }
  closeInstructionPanel();
}

function selectInstructionPresetByIndex(index: number) {
  const list = normalizedInstructionPresets.value;
  if (list.length === 0) return;
  const nextIndex = Math.max(0, Math.min(list.length - 1, index));
  instructionFocusIndex.value = nextIndex;
  applyInstructionPreset(list[nextIndex]);
}

function moveInstructionFocus(delta: number) {
  const list = normalizedInstructionPresets.value;
  if (list.length === 0) return;
  const next = instructionFocusIndex.value + delta;
  instructionFocusIndex.value = Math.max(0, Math.min(list.length - 1, next));
}

function removeSelectedInstructionPreset(id: string) {
  selectedInstructionPrompts.value = selectedInstructionPrompts.value.filter((item) => item.id !== id);
  emitSelectedInstructionPrompts();
}

function clearSelectedInstructionPrompts() {
  if (selectedInstructionPrompts.value.length === 0) return;
  selectedInstructionPrompts.value = [];
  emitSelectedInstructionPrompts();
}

function removeSelectedMention(agentId: string) {
  emit("removeMention", agentId);
  closeMentionPanel();
}

function applyMention(item: ChatMentionTarget | undefined) {
  if (!item || !mentionRange.value) return;
  const current = String(localChatInput.value || "");
  const before = current.slice(0, mentionRange.value.start);
  const after = current.slice(mentionRange.value.end);
  const nextValue = `${before}${after}`;
  localChatInput.value = nextValue;
  if (selectedMentions.value.some((entry) => entry.agentId === item.agentId)) {
    emit("removeMention", item.agentId);
  } else {
    emit("addMention", item);
  }
  closeMentionPanel();
  nextTick(() => {
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = Math.min(before.length, nextValue.length);
    el.focus();
    el.setSelectionRange(cursor, cursor);
    scheduleResizeChatInput();
  });
}

function selectMentionByIndex(index: number) {
  const list = filteredMentionOptions.value;
  if (list.length === 0) return;
  const nextIndex = Math.max(0, Math.min(list.length - 1, index));
  mentionFocusIndex.value = nextIndex;
  applyMention(list[nextIndex]);
}

function moveMentionFocus(delta: number) {
  const list = filteredMentionOptions.value;
  if (list.length === 0) return;
  const next = mentionFocusIndex.value + delta;
  mentionFocusIndex.value = Math.max(0, Math.min(list.length - 1, next));
}

function updateMentionState() {
  const el = chatInputRef.value;
  if (!el || el.selectionStart !== el.selectionEnd) {
    closeMentionPanel();
    return;
  }
  const value = String(localChatInput.value || "");
  const cursor = el.selectionStart ?? value.length;
  const beforeCursor = value.slice(0, cursor);
  const match = beforeCursor.match(/(?:^|\s)@$/);
  if (!match) {
    closeMentionPanel();
    return;
  }
  mentionQuery.value = "";
  const queryStart = cursor - 1;
  mentionRange.value = { start: queryStart, end: cursor };
  refreshMentionPanelPosition();
  mentionPanelOpen.value = true;
  if (mentionFocusIndex.value >= filteredMentionOptions.value.length) {
    mentionFocusIndex.value = 0;
  }
}

function handleChatModelChange(event: Event) {
  const value = String((event.target as HTMLSelectElement)?.value || "").trim();
  if (!value || value === props.selectedChatModelId) return;
  emit("update:selectedChatModelId", value);
}

function togglePlanMode() {
  if (!planModeToggleAllowed.value) return;
  emit("update:planModeEnabled", !props.planModeEnabled);
}

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  const minHeight = 32;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.max(Math.min(el.scrollHeight, maxHeight), minHeight);
  el.style.height = `${nextHeight}px`;
  el.style.overflowY = "auto";
}

function handleChatInputInput() {
  scheduleResizeChatInput();
  updateMentionState();
}

function scheduleResizeChatInput() {
  if (resizeInputRaf.value) cancelAnimationFrame(resizeInputRaf.value);
  resizeInputRaf.value = requestAnimationFrame(() => {
    resizeChatInput();
    resizeInputRaf.value = 0;
  });
}

function applyChatInputHistoryValue(value: string) {
  chatInputHistoryApplying.value = true;
  localChatInput.value = value;
  nextTick(() => {
    chatInputHistoryApplying.value = false;
    scheduleResizeChatInput();
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = value.length;
    el.setSelectionRange(cursor, cursor);
  });
}

function canNavigateHistory(el: HTMLTextAreaElement, direction: "up" | "down"): boolean {
  if (el.selectionStart !== el.selectionEnd) return false;
  if (direction === "up") return el.selectionStart === 0;
  return el.selectionStart === el.value.length;
}

function navigateChatInputHistory(direction: "up" | "down"): boolean {
  const list = chatInputHistory.value;
  if (list.length === 0) return false;
  if (direction === "up") {
    if (chatInputHistoryCursor.value === -1) {
      chatInputHistoryDraft.value = localChatInput.value;
      chatInputHistoryCursor.value = 0;
      applyChatInputHistoryValue(list[0]);
      return true;
    }
    if (chatInputHistoryCursor.value < list.length - 1) {
      chatInputHistoryCursor.value += 1;
      applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
      return true;
    }
    return false;
  }
  if (chatInputHistoryCursor.value === -1) return false;
  if (chatInputHistoryCursor.value === 0) {
    chatInputHistoryCursor.value = -1;
    const draft = chatInputHistoryDraft.value;
    chatInputHistoryDraft.value = "";
    applyChatInputHistoryValue(draft);
    return true;
  }
  chatInputHistoryCursor.value -= 1;
  applyChatInputHistoryValue(list[chatInputHistoryCursor.value]);
  return true;
}

function recordSentTextIfNeeded(rawText: string) {
  const text = String(rawText || "").trim();
  if (!text) return;
  setTimeout(() => {
    if (String(props.chatInput || "").trim()) return;
    pushChatInputHistory(text);
  }, 0);
}

function handleSendChat() {
  const plainText = String(localChatInput.value || "").trim();
  emit("sendChat");
  recordSentTextIfNeeded(plainText);
  clearSelectedInstructionPrompts();
  closeInstructionPanel();
  closeMentionPanel();
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.repeat) return;
  if (event.key !== "Tab" || !event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return;
  if (!planModeToggleAllowed.value) return;
  const activeElement = document.activeElement;
  const textareaFocused = !!chatInputRef.value && activeElement === chatInputRef.value;
  const composerFocused = !!composerRootRef.value && activeElement === composerRootRef.value;
  if (!textareaFocused && !composerFocused) return;
  event.preventDefault();
  togglePlanMode();
}

function handleChatInputKeydown(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (mentionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMentionPanel();
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveMentionFocus(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveMentionFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      selectMentionByIndex(mentionFocusIndex.value);
      return;
    }
  }
  if (event.key === "Tab" && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    toggleInstructionPanel();
    return;
  }
  if (instructionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeInstructionPanel();
      return;
    }
    if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
      event.preventDefault();
      moveInstructionFocus(-1);
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowRight") {
      event.preventDefault();
      moveInstructionFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      selectInstructionPresetByIndex(instructionFocusIndex.value);
      return;
    }
  }
  if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
    if (props.frozen || props.busy) return;
    event.preventDefault();
    handleSendChat();
    return;
  }
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
  const el = chatInputRef.value;
  if (!el) return;
  const direction = event.key === "ArrowUp" ? "up" : "down";
  if (!canNavigateHistory(el, direction)) return;
  if (navigateChatInputHistory(direction)) {
    event.preventDefault();
  }
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function avatarInitial(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function isMentionSelected(agentId: string): boolean {
  return selectedMentions.value.some((item) => item.agentId === agentId);
}

async function handleRecallToInput(event: {
  source?: string;
  messagePreview?: string;
  messageText?: string;
  id?: string;
  queueMode?: "normal" | "guided";
}) {
  if (event.source === "user" && event.queueMode !== "guided") {
    if (event.id) {
      const result = await recallQueueEvent(event.id);
      if (result.removed) {
        localChatInput.value = result.messageText || event.messageText || event.messagePreview || "";
      }
    }
  }
}

function focusInput(options?: FocusOptions) {
  chatInputRef.value?.focus(options);
}

defineExpose({
  focusInput,
  openSelectionDeliverCard,
  openSelectionDelegateCard,
  openSelectionShareCard,
});

onMounted(() => {
  loadChatInputHistory();
  loadRecentDelegateRequests();
  window.addEventListener("keydown", handleWindowKeydown);
  window.addEventListener("resize", refreshMentionPanelPosition);
  window.addEventListener("scroll", refreshMentionPanelPosition, true);
  nextTick(() => {
    resizeChatInput();
    refreshMentionPanelPosition();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
  window.removeEventListener("resize", refreshMentionPanelPosition);
  window.removeEventListener("scroll", refreshMentionPanelPosition, true);
  if (resizeInputRaf.value) {
    cancelAnimationFrame(resizeInputRaf.value);
    resizeInputRaf.value = 0;
  }
});

watch(
  () => props.chatInput,
  (nextValue, prevValue) => {
    if (!chatInputHistoryApplying.value && nextValue !== prevValue && chatInputHistoryCursor.value !== -1) {
      chatInputHistoryCursor.value = -1;
      chatInputHistoryDraft.value = "";
    }
    nextTick(() => scheduleResizeChatInput());
    nextTick(() => {
      refreshMentionPanelPosition();
      updateMentionState();
    });
  },
);

watch(
  () => props.selectionModeEnabled,
  (enabled, previous) => {
    if (enabled && !previous) {
      closeSelectionDeliverCard();
      closeSelectionDelegateCard();
      closeSelectionShareCard();
      clearSelectionDelegateFields();
      return;
    }
    if (!enabled) {
      closeSelectionDeliverCard();
      closeSelectionDelegateCard();
      closeSelectionShareCard();
    }
  },
);

watch(
  () => props.chatting,
  (isChatting, wasChatting) => {
    if (wasChatting && !isChatting && !props.frozen) {
      nextTick(() => focusInput({ preventScroll: true }));
    }
  },
);

watch(
  () => props.activeConversationId,
  () => {
    closeInstructionPanel();
    closeMentionPanel();
    clearSelectedInstructionPrompts();
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => normalizedInstructionPresets.value,
  (list) => {
    if (list.length === 0) {
      instructionFocusIndex.value = 0;
      selectedInstructionPrompts.value = [];
      emitSelectedInstructionPrompts();
      instructionPanelOpen.value = false;
      return;
    }
    if (instructionFocusIndex.value >= list.length) {
      instructionFocusIndex.value = list.length - 1;
    }
    selectedInstructionPrompts.value = selectedInstructionPrompts.value.filter((item) =>
      list.some((entry) => entry.id === item.id),
    );
    emitSelectedInstructionPrompts();
  },
  { deep: true },
);

watch(
  () => props.selectedMentions.map((item) => `${item.agentId}:${item.departmentId}`).join("|"),
  () => {
    closeMentionPanel();
  },
);
</script>
