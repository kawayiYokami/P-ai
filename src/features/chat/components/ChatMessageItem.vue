<template>
  <div
    :data-message-id="String(block.id || '')"
    :data-message-role="isOwnMessage(block) ? 'user' : block.role"
    :data-active-turn-user="activeTurnUser ? 'true' : undefined"
    :class="[
      'ecall-chat-message-row group/user-turn relative rounded-2xl px-4 transition-colors',
      shouldAnimateEnter(block) ? 'ecall-message-enter' : '',
      isOwnMessage(block) ? 'ecall-chat-message-row-own' : 'ecall-chat-message-row-other',
      isOwnMessage(block) && compactWithPrevious ? 'ecall-message-continued' : '',
      selectionModeEnabled ? 'ecall-chat-message-row-selectable' : '',
      selectionModeEnabled && selected ? 'ecall-message-selected bg-neutral/10 ring-1 ring-neutral/20 shadow-sm' : '',
    ]"
    @click="handleSelectionRowClick"
    @contextmenu="openContextMenu($event)"
  >
    <div
      v-if="selectionModeEnabled"
      :class="[
        'ecall-message-selection-control',
        isOwnMessage(block) ? 'ecall-message-selection-control-right' : 'ecall-message-selection-control-left',
      ]"
    >
      <button
        type="button"
        data-selection-ignore="true"
        class="inline-flex h-4 w-4 items-center justify-center rounded-sm border transition-colors"
        :class="selected
          ? 'border-primary bg-primary text-primary-content'
          : 'border-base-300 bg-base-100 text-transparent hover:border-primary/60'"
        :title="selected ? t('chat.messageItem.cancelSelect') : t('chat.messageItem.selectMessage')"
        @click.stop="emit('toggleMessageSelected', selectionKey)"
      >
        <span class="text-caption leading-none">✓</span>
      </button>
    </div>
    <ChatBubbleShell
      :tone="messageShellTone(block)"
      :name="displayName"
      :meta="assistantMetaText"
      :avatar-url="avatarUrl"
      :streaming="!!streamingHeaderStatus"
      :streaming-text="streamingHeaderStatus"
      :wide="blockNeedsWideBubble(block)"
      :content-empty="bubbleContentEmpty(block)"
    >
      <template v-if="showActivitySummary(block)" #activity>
        <div
          v-memo="activityPanelMemoKey(block)"
          class="flex flex-col opacity-90"
        >
          <details
            ref="activityDetailsRef"
            class="collapse rounded-none min-w-55"
            :class="{ 'pointer-events-none': !showActivityPanel(block) }"
            :open="activityPanelOpen(block)"
            @toggle="onActivityToggle"
          >
            <summary
              class="collapse-title px-0 py-0.5 min-h-0 text-xs font-normal flex items-center gap-1.5 text-base-content/55"
              :class="showActivityPanel(block) ? 'hover:bg-base-200 cursor-pointer' : 'cursor-default'"
            >
              <span class="flex min-w-0 flex-1 items-center gap-1.5">
                <span class="shrink-0">
                  <template v-if="showActivityPanel(block)">
                    {{ activityStatusText(block) }}<AnimatedCountText :target="block.activityReasoningCharCount || 0" />
                  </template>
                  <template v-else>{{ t("chat.messageItem.notThought") }}</template>
                </span>
                <span v-if="showActivityPanel(block) && activityToolCountsLabel(block)" class="inline-flex h-3 items-center text-base-content/40">·</span>
                <span
                  v-if="showActivityPanel(block) && activityToolCountsLabel(block)"
                  v-memo="[activityToolCountsLabel(block)]"
                  class="min-w-0 truncate text-base-content/55"
                >
                  {{ activityToolCountsLabel(block) }}
                </span>
              </span>
            </summary>
          </details>
        </div>
      </template>
      <template v-if="showActivitySummary(block)" #activity-panel>
        <div v-memo="activityPanelMemoKey(block)" class="min-w-0">
          <Transition
            :css="false"
            @enter="animateEnter"
            @leave="animateLeave"
            @enter-cancelled="cleanupAnimation"
            @leave-cancelled="cleanupAnimation"
          >
            <div
              v-if="showActivityPanel(block) && activityPanelOpen(block)"
              class="px-0 pb-1 pt-2 text-xs text-base-content/70"
            >
              <div class="flex flex-col">
                <TransitionGroup name="ecall-activity-item" tag="ul" class="ecall-activity-timeline" :appear="false">
                  <li v-for="(item, itemIndex) in resolvedActivityItems(block)" :key="`${block.id}-activity-${activityItemKey(item)}`" class="flex gap-1.5" :class="activityItemNodeClass(item)">
                      <div class="flex w-4 shrink-0 flex-col items-center pt-1">
                        <span
                          v-if="item.kind === 'tool' && item.status === 'doing'"
                          class="loading loading-spinner loading-xs text-primary"
                        ></span>
                        <svg
                          v-else-if="item.kind === 'reasoning'"
                          viewBox="0 0 24 24"
                          class="h-4 w-4"
                        >
                          <circle cx="12" cy="12" r="10" fill="currentColor" />
                          <path d="M12 4Q13 11 20 12Q13 13 12 20Q11 13 4 12Q11 11 12 4Z" class="text-base-100" fill="currentColor" />
                        </svg>
                        <svg
                          v-else-if="item.kind === 'content'"
                          viewBox="0 0 24 24"
                          class="h-4 w-4"
                        >
                          <circle cx="12" cy="12" r="10" fill="currentColor" />
                          <path d="M8.4 10.2h7.2M8.4 13.8h7.2" class="text-base-100" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                        </svg>
                        <svg
                          v-else
                          viewBox="0 0 24 24"
                          class="h-4 w-4"
                        >
                          <circle cx="12" cy="12" r="10" fill="currentColor" />
                          <path
                            d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                            class="text-base-100"
                            fill="currentColor"
                            stroke="currentColor"
                            stroke-width="1.5"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            transform="translate(3.84 3.84) scale(0.68)"
                          />
                        </svg>
                        <span v-if="itemIndex !== resolvedActivityItems(block).length - 1" class="mt-1 w-px flex-1 bg-current" />
                      </div>
                      <div class="min-w-0 flex-1">
                        <details
                          v-if="item.kind === 'tool' && activityItemCanExpand(item)"
                          class="collapse rounded-none"
                        >
                          <summary class="collapse-title flex min-h-0 items-center gap-1.5 px-1 py-1 text-xs hover:bg-base-200">
                            <span
                              class="ecall-activity-item-summary min-w-0 flex-1"
                              :class="activityItemTitleClass(item)"
                            >
                              <span>{{ activityItemDisplay(item).text }}</span>
                              <span
                                v-if="activityItemDisplay(item).adds > 0"
                                class="ml-1 shrink-0 text-success"
                              >+{{ activityItemDisplay(item).adds }}</span>
                              <span
                                v-if="activityItemDisplay(item).removes > 0"
                                class="ml-1 shrink-0 text-error"
                              >-{{ activityItemDisplay(item).removes }}</span>
                            </span>
                            <ChevronDown
                              class="ecall-activity-chevron mt-0.5 h-3.5 w-3.5 shrink-0 text-base-content/45 transition-transform duration-150"
                            />
                          </summary>
                          <div class="collapse-content pb-2 pr-1 pt-1">
                            <pre
                              class="m-0 max-h-72 overflow-auto whitespace-pre-wrap break-all rounded bg-base-200/60 p-2 text-xs leading-relaxed"
                              :class="activityItemDetailClass(item)"
                            ><code>{{ activityToolDetailsText(item) }}</code></pre>
                          </div>
                        </details>
                        <div
                          v-else-if="item.kind === 'tool'"
                          class="flex min-h-0 items-center gap-1.5 px-1 py-1 text-xs"
                          @click.stop
                        >
                          <span
                            class="ecall-activity-item-summary min-w-0 flex-1"
                            :class="activityItemTitleClass(item)"
                          >
                            <span>{{ activityItemDisplay(item).text }}</span>
                            <span
                              v-if="activityItemDisplay(item).adds > 0"
                              class="ml-1 shrink-0 text-success"
                            >+{{ activityItemDisplay(item).adds }}</span>
                            <span
                              v-if="activityItemDisplay(item).removes > 0"
                              class="ml-1 shrink-0 text-error"
                            >-{{ activityItemDisplay(item).removes }}</span>
                          </span>
                        </div>
                        <div v-else class="flex px-1 py-1">
                          <ExpandableText
                            class="min-w-0 flex-1"
                            :text="activityItemText(item)"
                            :text-class="activityItemDetailClass(item)"
                            :follow="!!item.running"
                          />
                        </div>
                      </div>
                    </li>
                </TransitionGroup>
                <button
                  type="button"
                  class="btn btn-sm mt-2 w-full border-0 bg-base-300 text-base-content/70 hover:bg-base-300 hover:text-base-content"
                  data-selection-ignore="true"
                  @click.stop="closeActivityDetails"
                >
                  {{ t("common.collapse") }}
                </button>
              </div>
            </div>
          </Transition>
        </div>
      </template>

      <template v-if="!isOwnMessage(block)">
        <div
          v-if="!showAssistantPreStreamingDots(block)"
          class="assistant-markdown ecall-assistant-bubble max-w-full"
          :class="{ 'ecall-assistant-bubble-wide': blockNeedsWideBubble(block) }"
          :data-bubble-background="assistantBubbleBackgroundEnabled ? 'on' : 'off'"
          :data-segmented-markdown="segmentedMarkdownEnabled ? 'on' : 'off'"
        >
          <div v-if="block.text">
            <div
              v-if="plainMarkdownDebugEnabled"
              @click="emit('assistantLinkClick', $event)"
            >
              <PlainMarkdownRenderer :text="assistantRenderedText" />
            </div>
            <div v-else ref="markdownContainerRef">
              <div class="ecall-assistant-segment-list">
                <template
                  v-for="(piece, pieceIndex) in assistantMarkdownPieces"
                  :key="piece.key"
                >
                  <div
                    v-for="(segment, segmentIndex) in piece.segments"
                    :key="segment.key"
                    :class="[
                      'ecall-assistant-segment',
                      segment.kind === 'text' ? 'ecall-assistant-segment-text' : 'ecall-assistant-segment-rich',
                    ]"
                  >
                    <AppMarkdownRenderer
                      class="ecall-markdown-content max-w-none"
                      :blocks="segment.blocks"
                      :is-dark="markdownIsDark"
                      :streaming="!!block.isStreaming && pieceIndex === assistantMarkdownPieces.length - 1 && segmentIndex === piece.segments.length - 1"
                      :local-image-base-path="currentWorkspaceRootPath"
                      :toolcall-preview-map="toolcallPreviewMap"
                      @math-context-menu="openMathContextMenu"
                      @open-image-preview="emit('openImagePreview', $event)"
                      @click="emit('assistantLinkClick', $event)"
                    />
                  </div>
                </template>
              </div>
            </div>
          </div>
          <div
            v-if="block.planCard"
            class="ecall-assistant-segment ecall-assistant-segment-text space-y-3"
            :class="block.text ? 'mt-3' : ''"
          >
            <div class="text-xs italic opacity-60 mb-1">{{ t("chat.plan.sidebarHint") }}</div>
            <div @click="emit('assistantLinkClick', $event)">
              <a :href="block.planCard.path" class="link link-primary text-sm" :title="block.planCard.path">{{ t("chat.plan.linkLabel") }}{{ block.planCard.path.split(/[/\\]/).filter(Boolean).pop() }}</a>
            </div>
            <div v-if="block.providerMeta?.planCard && block.planCard.action === 'present'" class="space-y-2">
              <button
                type="button"
                class="ecall-plan-confirm-action btn btn-sm btn-primary"
                :disabled="chatting || busy || frozen || !canConfirmPlan"
                @click="emit('confirmPlan', { messageId: block.sourceMessageId || block.id })"
              >
                {{ t("chat.plan.confirmAction") }}
              </button>
              <div class="text-xs opacity-60">{{ t("chat.plan.confirmHint") }}</div>
            </div>
          </div>
          <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
            <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
              <img
                v-if="isImageMime(img.mime) && resolvedImageSrc(img, idx)"
                :src="resolvedImageSrc(img, idx)"
                loading="lazy"
                decoding="async"
                class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
                @click.stop="openResolvedImagePreview(img, idx)"
              />
              <div
                v-else-if="isImageMime(img.mime)"
                class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-xs text-base-content/55"
              >
                <span class="loading loading-spinner loading-xs mr-2"></span>
                <span>{{ t('chat.messageItem.imageLoading') }}</span>
              </div>
              <ChatAttachmentItem
                v-else-if="isPdfMime(img.mime)"
                :attachment="{ kind: 'file', label: 'PDF' }"
              />
            </template>
          </div>
          <div v-if="block.audios.length > 0" :class="block.taskTrigger || block.text || block.images.length > 0 ? 'mt-2 flex flex-col gap-1' : 'flex flex-col gap-1'">
            <ChatAttachmentItem
              v-for="(aud, idx) in block.audios"
              :key="`${block.id}-aud-${idx}`"
              :attachment="{ kind: 'audio', label: aud.name ? displayFileName(aud.name) : t('chat.voice', { index: idx + 1 }) }"
              :interactive="true"
              :playing="playingAudioId === `${block.id}-aud-${idx}`"
              @activate="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
            />
          </div>
          <div
            v-if="block.attachmentFiles.length > 0"
            :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap gap-1' : 'flex flex-wrap gap-1'"
          >
            <ChatAttachmentItem
              v-for="(file, idx) in block.attachmentFiles"
              :key="`${block.id}-file-${idx}`"
              :attachment="{ kind: 'file', label: displayFileName(file.fileName, file.path) }"
              :interactive="true"
              :title="file.path"
              @activate="openAttachmentPath(file.path)"
            />
          </div>
        </div>
      </template>

      <template v-else>
        <div class="ecall-user-message-content">
          <div
            v-if="!!ownMessageDisplayText(block).trim()"
            class="whitespace-pre-wrap break-all"
            style="overflow-wrap: anywhere;"
          >{{ ownMessageDisplayText(block) }}</div>
          <div
            v-if="block.extraTextReferences && block.extraTextReferences.length > 0"
            :class="block.text ? 'mt-2 flex flex-wrap justify-end gap-1' : 'flex flex-wrap justify-end gap-1'"
          >
            <ChatAttachmentItem
              v-for="(reference, idx) in block.extraTextReferences"
              :key="`${block.id}-extra-ref-${idx}`"
              :attachment="{ kind: 'context', label: extraTextReferenceDisplayParts(reference.text).fileName, detail: extraTextReferenceDisplayParts(reference.text).lineSuffix }"
            />
          </div>
          <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid justify-items-end gap-1' : 'grid justify-items-end gap-1'">
            <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
              <img
                v-if="isImageMime(img.mime) && resolvedImageSrc(img, idx)"
                :src="resolvedImageSrc(img, idx)"
                loading="lazy"
                decoding="async"
                class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
                @click.stop="openResolvedImagePreview(img, idx)"
              />
              <div
                v-else-if="isImageMime(img.mime)"
                class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-xs text-base-content/55"
              >
                <span class="loading loading-spinner loading-xs mr-2"></span>
                <span>{{ t('chat.messageItem.imageLoading') }}</span>
              </div>
              <ChatAttachmentItem
                v-else-if="isPdfMime(img.mime)"
                :attachment="{ kind: 'file', label: 'PDF' }"
              />
            </template>
          </div>
          <div v-if="block.audios.length > 0" :class="block.taskTrigger || block.text || block.images.length > 0 ? 'mt-2 flex flex-col items-end gap-1' : 'flex flex-col items-end gap-1'">
            <ChatAttachmentItem
              v-for="(aud, idx) in block.audios"
              :key="`${block.id}-aud-${idx}`"
              :attachment="{ kind: 'audio', label: aud.name ? displayFileName(aud.name) : t('chat.voice', { index: idx + 1 }) }"
              :interactive="true"
              :playing="playingAudioId === `${block.id}-aud-${idx}`"
              @activate="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
            />
          </div>
          <div
            v-if="block.attachmentFiles.length > 0"
            :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap justify-end gap-1' : 'flex flex-wrap justify-end gap-1'"
          >
            <ChatAttachmentItem
              v-for="(file, idx) in block.attachmentFiles"
              :key="`${block.id}-file-${idx}`"
              :attachment="{ kind: 'file', label: displayFileName(file.fileName, file.path) }"
              :interactive="true"
              :title="file.path"
              @activate="openAttachmentPath(file.path)"
            />
          </div>
        </div>
      </template>

      <template v-if="showMessageFooterActions(block)" #footer>
        <button
          type="button"
          class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
          :title="t('chat.copy')"
          @click="emit('copyMessage', block)"
        >
          <Copy class="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
          :title="t('chat.selection.copyImageAsImage')"
          :disabled="copyMessageImageBusy"
          @click="copyCurrentMessageAsImage"
        >
          <ImageIcon class="h-3.5 w-3.5" />
        </button>
        <button
          v-if="canRecallBlock(block)"
          type="button"
          class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
          :title="t('chat.recall')"
          :disabled="selectionModeEnabled || busy"
          @click="emit('recallTurn', { turnId: recallTurnId(block) })"
        >
          <Undo2 class="h-3.5 w-3.5" />
        </button>
      </template>
    </ChatBubbleShell>

  </div>

  <Teleport to="body">
    <ul
      v-if="contextMenuOpen"
      ref="contextMenuRef"
      tabindex="0"
      class="menu fixed z-[1200] w-44 rounded-box border border-base-300 bg-base-100 p-1 text-base-content shadow-xl"
      :data-theme="teleportTheme"
      :style="{ left: contextMenuX + 'px', top: contextMenuY + 'px' }"
      @click.stop
      @mousedown.stop
      @keydown.esc.prevent.stop="closeContextMenu"
    >
      <li>
        <button type="button" @click="handleContextMenuAction('select')">
          <ListCheck class="h-4 w-4" />
          <span>{{ t('chat.messageItem.multiSelect') }}</span>
        </button>
      </li>
      <li>
        <button type="button" @click="handleContextMenuAction('copy')">
          <Copy class="h-4 w-4" />
          <span>{{ t('common.copy') }}</span>
        </button>
      </li>
      <li>
        <button type="button" @click="handleContextMenuAction('copyAsImage')">
          <ImageIcon class="h-4 w-4" />
          <span>{{ t('chat.selection.copyImageAsImage') }}</span>
        </button>
      </li>
      <li v-if="isDevBuild">
        <button type="button" @click="handleContextMenuAction('showRawData')">
          <Braces class="h-4 w-4" />
          <span>显示原始 ChatMessage</span>
        </button>
      </li>
      <li v-if="mathContextCopyText">
        <button type="button" @click="handleContextMenuAction('copyMath')">
          <Copy class="h-4 w-4" />
          <span>{{ t('chat.copyMath') }}</span>
        </button>
      </li>
      <li v-if="canRecallBlock(block)">
        <button type="button" @click="handleContextMenuAction('branchFromMessage')">
          <Split class="h-4 w-4" />
          <span>{{ t('chat.messageItem.branchFromMessage') }}</span>
        </button>
      </li>
      <li v-if="canRecallBlock(block)">
        <button type="button" class="text-error" @click="handleContextMenuAction('recall')">
          <Undo2 class="h-4 w-4" />
          <span>{{ t('chat.recall') }}</span>
        </button>
      </li>
    </ul>
  </Teleport>

  <dialog
    ref="rawMessageDialogRef"
    class="modal"
    @close="closeRawMessageData"
    @cancel.prevent="closeRawMessageData"
  >
    <div class="modal-box flex max-h-[85vh] w-full max-w-3xl flex-col p-0 overflow-hidden">
      <header class="flex items-center justify-between border-b border-base-300 px-4 py-3">
        <h2 class="font-semibold">原始 ChatMessage</h2>
        <button type="button" class="btn btn-ghost btn-sm" @click="closeRawMessageData">关闭</button>
      </header>
      <!-- 未打开时不渲染：大消息 JSON 有数十万字符，visibility:hidden 仍参与布局 -->
      <pre v-if="rawMessageDataOpen" class="m-0 overflow-auto whitespace-pre-wrap break-all p-4 text-xs leading-relaxed"><code>{{ rawMessageData }}</code></pre>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeRawMessageData">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect, watchPostEffect } from "vue";
import { useI18n } from "vue-i18n";
import { Braces, ChevronDown, Copy, FileText, ImageIcon, ListCheck, Split, Undo2 } from "@lucide/vue";
import { invokeTauri, openTransportWorkspaceFile, readTransportChatImage } from "../../../services/tauri-api";
import type { ChatActivityItem, ChatMessageBlock } from "../../../types/app";
import {
  normalizeAssistantStreamBlocks,
  assistantContentBlocksFromMessage,
  streamBlocksToActivityItems,
  TOOL_TEXT_BREAK_PLACEHOLDER,
} from "../../../utils/chat-message-semantics";
import { formatIsoToLocalDateTime } from "../../../utils/time";
import { useChatMessageAppearance } from "../../shell/composables/use-chat-message-appearance";
import { AppMarkdownRenderer, groupMarkdownSegments, initKatex, parseMarkdownBlocks, type MarkdownSegment } from "../markdown";
import { hideIncompleteInlineMath } from "../markdown/streaming-math";
import { normalizeLocalLinkHref } from "../utils/local-link";
import { textContentSignature } from "../utils/text-signature";
import { createToolCallPresentation } from "../utils/tool-call-presentation";
import { buildToolcallPreviewMap, parseToolCallResultStatus } from "../utils/toolcall-preview";
import { generateShareFromMessageIds } from "../utils/share-generator";
import { frontendDispatchElapsedByMessageId } from "../composables/use-chat-flow-frontend-dispatch";
import { useCollapseTransition } from "../composables/use-collapse-transition";
import { displayFileName, extraTextReferenceDisplayParts } from "../utils/chat-attachment-display";
import ChatBubbleShell from "./ChatBubbleShell.vue";
import ChatAttachmentItem from "./ChatAttachmentItem.vue";
import PlainMarkdownRenderer from "./PlainMarkdownRenderer.vue";
import AnimatedCountText from "./AnimatedCountText.vue";
import ExpandableText from "../../shared/components/ExpandableText.vue";

initKatex();

const imageDataUrlCache = new Map<string, string>();
const imageDataUrlPromiseCache = new Map<string, Promise<string>>();
const debugPlainMarkdownRender = typeof window !== "undefined"
  && window.localStorage.getItem("easy-call.debug.chat-plain-markdown") === "1";

const props = defineProps<{
  activeConversationId: string;
  block: ChatMessageBlock;
  selectionKey: string;
  selectionModeEnabled: boolean;
  selected: boolean;
  chatting: boolean;
  busy: boolean;
  frozen: boolean;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  agentNameMap?: Record<string, string>;
  markdownIsDark: boolean;
  playingAudioId: string;
  activeTurnUser: boolean;
  compactWithPrevious: boolean;
  canRegenerate: boolean;
  canConfirmPlan: boolean;
  currentWorkspaceRootPath?: string;
  currentTheme?: string;
  disableRecallAndBranchActions?: boolean;
  isLastUserMessage?: boolean;
  isLastAssistantMessage?: boolean;
}>();

const emit = defineEmits<{
  (e: "enterSelectionMode", selectionKey: string): void;
  (e: "toggleMessageSelected", selectionKey: string): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "createConversationBranchFromTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "confirmPlan", payload: { messageId: string }): void;
  (e: "copyMessage", block: ChatMessageBlock): void;
  (e: "copyMessageImageDone"): void;
  (e: "copyMessageImageFailed"): void;
  (e: "openImagePreview", image: { mime?: string; bytesBase64?: string; dataUrl?: string; localPath?: string; src?: string; alt?: string }): void;
  (e: "toggleAudioPlayback", payload: { id: string; audio: { mime: string; bytesBase64?: string; mediaRef?: string } }): void;
  (e: "assistantLinkClick", event: MouseEvent): void;
  (e: "activityToggle", payload: { blockId: string; open: boolean }): void;
}>();

const { t } = useI18n();
const { animateEnter, animateLeave, cleanupAnimation } = useCollapseTransition();
const {
  assistantBubbleBackgroundEnabled,
  segmentedMarkdownEnabled,
  chatTimeDisplayMode,
} = useChatMessageAppearance();
const {
  joinNonEmpty,
  normalizeToolCallArgs,
  toolCallDisplayName,
  toolCallSummaryText,
  toolCallTitle,
  toolTimelineText,
} = createToolCallPresentation({
  t: (key, params) => String(t(key, params ?? {})),
  agentName: (agentId) => props.agentNameMap?.[agentId] || agentId,
});
const resolvedImageSrcMap = ref<Record<string, string>>({});
const markdownContainerRef = ref<HTMLElement | null>(null);
const activityDetailsRef = ref<HTMLDetailsElement | null>(null);
const activityExpanded = ref(false);
const copyMessageImageBusy = ref(false);
const planMarkdownText = ref("");
const planMarkdownError = ref("");
const planMarkdownLoading = ref(false);
const plainMarkdownDebugEnabled = debugPlainMarkdownRender;
const assistantRawRenderedText = computed(() => formatAssistantStreamingText(props.block));
const assistantRenderedText = computed(() =>
  assistantRawRenderedText.value.split(TOOL_TEXT_BREAK_PLACEHOLDER).join("\n\n"),
);
const assistantMarkdownPieces = computed<Array<{ key: string; segments: MarkdownSegment[] }>>(() => {
  if (plainMarkdownDebugEnabled) return [];
  const text = assistantRawRenderedText.value;
  if (!text) return [];
  const segmented = segmentedMarkdownEnabled.value;
  const pieces = segmented
    ? text.split(TOOL_TEXT_BREAK_PLACEHOLDER)
    : [assistantRenderedText.value];
  const result: Array<{ key: string; segments: MarkdownSegment[] }> = [];
  pieces.forEach((piece, pieceIndex) => {
    if (!piece.trim()) return;
    const blocks = parseMarkdownBlocks(piece, !!props.block.isStreaming);
    result.push({
      key: `piece-${pieceIndex}`,
      segments: segmented
        ? groupMarkdownSegments(blocks)
        : [{ kind: "text", key: `piece-${pieceIndex}-all`, blocks }],
    });
  });
  return result;
});
const teleportTheme = computed(() => {
  const documentTheme = typeof document === "undefined" ? "" : document.documentElement.getAttribute("data-theme");
  return String(props.currentTheme || documentTheme || "light").trim() || "light";
});
let disposed = false;

const contextMenuOpen = ref(false);
const contextMenuRef = ref<HTMLElement | null>(null);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const mathContextCopyText = ref("");
const rawMessageDataOpen = ref(false);
const rawMessageDialogRef = ref<HTMLDialogElement | null>(null);

function syncRawMessageDialog() {
  const d = rawMessageDialogRef.value;
  if (!d) return;
  if (rawMessageDataOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(rawMessageDataOpen, syncRawMessageDialog);
watch(rawMessageDialogRef, syncRawMessageDialog);

const isDevBuild = import.meta.env.DEV;
// 惰性序列化：未打开时不 stringify（大消息 JSON 数十万字符，白耗 CPU）
const rawMessageData = computed(() => {
  if (!rawMessageDataOpen.value) return "";
  try {
    return JSON.stringify(props.block.rawMessage || props.block, null, 2);
  } catch (error) {
    return `无法序列化消息数据：${error instanceof Error ? error.message : String(error)}`;
  }
});
const relativeTimeNowTick = ref(Date.now());
let relativeTimeNowTimer = 0;

watch(
  () => ({
    conversationId: String(props.activeConversationId || "").trim(),
    action: String(props.block.planCard?.action || "").trim(),
    path: String(props.block.planCard?.path || "").trim(),
    blockId: String(props.block.id || "").trim(),
  }),
  async (snapshot, _previous, onCleanup) => {
    let cancelled = false;
    onCleanup(() => {
      cancelled = true;
    });
    planMarkdownText.value = "";
    planMarkdownError.value = "";
    planMarkdownLoading.value = false;
    if (snapshot.action !== "present" || !snapshot.path || !snapshot.conversationId) {
      return;
    }
    planMarkdownLoading.value = true;
    try {
      const input = { conversationId: snapshot.conversationId, path: snapshot.path };
      const content = await invokeTauri<string>("conversation.plan.readFile", input);
      if (cancelled || disposed) return;
      planMarkdownText.value = String(content || "");
    } catch (error) {
      if (cancelled || disposed) return;
      const message =
        error instanceof Error ? error.message : String(error || t('chat.messageItem.readPlanFailed'));
      planMarkdownError.value = message;
    } finally {
      if (!cancelled && !disposed) {
        planMarkdownLoading.value = false;
      }
    }
  },
  { immediate: true },
);

const displayName = computed(() => messageName(props.block));
const avatarUrl = computed(() => messageAvatarUrl(props.block));
const assistantCreatedAtText = computed(() => {
  if (isOwnMessage(props.block) || props.block.isStreaming) return "";
  if (chatTimeDisplayMode.value === "absolute") {
    return formatIsoToLocalDateTime(props.block.createdAt, "");
  }
  return formatRecentRelativeTime(props.block.createdAt, relativeTimeNowTick.value);
});
const assistantMetaText = assistantCreatedAtText;
const streamingHeaderStatus = computed(() => assistantStreamingHeaderStatus(props.block));
const toolcallPreviewMap = computed<Record<string, { title: string; body: string; filePath?: string; fileLabel?: string }>>(() => {
  const previews = buildToolcallPreviewMap(props.block.activityItems, toolTimelineText("noArgs"));
  for (const item of props.block.activityItems) {
    if (item.kind !== "tool") continue;
    const toolCallId = String(item.toolCallId || "").trim();
    if (!toolCallId || !previews[toolCallId]) continue;
    previews[toolCallId].title = activityItemTitle(item);
  }
  return previews;
});

function detailsOpenFromEvent(event: Event): boolean {
  const target = event.target;
  return target instanceof HTMLDetailsElement ? target.open : false;
}

function messageName(block: ChatMessageBlock): string {
  if (block.remoteImOrigin) {
    return block.remoteImOrigin.senderName || block.remoteImOrigin.remoteContactName || "IM";
  }
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaNameMap[id]) return props.personaNameMap[id];
  if (!id || id === "user-persona") return props.userAlias || t("archives.roleUser");
  return id;
}

function messageAvatarUrl(block: ChatMessageBlock): string {
  if (block.remoteImOrigin) return "";
  const id = String(block.speakerAgentId || "").trim();
  if (id && props.personaAvatarUrlMap[id]) return props.personaAvatarUrlMap[id];
  if (!id || id === "user-persona") return props.userAvatarUrl || "";
  return "";
}

function isOwnMessage(block: ChatMessageBlock): boolean {
  if (block.remoteImOrigin) {
    return block.role === "user" && block.remoteImOrigin.remoteContactType !== "group";
  }
  const id = String(block.speakerAgentId || "").trim();
  return !id || id === "user-persona";
}

function messageShellTone(block: ChatMessageBlock): "assistant" | "user" | "system" {
  if (isOwnMessage(block)) return "user";
  if (String(block.role || "").trim().toLowerCase() === "system") return "system";
  if (String(block.speakerAgentId || "").trim() === "system-persona") return "system";
  if (String(block.role || "").trim().toLowerCase() === "user") return "user";
  return "assistant";
}

function recallTurnId(block: ChatMessageBlock): string {
  return String(block.sourceMessageId || block.id || "").trim();
}

function canRecallBlock(block: ChatMessageBlock): boolean {
  if (props.disableRecallAndBranchActions) return false;
  if (block.remoteImOrigin) return false;
  if (block.isStreaming) return false;
  if (String(block.role || "").trim().toLowerCase() === "system") return false;
  if (String(block.speakerAgentId || "").trim() === "system-persona") return false;
  return !!recallTurnId(block);
}

function showMessageFooterActions(block: ChatMessageBlock): boolean {
  return !block.isStreaming && !props.selectionModeEnabled;
}

function ownMessageDisplayText(block: ChatMessageBlock): string {
  const mentions = Array.isArray(block.mentions) ? block.mentions : [];
  const mentionPrefix = mentions
    .map((item) => `@${String(item.agentName || "").trim()}`)
    .filter((item) => item !== "@")
    .join(",");
  const body = String(block.text || "");
  if (!mentionPrefix) return body;
  if (!body.trim()) return mentionPrefix;
  return `${mentionPrefix} ${body}`;
}

function showStreamingUi(block: ChatMessageBlock): boolean {
  return !!block.isStreaming && !isOwnMessage(block);
}

function normalizedStreamingPhaseLabel(block: ChatMessageBlock): string {
  const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
  const schedulingState = String((providerMeta as Record<string, unknown>)._schedulingState || "").trim();
  const rawContextPercent = (providerMeta as Record<string, unknown>)._contextUsagePercent;
  const contextUsagePercent = typeof rawContextPercent === "number"
    ? rawContextPercent
    : Number.parseInt(String(rawContextPercent || "").trim(), 10);
  const withWater = (text: string): string => {
    if (schedulingState === "waiting_response" && Number.isFinite(contextUsagePercent) && contextUsagePercent > 0) {
      return `${text}（${contextUsagePercent}%）`;
    }
    return text;
  };
  if (schedulingState) {
    switch (schedulingState) {
      case "preparing_context":
        return withWater(t("chat.statusPreparingMessage"));
      case "waiting_response":
        return withWater(t("chat.statusWaitingReply"));
      case "streaming_reasoning":
        return t("chat.statusThinking");
      case "streaming_text":
        return t("chat.statusTypingBody");
      case "streaming_tool":
        return t("chat.statusGeneratingTools");
      case "executing_tool":
        return t("chat.statusGeneratingTools");
      case "idle":
        return "";
      default:
        break;
    }
  }
  // Fallback：旧后端无 schedulingState 时，仅读 tool_status，不再以 hasReasoning -> 已阅读消息 推断
  const preStreamingStatusText = String(providerMeta._preStreamingStatusText || "").trim();
  const toolStatusText = String(providerMeta._toolStatusText || "").trim();
  const toolStatusState = String(providerMeta._toolStatusState || "").trim();
  const doingTool = toolCallsForBlock(block).some((call) => call.status === "doing");
  const hasSpeechContent = hasStreamingSpeechContent(block);

  const normalizeRequestPhaseText = (text: string): string => {
    if (!text) return "";
    if (text.includes("准备调度") || text.includes("处理附件") || text.includes("上下文")) {
      return t("chat.statusPreparingMessage");
    }
    if (
      text.includes("等待回应")
      || text.includes("等待响应")
      || text.includes("进入模型请求阶段")
      || text.includes("重新开始当前调度")
      || text.includes("重新发起")
      || text.includes("调度")
      || text.includes("模型请求")
    ) {
      return t("chat.statusWaitingReply");
    }
    return "";
  };

  if (doingTool || block.activityStatus === "running_tool") {
    return t("chat.statusGeneratingTools");
  }
  if (hasSpeechContent) {
    return t("chat.statusTypingBody");
  }
  if (toolStatusState === "running") {
    const requestPhase = normalizeRequestPhaseText(toolStatusText);
    if (requestPhase) return requestPhase;
  }
  if (preStreamingStatusText) {
    const requestPhase = normalizeRequestPhaseText(preStreamingStatusText);
    if (requestPhase) return requestPhase;
  }
  return t("chat.statusWaitingReply");
}

function assistantStreamingHeaderStatus(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return "";
  const withElapsed = (text: string): string => {
    const elapsed = frontendDispatchElapsedLabel(block);
    return elapsed ? `${text}（${elapsed}）` : text;
  };
  return withElapsed(normalizedStreamingPhaseLabel(block));
}

function showAssistantPreStreamingDots(block: ChatMessageBlock): boolean {
  if (!showStreamingUi(block)) return false;
  const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
  const preStreamingStatusText = String(providerMeta._preStreamingStatusText || "").trim();
  if (!preStreamingStatusText) return false;
  return !hasStreamingSpeechContent(block)
    && toolCallsForBlock(block).length === 0
    && !showActivityPanel(block)
    && block.images.length === 0
    && block.audios.length === 0
    && block.attachmentFiles.length === 0;
}

function bubbleContentEmpty(block: ChatMessageBlock): boolean {
  const own = isOwnMessage(block);
  const ownHasContent = own ? ownBubbleHasContent(block) : false;
  const assistantDots = !own && showAssistantPreStreamingDots(block);
  const assistantHasContent = !own ? assistantBubbleHasContent(block) : false;
  const empty = own ? !ownHasContent : assistantDots || !assistantHasContent;
  return empty;
}

function ownBubbleHasContent(block: ChatMessageBlock): boolean {
  return !!ownMessageDisplayText(block).trim()
    || (block.extraTextReferences?.length || 0) > 0
    || block.images.length > 0
    || block.audios.length > 0
    || block.attachmentFiles.length > 0;
}

function assistantBubbleHasContent(block: ChatMessageBlock): boolean {
  return hasStreamingSpeechContent(block)
    || !!block.planCard
    || block.images.length > 0
    || block.audios.length > 0
    || block.attachmentFiles.length > 0;
}

function hasStreamingSpeechContent(block: ChatMessageBlock): boolean {
  if (stripToolcallMarkers(block.text || "")) return true;
  if (Array.isArray(block.streamSegments) && block.streamSegments.some((item) => stripToolcallMarkers(String(item || "")))) return true;
  if (stripToolcallMarkers(block.streamTail || "")) return true;
  if (stripToolcallMarkers(block.streamAnimatedDelta || "")) return true;
  return false;
}

function shouldAnimateEnter(block: ChatMessageBlock): boolean {
  void block;
  return false;
}

function toolCallsForBlock(block: ChatMessageBlock): Array<{ name: string; argsText: string; status?: "doing" | "done" }> {
  return block.toolCalls;
}

function showActivityPanel(block: ChatMessageBlock): boolean {
  if (isOwnMessage(block) || block.remoteImOrigin) return false;
  const streamBlocks = assistantContentBlocksFromMessage(block);
  if (streamBlocks.length > 0) {
    const hasTrueContent = streamBlocks.some((b) => {
      if (String(b.reasoning || "").trim()) return true;
      if (Array.isArray(b.tools) && b.tools.some((t) => String(t.name || t.argsText || t.resultText || "").trim())) return true;
      return false;
    });
    if (hasTrueContent) return true;
    // 真块为空或仅纯文本时不抢跑，调度阶段仅由上面一行承接
    return false;
  }
  return block.activityItems.some((item) => hasExpandableActivityItem(item));
}

function showActivitySummary(block: ChatMessageBlock): boolean {
  if (isOwnMessage(block) || block.remoteImOrigin) return false;
  if (showActivityPanel(block)) return true;
  return !block.isStreaming;
}

function hasExpandableActivityItem(item: ChatActivityItem): boolean {
  if (item.kind === "reasoning") return !!String(item.text || "").trim();
  if (item.kind === "tool") return !!String(item.name || item.argsText || item.resultText || "").trim();
  return false;
}

function resolvedActivityItems(block: ChatMessageBlock): ChatActivityItem[] {
  if (!activityPanelOpen(block)) return block.activityItems;
  // 展开圆点明细时读取正式助理内容块，避免继续使用空参 summary。
  const streamBlocks = assistantContentBlocksFromMessage(block);
  if (streamBlocks.length <= 0) return block.activityItems;
  return streamBlocksToActivityItems(streamBlocks, !!block.activityRunning);
}

function activityShouldAutoExpand(block: ChatMessageBlock): boolean {
  void block;
  return false;
}

function activityPanelOpen(block: ChatMessageBlock): boolean {
  return activityExpanded.value || activityShouldAutoExpand(block);
}

function onActivityToggle(event: Event): void {
  activityExpanded.value = detailsOpenFromEvent(event);
  emit("activityToggle", { blockId: String(props.block.id || ""), open: activityExpanded.value });
}

function closeActivityDetails(): void {
  const details = activityDetailsRef.value;
  if (details instanceof HTMLDetailsElement) {
    details.open = false;
  }
  activityExpanded.value = false;
  emit("activityToggle", { blockId: String(props.block.id || ""), open: false });
}

function activityReasoningCountLabel(block: ChatMessageBlock): string {
  const count = Number(block.activityReasoningCharCount || 0);
  return count > 0 ? `(${count.toLocaleString("zh-CN")})` : "";
}

function activityStatusText(block: ChatMessageBlock): string {
  void block;
  return t('chat.messageItem.thinkingAndTools');
}

function activityToolCountsLabel(block: ChatMessageBlock): string {
  const counts = new Map<string, number>();
  const order: string[] = [];
  for (const item of block.activityItems) {
    if (item.kind !== "tool") continue;
    let name = toolCallDisplayName(item.name);
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) {
      name = `${name}(${t("chat.toolReview.denied") || "已拒绝"})`;
    } else if (status.isFailed) {
      name = `${name}(${t("chat.toolReview.failed") || "失败"})`;
    }
    if (!counts.has(name)) {
      counts.set(name, 0);
      order.push(name);
    }
    counts.set(name, (counts.get(name) || 0) + 1);
  }
  return order
    .map((name) => {
      const total = counts.get(name) || 0;
      return total > 1 ? `${name}(${total})` : name;
    })
    .join(" · ");
}

function activityItemsSignature(block: ChatMessageBlock): string {
  return resolvedActivityItems(block)
    .map((item) => {
      if (item.kind === "reasoning") {
        return [
          "r",
          String(item.id || "").trim(),
          textContentSignature(item.text),
          item.running ? "1" : "0",
        ].join(":");
      }
      if (item.kind === "content") {
        return [
          "c",
          String(item.id || "").trim(),
          textContentSignature(item.text),
          item.running ? "1" : "0",
        ].join(":");
      }
      return [
        "t",
        String(item.id || "").trim(),
        String(item.toolCallId || "").trim(),
        String(item.name || "").trim(),
        String(item.status || "").trim(),
        textContentSignature(item.argsText),
        textContentSignature(item.resultText),
      ].join(":");
    })
    .join("|");
}

function activityPanelMemoKey(block: ChatMessageBlock): unknown[] {
  const panelOpen = activityPanelOpen(block);
  return [
    String(block.id || "").trim(),
    showActivityPanel(block),
    activityExpanded.value,
    panelOpen,
    activityStatusText(block),
    activityReasoningCountLabel(block),
    activityToolCountsLabel(block),
    // 折叠时内容区不渲染，items 全文签名只用于展开态检测内容变化；
    // 数字/状态变化已由上面几项覆盖，折叠态跳过可避免流式时对思维链全文反复哈希。
    // 条目 details 为原生开合，不进 memoKey——点击条目不得触发面板重渲染。
    ...(panelOpen ? [activityItemsSignature(block)] : []),
  ];
}

function activityItemKey(item: ChatActivityItem): string {
  return `${item.kind}:${String(item.id || "")}`;
}

function activityItemText(item: ChatActivityItem): string {
  if (item.kind === "content") return stripToolcallMarkers(item.text);
  if (item.kind === "reasoning") return String(item.text || "");
  return "";
}

function activityItemTextParts(item: ChatActivityItem): { summary: string; remaining: string } {
  const text = activityItemText(item);
  const lineBreakIndex = text.search(/\r\n|\n|\r/);
  if (lineBreakIndex < 0) return { summary: text, remaining: "" };
  const lineBreakLength = text.startsWith("\r\n", lineBreakIndex) ? 2 : 1;
  return {
    summary: text.slice(0, lineBreakIndex),
    remaining: text.slice(lineBreakIndex + lineBreakLength),
  };
}

function activityItemCanExpand(item: ChatActivityItem): boolean {
  return item.kind === "tool" && !!activityToolArgsText(item);
}

function stripToolcallMarkers(text: string): string {
  return String(text || "").replace(/\[toolcall:[^\]\n]+\]/g, "");
}

function activityToolArgsText(item: ChatActivityItem): string {
  if (item.kind !== "tool") return "";
  const raw = String(item.argsText || "");
  if (!raw.trim()) return raw;
  try {
    const parsed = JSON.parse(raw);
    if (parsed !== null && parsed !== undefined && (typeof parsed === "object" || Array.isArray(parsed))) {
      return JSON.stringify(parsed, null, 2);
    }
  } catch {
    // 非 JSON 原文保留
  }
  return raw;
}

function activityItemNodeClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.markdownIsDark ? "ecall-activity-reasoning-dark" : "ecall-activity-reasoning";
  }
  if (item.kind === "content") return "text-base-content";
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "text-warning";
    if (status.isFailed) return "text-error";
  }
  return props.markdownIsDark ? "ecall-activity-tool-dark" : "ecall-activity-tool";
}

function activityItemTitleClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.markdownIsDark ? "italic ecall-activity-reasoning-dark" : "italic ecall-activity-reasoning";
  }
  if (item.kind === "content") return "text-base-content";
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "text-warning";
    if (status.isFailed) return "text-error";
  }
  return props.markdownIsDark ? "ecall-activity-tool-dark" : "ecall-activity-tool";
}

function activityItemDetailClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.markdownIsDark ? "ecall-activity-reasoning-dark" : "ecall-activity-reasoning";
  }
  if (item.kind === "content") return "text-base-content/80";
  return props.markdownIsDark ? "ecall-activity-tool-dark" : "ecall-activity-tool";
}

function activityItemTitle(item: ChatActivityItem): string {
  if (item.kind === "reasoning" || item.kind === "content") {
    return activityItemTextParts(item).summary;
  }
  const baseTitle = joinNonEmpty([
    toolCallDisplayName(item.name),
    toolCallSummaryText(item),
  ]);
  const status = parseToolCallResultStatus(item.resultText);
  if (status.isDenied) {
    return `${baseTitle} (${t("chat.toolReview.denied") || "已拒绝"})`;
  }
  if (status.isFailed) {
    return `${baseTitle} (${t("chat.toolReview.failed") || "失败"})`;
  }
  return baseTitle;
}

function countTextLines(text: string): number {
  const normalized = String(text || "").replace(/\r\n/g, "\n");
  if (!normalized.trim()) return 0;
  return normalized.split("\n").length;
}

function toolCallDiffStats(toolCall: { name: string; argsText: string; resultText?: string }): { adds: number; removes: number } {
  if (toolCall.resultText) {
    const status = parseToolCallResultStatus(toolCall.resultText);
    if (status.isDenied || status.isFailed) {
      return { adds: 0, removes: 0 };
    }
  }
  const toolName = String(toolCall.name || "").trim();
  const args = normalizeToolCallArgs(toolCall.argsText);
  if (typeof args !== "object" || args === null) return { adds: 0, removes: 0 };
  const obj = args as Record<string, unknown>;

  if (toolName === "write") {
    return {
      adds: countTextLines(String(obj.content || "")),
      removes: 0,
    };
  }

  if (toolName === "update") {
    const oldLines = countTextLines(String(obj.oldString || ""));
    const newLines = countTextLines(String(obj.newString || ""));
    return {
      adds: newLines,
      removes: oldLines,
    };
  }

  return { adds: 0, removes: 0 };
}

function activityToolDetailsText(item: ChatActivityItem): string {
  if (item.kind !== "tool") return "";
  const args = activityToolArgsText(item);
  if (!item.resultText) return args;
  const status = parseToolCallResultStatus(item.resultText);
  if (status.isDenied) {
    const reason = status.blockedReason || status.message;
    const label = reason
      ? `\n\n[${t("chat.toolReview.denied") || "已拒绝"}]: ${reason}`
      : `\n\n[${t("chat.toolReview.denied") || "已拒绝"}]`;
    return `${args}${label}`;
  }
  if (status.isFailed) {
    const reason = status.message || status.blockedReason;
    const label = reason
      ? `\n\n[${t("chat.toolReview.failed") || "执行失败"}]: ${reason}`
      : `\n\n[${t("chat.toolReview.failed") || "执行失败"}]`;
    return `${args}${label}`;
  }
  return args;
}

function activityItemDisplay(item: ChatActivityItem): { text: string; adds: number; removes: number } {
  if (item.kind !== "tool") {
    return { text: activityItemTitle(item), adds: 0, removes: 0 };
  }
  return {
    text: activityItemTitle(item),
    ...toolCallDiffStats(item),
  };
}

function toolStatusLabel(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return t('chat.messageItem.toolDone');
  return toolSummaryDoing(block) ? t('chat.messageItem.toolRunning') : t('chat.messageItem.toolDone');
}

function toolSummaryDoing(block: ChatMessageBlock): boolean {
  if (!showStreamingUi(block)) return false;
  return toolCallsForBlock(block).some((call) => String(call.status || "").trim() === "doing");
}

function toolTimelineDotClass(block: ChatMessageBlock, toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
  if (!showStreamingUi(block)) return "bg-success";
  return toolCall.status === "doing" ? "bg-primary" : "bg-success";
}

function toolTimelineHrClass(block: ChatMessageBlock, toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
  if (!showStreamingUi(block)) return "bg-success/35";
  return toolCall.status === "doing" ? "bg-primary/35" : "bg-success/35";
}

function toolNamesLabel(block: ChatMessageBlock): string {
  const calls = toolCallsForBlock(block);
  if (calls.length === 0) return "";
  const counts = new Map<string, number>();
  const order: string[] = [];
  for (const call of calls) {
    const name = toolCallDisplayName(String(call.name || "").trim()) || toolTimelineText("unknownTool");
    if (!counts.has(name)) {
      counts.set(name, 0);
      order.push(name);
    }
    counts.set(name, (counts.get(name) || 0) + 1);
  }
  return order
    .map((name) => {
      const total = counts.get(name) || 0;
      return total > 1 ? `${name}（+${total - 1}）` : name;
    })
    .join("，");
}

function formatDispatchElapsed(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(Number(ms || 0) / 1000));
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const padded = (value: number) => String(value).padStart(2, "0");
  if (days > 0) return t('chat.messageItem.durationDays', { days, hours: padded(hours), minutes: padded(minutes), seconds: padded(seconds) });
  if (hours > 0) return t('chat.messageItem.durationHours', { hours: padded(hours), minutes: padded(minutes), seconds: padded(seconds) });
  if (minutes > 0) return t('chat.messageItem.durationMinutes', { minutes: padded(minutes), seconds: padded(seconds) });
  return t('chat.messageItem.durationSeconds', { seconds: padded(seconds) });
}

function numericMetaValue(block: ChatMessageBlock, key: string): number {
  const fromBlock = Number((block as ChatMessageBlock & Record<string, unknown>)[key]);
  if (Number.isFinite(fromBlock) && fromBlock > 0) return fromBlock;
  const meta = (block.providerMeta || {}) as Record<string, unknown>;
  const fromMeta = Number(meta[key]);
  return Number.isFinite(fromMeta) && fromMeta > 0 ? fromMeta : 0;
}

function frontendDispatchElapsedLabel(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return "";
  const messageId = String(block.sourceMessageId || block.id || "").trim();
  // 优先读独立计时器状态：它每秒更新但不触碰消息对象，避免带动虚拟列表重算；
  // 无活跃计时器时（历史消息/缓存恢复）回退读 block 投影里的耗时字段。
  const liveElapsedMs = messageId ? frontendDispatchElapsedByMessageId.get(messageId) : undefined;
  const elapsedMs = liveElapsedMs ?? (numericMetaValue(block, "frontendDispatchElapsedMs")
    || numericMetaValue(block, "_frontendDispatchElapsedMs"));
  const startedAtMs = numericMetaValue(block, "_frontendDispatchStartedAtMs");
  if (elapsedMs <= 0 && startedAtMs <= 0) return "";
  return formatDispatchElapsed(elapsedMs);
}

function padTimePart(value: number): string {
  return String(value).padStart(2, "0");
}

function formatRecentRelativeTime(value: string | undefined, nowMs: number): string {
  const raw = String(value || "").trim();
  if (!raw) return "";
  const date = new Date(raw);
  const timestamp = date.getTime();
  if (!Number.isFinite(timestamp)) return raw;

  const diffMs = Math.max(0, nowMs - timestamp);
  const seconds = Math.floor(diffMs / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) return t("config.memory.justNow");
  if (minutes < 60) return t("config.memory.minutesAgo", { count: minutes });
  if (hours < 24) return t("config.memory.hoursAgo", { count: hours });
  if (days < 7) return t("config.memory.daysAgo", { count: days });

  const now = new Date(nowMs);
  const year = date.getFullYear();
  const monthDay = `${padTimePart(date.getMonth() + 1)}-${padTimePart(date.getDate())}`;
  const clock = `${padTimePart(date.getHours())}:${padTimePart(date.getMinutes())}`;
  if (year === now.getFullYear()) return `${monthDay} ${clock}`;
  return `${year}-${monthDay}`;
}

function handleSelectionRowClick(event: MouseEvent): void {
  if (!props.selectionModeEnabled) return;
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('[data-selection-ignore="true"], button, a, input, textarea, select, summary, label')) {
    return;
  }
  emit("toggleMessageSelected", props.selectionKey);
}

function hasNativeTextSelection(): boolean {
  try {
    const selection = window.getSelection?.();
    return !!selection && selection.rangeCount > 0 && !selection.isCollapsed && !!String(selection.toString() || "").trim();
  } catch {
    return false;
  }
}

function handleGlobalPointerDownForContextMenu(event: PointerEvent) {
  const target = event.target;
  if (!(target instanceof Node)) {
    closeContextMenu();
    return;
  }
  if (contextMenuRef.value?.contains(target)) return;
  closeContextMenu();
}

function openContextMenu(event: MouseEvent) {
  if (hasNativeTextSelection()) {
    closeContextMenu();
    return;
  }
  mathContextCopyText.value = "";
  event.preventDefault();
  const menuWidth = 176; // w-44
  const menuHeight = 236; // estimate
  const margin = 8;
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  contextMenuX.value = Math.min(Math.max(margin, event.clientX), viewportWidth - menuWidth - margin);
  contextMenuY.value = Math.min(Math.max(margin, event.clientY), viewportHeight - menuHeight - margin);
  contextMenuOpen.value = true;
  window.addEventListener("pointerdown", handleGlobalPointerDownForContextMenu, true);
}

function openMathContextMenu(payload: { clientX: number; clientY: number; copyText: string }) {
  if (!String(payload.copyText || "").trim()) return;
  const menuWidth = 176; // w-44
  const menuHeight = 272; // estimate with extra entries
  const margin = 8;
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  mathContextCopyText.value = payload.copyText;
  contextMenuX.value = Math.min(Math.max(margin, payload.clientX), viewportWidth - menuWidth - margin);
  contextMenuY.value = Math.min(Math.max(margin, payload.clientY), viewportHeight - menuHeight - margin);
  contextMenuOpen.value = true;
  window.addEventListener("pointerdown", handleGlobalPointerDownForContextMenu, true);
}

function closeContextMenu() {
  contextMenuOpen.value = false;
  mathContextCopyText.value = "";
  window.removeEventListener("pointerdown", handleGlobalPointerDownForContextMenu, true);
}

function closeRawMessageData() {
  rawMessageDataOpen.value = false;
}

async function copyTextToClipboard(text: string) {
  if (!String(text || "").trim()) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {}
}

function handleContextMenuAction(action: string) {
  const mathCopyText = mathContextCopyText.value;
  closeContextMenu();
  if (action === "select") {
    // 多选模式忙碌时禁用；分支/转发等子代理操作不受影响
    if (props.chatting || props.busy || props.frozen) return;
    emit("enterSelectionMode", props.selectionKey);
  } else if (action === "copy") {
    emit("copyMessage", props.block);
  } else if (action === "copyMath") {
    void copyTextToClipboard(mathCopyText);
  } else if (action === "copyAsImage") {
    void copyCurrentMessageAsImage();
  } else if (action === "showRawData") {
    if (isDevBuild) rawMessageDataOpen.value = true;
  } else if (action === "branchFromMessage") {
    const turnId = recallTurnId(props.block);
    if (!turnId) return;
    emit("createConversationBranchFromTurn", { turnId });
  } else if (action === "recall") {
    const turnId = recallTurnId(props.block);
    if (!turnId) return;
    emit("recallTurn", { turnId });
  }
}

function currentMessageShareBlock(): ChatMessageBlock | null {
  const sourceId = String(props.block.sourceMessageId || props.block.id || "").trim();
  if (!sourceId) return null;
  return props.block;
}

async function copyCurrentMessageAsImage() {
  if (copyMessageImageBusy.value || props.selectionModeEnabled) return;
  const messageBlock = currentMessageShareBlock();
  if (!messageBlock) return;
  copyMessageImageBusy.value = true;
  try {
    if (!navigator.clipboard?.write || typeof ClipboardItem === "undefined") {
      emit("copyMessageImageFailed");
      return;
    }
    const generated = await generateShareFromMessageIds({
      conversationId: String(props.activeConversationId || "").trim(),
      messageIds: [String(messageBlock.sourceMessageId || messageBlock.id || "").trim()].filter(Boolean),
      formats: ["png"],
      title: String(t("chat.shareDocumentTitle")),
      subtitle: String(t("chat.shareDocumentSubtitle", { count: 1 })),
      userAlias: props.userAlias,
      userAvatarUrl: props.userAvatarUrl,
      personaNameMap: props.personaNameMap,
      personaAvatarUrlMap: props.personaAvatarUrlMap,
      trigger: "single_message_copy_image",
    });
    const dataUrl = String(generated.pngDataUrl || "");
    if (!dataUrl) {
      emit("copyMessageImageFailed");
      return;
    }
    const blob = await (await fetch(dataUrl)).blob();
    if (blob.type !== "image/png") {
      emit("copyMessageImageFailed");
      return;
    }
    await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
    emit("copyMessageImageDone");
  } catch (error) {
    console.warn("[消息复制图片] 失败", error);
    emit("copyMessageImageFailed");
  } finally {
    copyMessageImageBusy.value = false;
  }
}

function formatThinkAsMarkdown(raw: string): string {
  const input = raw || "";
  const openTag = "<think>";
  const closeTag = "</think>";
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const openIdx = input.indexOf(openTag, cursor);
    if (openIdx < 0) {
      output += input.slice(cursor);
      break;
    }

    output += input.slice(cursor, openIdx);
    const afterOpen = openIdx + openTag.length;
    const closeIdx = input.indexOf(closeTag, afterOpen);
    if (closeIdx < 0) {
      const tail = input.slice(afterOpen).trim();
      if (tail) output += `\n\n*${tail}*`;
      cursor = input.length;
      break;
    }

    const inner = input.slice(afterOpen, closeIdx).trim();
    if (inner) output += `\n\n*${inner}*\n\n`;
    cursor = closeIdx + closeTag.length;
  }

  return output.trim();
}

function formatAssistantStreamingText(block: ChatMessageBlock): string {
  const rendered = formatThinkAsMarkdown(String(block.text || ""));
  if (!block.isStreaming || isOwnMessage(block)) return rendered;
  return hideIncompleteInlineMath(hideIncompleteDisplayMath(rendered));
}

function hideIncompleteDisplayMath(text: string): string {
  if (!text.includes("$$")) return text;

  const lines = text.split("\n");
  let inCodeFence = false;
  let offset = 0;
  let openMathStart = -1;

  for (const line of lines) {
    if (/^\s*```/.test(line)) {
      inCodeFence = !inCodeFence;
      offset += line.length + 1;
      continue;
    }
    if (!inCodeFence) {
      let searchFrom = 0;
      while (searchFrom < line.length) {
        const delimiterIndex = findUnescapedDoubleDollar(line, searchFrom);
        if (delimiterIndex < 0) break;
        const absoluteIndex = offset + delimiterIndex;
        openMathStart = openMathStart >= 0 ? -1 : absoluteIndex;
        searchFrom = delimiterIndex + 2;
      }
    }
    offset += line.length + 1;
  }

  if (openMathStart < 0) return text;
  return text.slice(0, openMathStart);
}

function findUnescapedDoubleDollar(text: string, from: number): number {
  let cursor = Math.max(0, from);
  while (cursor < text.length) {
    const index = text.indexOf("$$", cursor);
    if (index < 0) return -1;
    let backslashCount = 0;
    for (let i = index - 1; i >= 0 && text[i] === "\\"; i -= 1) {
      backslashCount += 1;
    }
    if (backslashCount % 2 === 0) return index;
    cursor = index + 2;
  }
  return -1;
}

function normalizeRenderedLocalLinks() {
  const container = markdownContainerRef.value;
  if (!container) return;
  const anchors = Array.from(container.querySelectorAll("a[href]"));
  for (const anchor of anchors) {
    const rawHref = anchor.getAttribute("href")?.trim() || "";
    const normalizedHref = normalizeLocalLinkHref(rawHref);
    if (normalizedHref && normalizedHref !== rawHref) {
      anchor.setAttribute("href", normalizedHref);
    }
  }
}

function blockWideContentText(block: ChatMessageBlock): string {
  return formatAssistantStreamingText(block);
}

function blockHasMermaid(block: ChatMessageBlock): boolean {
  return /```(?:\s*)mermaid\b/i.test(blockWideContentText(block));
}

function blockHasCodeFence(block: ChatMessageBlock): boolean {
  return /```[\w-]*\s*[\r\n]/i.test(blockWideContentText(block));
}

function blockNeedsWideBubble(block: ChatMessageBlock): boolean {
  return blockHasMermaid(block) || blockHasCodeFence(block);
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}

function imageCacheKey(image: { mime: string; bytesBase64?: string; mediaRef?: string }): string {
  const mime = String(image.mime || "").trim().toLowerCase();
  const mediaRef = String(image.mediaRef || "").trim();
  if (mediaRef) return `${mime}::${mediaRef}`;
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  return `${mime}::inline::${bytesBase64}`;
}

function imageRenderKey(index: number): string {
  return `${String(props.block.id || "").trim() || "message"}::${index}`;
}

async function loadImageDataUrl(image: { mime: string; bytesBase64?: string; mediaRef?: string }): Promise<string> {
  const mime = String(image.mime || "").trim() || "image/webp";
  const bytesBase64 = String(image.bytesBase64 || "").trim();
  if (bytesBase64) {
    return `data:${mime};base64,${bytesBase64}`;
  }
  const mediaRef = String(image.mediaRef || "").trim();
  if (!mediaRef) return "";
  const cacheKey = imageCacheKey(image);
  const cached = imageDataUrlCache.get(cacheKey);
  if (cached) return cached;
  const pending = imageDataUrlPromiseCache.get(cacheKey);
  if (pending) return pending;
  const legacyMarker = mediaRef.startsWith("@media:") || mediaRef.startsWith("@download:");
  const task = readTransportChatImage({
    ...(legacyMarker ? { mediaRef } : { path: mediaRef }),
    mime,
  })
    .then((result) => {
      const dataUrl = String(result?.dataUrl || "").trim();
      if (dataUrl) imageDataUrlCache.set(cacheKey, dataUrl);
      imageDataUrlPromiseCache.delete(cacheKey);
      return dataUrl;
    })
    .catch((error) => {
      imageDataUrlPromiseCache.delete(cacheKey);
      throw error;
    });
  imageDataUrlPromiseCache.set(cacheKey, task);
  return task;
}

watchEffect(() => {
  const nextEntries = props.block.images
    .map((image, index) => {
      const src = image.bytesBase64
        ? `data:${image.mime};base64,${image.bytesBase64}`
        : "";
      return [imageRenderKey(index), src] as const;
    })
    .filter((entry) => !!entry[1]);
  if (nextEntries.length <= 0) return;
  resolvedImageSrcMap.value = {
    ...resolvedImageSrcMap.value,
    ...Object.fromEntries(nextEntries),
  };
});

watchEffect(() => {
  for (const [index, image] of props.block.images.entries()) {
    if (!isImageMime(image.mime) || image.bytesBase64 || !image.mediaRef) continue;
    const key = imageRenderKey(index);
    if (resolvedImageSrcMap.value[key]) continue;
    void loadImageDataUrl(image)
      .then((dataUrl) => {
        if (!dataUrl || disposed) return;
        resolvedImageSrcMap.value = {
          ...resolvedImageSrcMap.value,
          [key]: dataUrl,
        };
      })
      .catch((error) => {
        console.warn("[聊天图片] 懒加载失败", {
          messageId: props.block.id,
          mediaRef: image.mediaRef,
          error,
        });
      });
  }
});

watchPostEffect(() => {
  void nextTick(() => {
    normalizeRenderedLocalLinks();
  });
});

onMounted(() => {
  relativeTimeNowTimer = window.setInterval(() => {
    relativeTimeNowTick.value = Date.now();
  }, 60_000);
});

onBeforeUnmount(() => {
  closeContextMenu();
  if (relativeTimeNowTimer) {
    window.clearInterval(relativeTimeNowTimer);
    relativeTimeNowTimer = 0;
  }
  disposed = true;
});

function resolvedImageSrc(
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
): string {
  const direct = String(image.bytesBase64 || "").trim();
  if (direct) return `data:${image.mime};base64,${direct}`;
  return String(resolvedImageSrcMap.value[imageRenderKey(index)] || "").trim();
}

function openResolvedImagePreview(
  image: { mime: string; bytesBase64?: string; mediaRef?: string },
  index: number,
) {
  const dataUrl = resolvedImageSrc(image, index);
  if (!dataUrl) return;
  emit("openImagePreview", {
    mime: image.mime,
    dataUrl,
    localPath: image.mediaRef && !image.mediaRef.startsWith("@") ? image.mediaRef : undefined,
  });
}

function openAttachmentPath(path: string) {
  const normalized = String(path || "").trim();
  if (!normalized) return;
  void openTransportWorkspaceFile(normalized).catch((error) => {
    console.warn("[聊天附件] 打开失败", { path: normalized, error });
  });
}
</script>

<style scoped>
/* 浅色主题：思维链橙色、工具绿色（加深保证白底可读） */
/* :deep 穿透：颜色类经 textClass 传入 ExpandableText 内部文本元素，scoped 规则需跨组件生效 */
:deep(.ecall-activity-reasoning) {
  color: #c2410c;
}

:deep(.ecall-activity-tool) {
  color: #15803d;
}

/* 深色主题：思维链橙色、工具绿色（提亮保证深底可读） */
:deep(.ecall-activity-reasoning-dark) {
  color: #fb923c;
}

:deep(.ecall-activity-tool-dark) {
  color: #4ade80;
}

.ecall-activity-item-summary {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

/* 条目 details 原生开合：chevron 旋转由 details[open] 驱动，不进 Vue 状态 */
.ecall-activity-timeline details[open] .ecall-activity-chevron {
  transform: rotate(180deg);
}

.ecall-chat-message-row {
  width: 100%;
}

.ecall-chat-message-row-selectable {
  padding-inline: 2rem;
}

.ecall-message-selection-control {
  position: absolute;
  top: 0.45rem;
  z-index: 2;
  display: flex;
  width: 1.25rem;
  justify-content: center;
}

.ecall-message-selection-control-left {
  left: 0.35rem;
}

.ecall-message-selection-control-right {
  right: 0.35rem;
}

.ecall-message-continued {
  padding-top: 0;
}

.ecall-meme-segment-flow {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.35rem 0.45rem;
}

.ecall-meme-text-segment {
  min-width: 0;
}

.ecall-inline-meme {
  display: inline-block;
  max-height: 4.5rem;
  max-width: min(8rem, 40vw);
  border-radius: 0.85rem;
  object-fit: contain;
  vertical-align: middle;
}

.ecall-local-image-wrapper {
  display: inline-block;
  vertical-align: middle;
}

.ecall-local-image-thumbnail {
  max-height: 18rem;
  max-width: min(28rem, 80vw);
  border-radius: 0.5rem;
  object-fit: contain;
  cursor: zoom-in;
}

.ecall-local-image-placeholder {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 6rem;
  max-width: min(16rem, 60vw);
  height: 4rem;
  border-radius: 0.5rem;
  opacity: 0.5;
  font-size: var(--app-text-sm-size);
  overflow: hidden;
  text-overflow: ellipsis;
}

.ecall-local-image-unavailable {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 4rem;
  max-width: min(12rem, 50vw);
  height: 3rem;
  border-radius: 0.5rem;
  opacity: 0.3;
  font-size: var(--app-text-xs-size);
  border: 1px dashed currentColor;
  overflow: hidden;
  text-overflow: ellipsis;
}


.ecall-inline-meme-markdown:deep(.markdown-renderer),
.ecall-inline-meme-markdown:deep(.node-slot),
.ecall-inline-meme-markdown:deep(.node-content) {
  display: inline;
}

.ecall-inline-meme-markdown:deep(.paragraph-node),
.ecall-inline-meme-markdown:deep(.text-node),
.ecall-inline-meme-markdown:deep(.strong-node),
.ecall-inline-meme-markdown:deep(.emphasis-node),
.ecall-inline-meme-markdown:deep(.link-node),
.ecall-inline-meme-markdown:deep(.inline-code-node) {
  display: inline;
  margin: 0;
}

.ecall-message-enter {
  animation: ecall-message-enter 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

@keyframes ecall-message-enter {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: var(--color-base-content);
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: color-mix(in srgb, var(--color-base-content) 50%, transparent);
  --tw-prose-hr: color-mix(in srgb, var(--color-base-content) 15%, transparent);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: color-mix(in srgb, var(--color-base-content) 20%, transparent);
  --tw-prose-captions: color-mix(in srgb, var(--color-base-content) 75%, transparent);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: var(--color-base-200);
  --tw-prose-th-borders: color-mix(in srgb, var(--color-base-content) 20%, transparent);
  --tw-prose-td-borders: color-mix(in srgb, var(--color-base-content) 15%, transparent);
}

.assistant-markdown :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow: visible !important;
  max-height: none !important;
  height: auto !important;
  font-family: inherit;
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size));
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content .paragraph-node),
.assistant-markdown :deep(.ecall-markdown-content .heading-node),
.assistant-markdown :deep(.ecall-markdown-content .list-node),
.assistant-markdown :deep(.ecall-markdown-content .list-item),
.assistant-markdown :deep(.ecall-markdown-content .blockquote),
.assistant-markdown :deep(.ecall-markdown-content .link-node),
.assistant-markdown :deep(.ecall-markdown-content .strong-node),
.assistant-markdown :deep(.ecall-markdown-content .inline-code),
.assistant-markdown :deep(.ecall-markdown-content .table-node-wrapper),
.assistant-markdown :deep(.ecall-markdown-content .hr-node) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content.markdown-renderer) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.ecall-markdown-content .markdown-renderer),
.assistant-markdown :deep(.ecall-markdown-content .node-slot),
.assistant-markdown :deep(.ecall-markdown-content .node-content),
.assistant-markdown :deep(.ecall-markdown-content .text-node) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content .code-block-container),
.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.ecall-markdown-content > :first-child) {
  margin-top: 0;
}

.assistant-markdown :deep(.ecall-markdown-content > :last-child) {
  margin-bottom: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(p,ul,ol,blockquote,pre,table,figure,.paragraph-node,.list-node,.blockquote,.table-node-wrapper,.code-block-container,._mermaid,.vmr-container)) {
  margin-top: 0.25rem;
  margin-bottom: 0.25rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4,.heading-node)) {
  margin-top: 0.7rem;
  margin-bottom: 0.32rem;
  line-height: 1.5;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,.heading-node.heading-1)) {
  font-size: var(--app-text-markdown-heading-1-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h2,.heading-node.heading-2)) {
  font-size: var(--app-text-markdown-heading-2-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h3,.heading-node.heading-3)) {
  font-size: var(--app-text-markdown-heading-3-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h4,.heading-node.heading-4)) {
  font-size: var(--app-text-markdown-heading-4-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node)) {
  padding-left: 1.05rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item)) {
  margin: 0.12rem 0;
  padding-left: 0;
  line-height: 1.65;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item) > :where(p,ul,ol,.paragraph-node,.list-node)) {
  margin-top: 0.2rem;
  margin-bottom: 0.2rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  padding: 0.5rem 0.68rem 0.5rem 0.82rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote) .markdown-renderer),
.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node,li,.list-item) .markdown-renderer) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content :where(hr,.hr-node)) {
  margin: 0.65rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(:not(pre) > code,.inline-code):not(.code-block-container *)) {
  font-size: var(--app-text-xs-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(table,.table-node)) {
  font-size: var(--app-text-sm-size);
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
}

.assistant-markdown {
  --ecall-chat-rich-block-bg: var(--color-base-100);
}

/* 有气泡背景且不分段：富块嵌在 base-100 气泡内，用 base-200 拉开层次；其余场景富块独立裸排，一律 base-100 */
.assistant-markdown[data-bubble-background="on"][data-segmented-markdown="off"] {
  --ecall-chat-rich-block-bg: var(--color-base-200);
}

.assistant-markdown :deep(.ecall-md-code-block) {
  --ecall-md-code-bg: var(--ecall-chat-rich-block-bg);
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  background: var(--ecall-chat-rich-block-bg);
}

.assistant-markdown :deep(.ecall-markdown-content :where(th,.table-node th)) {
  background: var(--ecall-chat-rich-block-bg) !important;
}

.assistant-markdown :deep(.ecall-markdown-content :where(td,.table-node td)) {
  background: var(--ecall-chat-rich-block-bg) !important;
}

.assistant-markdown :deep(.ecall-markdown-content .ecall-md-details) {
  background: var(--ecall-chat-rich-block-bg);
}

.ecall-assistant-segment-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.ecall-assistant-segment {
  min-width: 0;
}

/* 隐藏气泡背景：每个气泡各补一条顶边线当分隔；线贯穿消息内容区并与正文左右对齐，不占高度 */
.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment {
  position: relative;
  width: 100%;
}

.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment::before {
  position: absolute;
  top: 0;
  right: 1rem;
  left: 1rem;
  height: 1px;
  background: color-mix(in srgb, var(--color-base-content) 14%, transparent);
  content: "";
  pointer-events: none;
  transform: scaleY(0.5);
  transform-origin: center;
}

.ecall-assistant-segment-text {
  display: inline-block;
  width: fit-content;
  max-width: 100%;
  padding: 0.68rem 1rem;
}

.ecall-assistant-segment-rich {
  display: block;
  width: 100%;
  padding: 0;
}

/* 背景开关：只决定气泡底色是否显示，布局与文字位置恒定不动 */
.ecall-assistant-bubble[data-bubble-background="on"] .ecall-assistant-segment-text {
  border-radius: var(--radius-box, 1rem);
  background: var(--color-base-100);
}

.ecall-assistant-bubble {
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size));
  transition:
    box-shadow 220ms ease,
    transform 220ms ease,
    border-color 220ms ease,
    background-color 220ms ease;
  transform-origin: top left;
}

.ecall-assistant-bubble-wide {
  display: block;
  width: 100%;
  max-width: none;
}

</style>
