<template>
  <div
    :data-message-id="String(block.id || '')"
    :data-message-role="isOwnMessage(block) ? 'user' : block.role"
    :data-active-turn-user="activeTurnUser ? 'true' : undefined"
    :class="[
      'chat group/user-turn rounded-2xl px-3 transition-colors',
      shouldAnimateEnter(block) ? 'ecall-message-enter' : '',
      isOwnMessage(block) ? 'chat-end' : 'chat-start',
      !isOwnMessage(block) && bubbleBackgroundHidden && !selectionModeEnabled ? 'ecall-message-simple' : '',
      isOwnMessage(block) && compactWithPrevious ? 'ecall-message-continued' : '',
      selectionModeEnabled && selected ? 'ecall-message-selected bg-neutral/10 ring-1 ring-neutral/20 shadow-sm' : '',
    ]"
    @click="handleSelectionRowClick"
  >
    <div
      v-if="selectionModeEnabled && isOwnMessage(block)"
      class="flex w-5 min-w-5 items-start justify-center self-stretch pt-1"
    >
      <button
        type="button"
        data-selection-ignore="true"
        class="inline-flex h-4 w-4 items-center justify-center rounded-sm border transition-colors"
        :class="selected
          ? 'border-primary bg-primary text-primary-content'
          : 'border-base-300 bg-base-100 text-transparent hover:border-primary/60'"
        :title="selected ? '取消选择' : '选择消息'"
        @click.stop="emit('toggleMessageSelected', selectionKey)"
      >
        <span class="text-[10px] leading-none">✓</span>
      </button>
    </div>
    <div class="chat-image self-start ecall-chat-avatar-col" data-message-avatar-anchor="true">
      <div
        class="flex w-7 flex-col items-center gap-2"
        :class="compactWithPrevious ? 'invisible pointer-events-none' : ''"
        :aria-hidden="compactWithPrevious ? 'true' : undefined"
      >
        <div class="avatar">
          <div class="w-7 rounded-full">
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              :alt="displayName"
              class="w-7 h-7 rounded-full object-cover"
            />
            <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-xs">
              {{ avatarInitial(displayName) }}
            </div>
          </div>
        </div>
        <button
          v-if="selectionModeEnabled && !isOwnMessage(block)"
          type="button"
          data-selection-ignore="true"
          class="inline-flex h-4 w-4 items-center justify-center rounded-sm border transition-colors"
          :class="selected
            ? 'border-primary bg-primary text-primary-content'
            : 'border-base-300 bg-base-100 text-transparent hover:border-primary/60'"
          :title="selected ? '取消选择' : '选择消息'"
          @click.stop="emit('toggleMessageSelected', selectionKey)"
        >
          <span class="text-[10px] leading-none">✓</span>
        </button>
      </div>
    </div>
    <template v-if="!isOwnMessage(block)">
      <div class="ecall-message-stack min-w-0 flex flex-col self-stretch">
        <div
          :class="[
            'ecall-message-content min-w-0',
            blockNeedsWideBubble(block) ? 'ecall-message-content-wide' : '',
          ]"
        >
          <div class="chat-header mb-1 flex items-baseline gap-2">
            <template v-if="streamingHeaderStatus">
              <span
                class="text-xs font-semibold text-base-content opacity-80 ecall-shimmer-text ecall-reasoning-shimmer"
                :data-shimmer-text="streamingHeaderStatus"
              >{{ streamingHeaderStatus }}</span>
            </template>
            <template v-else>
              <span class="text-xs text-base-content opacity-80">{{ displayName }}</span>
              <span class="text-xs leading-none">
                <time v-if="formattedCreatedAt && !block.isStreaming" class="text-base-content/40 leading-none">{{ formattedCreatedAt }}</time>
              </span>
            </template>
          </div>
          <div
            v-if="!showAssistantPreStreamingDots(block)"
            :class="[
              'chat-bubble',
              'self-start text-base-content assistant-markdown ecall-assistant-bubble max-w-full',
              bubbleBackgroundHidden ? 'ecall-message-bubble-bg-hidden' : 'bg-base-100 border border-base-300/70',
              blockNeedsWideBubble(block) ? 'ecall-assistant-bubble-wide' : '',
            ]"
          >
            <div v-if="block.planCard" class="space-y-3">
              <div class="flex items-center gap-2">
                <span class="badge badge-sm badge-ghost">
                  {{ block.planCard.action === "complete" ? t("chat.plan.completeBadge") : t("chat.plan.badge") }}
                </span>
              </div>
              <div class="whitespace-pre-wrap text-sm leading-6">{{ block.planCard.context }}</div>
              <div v-if="block.planCard.action === 'present'" class="space-y-2">
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
            <div v-if="block.taskTrigger" class="space-y-2">
        <div class="flex items-center gap-2">
          <span class="badge badge-sm badge-ghost">{{ t("chat.taskTrigger.badge") }}</span>
        </div>
        <div v-if="block.taskTrigger.taskId" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.taskId") }}</div>
          <div class="font-mono text-xs leading-6 break-all">{{ block.taskTrigger.taskId }}</div>
        </div>
        <div v-if="block.taskTrigger.goal" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.goal") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.goal }}</div>
        </div>
        <div v-if="block.taskTrigger.why" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.why") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.why }}</div>
        </div>
        <div v-if="block.taskTrigger.todo" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.todo") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.todo }}</div>
        </div>
        <div v-if="block.taskTrigger.runAtLocal || block.taskTrigger.endAtLocal || block.taskTrigger.nextRunAtLocal || block.taskTrigger.everyMinutes" class="grid gap-1 text-sm leading-6">
          <div v-if="block.taskTrigger.runAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.runAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.runAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.nextRunAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.nextRunAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.nextRunAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.endAtLocal">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.endAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.endAtLocal) }}</span>
          </div>
          <div v-if="block.taskTrigger.everyMinutes">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.everyMinutes") }}</span>
            <span class="ml-2">{{ block.taskTrigger.everyMinutes }}</span>
          </div>
        </div>
      </div>
      <div v-if="!isOwnMessage(block) && block.reasoningStandard" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55" @toggle="onReasoningStandardToggle">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 hover:bg-base-200">
            <span class="inline-block shrink-0 text-[10px] leading-none text-success">▲</span>
            <span
              class="block min-w-0 flex-1 truncate font-medium"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div
            v-if="reasoningStandardExpanded"
            class="collapse-content px-0 pb-1 pt-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70"
            @click="collapseDetailsFromContentClick"
          >
            {{ block.reasoningStandard }}
          </div>
        </details>
      </div>
      <div v-if="!isOwnMessage(block) && resolvedInlineReasoning(block)" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55" @toggle="onInlineReasoningToggle">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 cursor-pointer hover:bg-base-200">
            <span class="inline-block shrink-0 text-[10px] leading-none text-success">▲</span>
            <span
              class="block min-w-0 flex-1 truncate font-medium"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div
            v-if="inlineReasoningExpanded"
            class="collapse-content px-0 pb-1 pt-2 whitespace-pre-wrap text-xs leading-relaxed text-base-content/70"
            @click="collapseDetailsFromContentClick"
          >
            {{ resolvedInlineReasoning(block) }}
          </div>
        </details>
      </div>
      <div v-if="toolCallsForBlock(block).length > 0" class="flex flex-col opacity-90">
        <details class="collapse border-l-2 border-base-content/20 pl-3 rounded-none min-w-55" @toggle="onToolCallsToggle">
          <summary class="collapse-title py-1 px-1 min-h-0 text-xs font-semibold flex items-center gap-1.5 text-base-content/80 hover:bg-base-200">
            <span
              v-if="toolSummaryDoing(block)"
              class="relative inline-flex h-2 w-2 shrink-0 overflow-visible text-success"
            >
              <span
                class="loading loading-spinner loading-md absolute left-1/2 top-1/2 h-5 w-5 max-h-none max-w-none -translate-x-1/2 -translate-y-1/2 text-success"
              ></span>
            </span>
            <span v-else class="inline-block h-2 w-2 rounded-full bg-success"></span>
            <span
              class="font-medium"
            >{{ toolStatusLabel(block) }}</span>
            <span v-if="toolNamesLabel(block)" class="truncate">{{ ` · ${toolNamesLabel(block)}` }}</span>
          </summary>
          <div
            v-if="toolCallsExpanded"
            class="collapse-content px-0 pb-1 pt-2 text-xs text-base-content/70"
            @click="collapseDetailsFromContentClick"
          >
            <ul class="timeline timeline-vertical timeline-compact">
              <li
                v-for="(toolCall, idx) in toolCallsForBlock(block)"
                :key="`${block.id}-tool-${idx}`"
              >
                <div class="timeline-start max-w-36 pr-2 text-xs font-semibold opacity-75">
                  {{ toolCallTitle(toolCall, idx + 1) }}
                </div>
                <div class="timeline-middle">
                  <span
                    class="inline-block h-2.5 w-2.5 rounded-full"
                    :class="toolTimelineDotClass(block, toolCall)"
                  ></span>
                </div>
                <div class="timeline-end mb-2 w-full min-w-0 pb-2 pl-3">
                  <pre
                    v-if="toolCallSummaryText(toolCall)"
                    class="m-0 whitespace-pre-wrap break-all text-xs leading-relaxed text-base-content/70"
                  >{{ toolCallSummaryText(toolCall) }}</pre>
                </div>
                <hr
                  v-if="idx < toolCallsForBlock(block).length - 1"
                  :class="toolTimelineHrClass(block, toolCall)"
                />
              </li>
            </ul>
          </div>
        </details>
      </div>
      <div v-if="hasRenderableMemeSegments(block)" :class="block.taskTrigger ? 'mt-3' : ''">
        <div ref="markdownContainerRef" class="ecall-meme-segment-flow">
          <template v-for="(segment, index) in block.memeSegments || []" :key="`${block.id}-meme-${index}`">
            <div
              v-if="segment.type === 'text' && segment.text"
              class="ecall-meme-text-segment"
            >
              <MarkdownRender
                class="ecall-markdown-content ecall-inline-meme-markdown max-w-none"
                custom-id="chat-markstream"
                :nodes="markdownNodesForText(block, segment.text, true, `meme:${index}`)"
                :is-dark="markdownIsDark"
                :final="true"
                :max-live-nodes="0"
                :batch-rendering="false"
                :initial-render-batch-size="0"
                :render-batch-size="0"
                :render-batch-delay="0"
                :render-batch-budget-ms="0"
                :code-block-props="markdownCodeBlockProps"
                :mermaid-props="markdownMermaidProps"
                :typewriter="false"
                @click="emit('assistantLinkClick', $event)"
              />
            </div>
            <img
              v-else-if="segment.type === 'meme'"
              :src="memeSegmentDataUrl(segment)"
              :alt="`:${segment.category}:`"
              class="ecall-inline-meme"
              loading="lazy"
              decoding="async"
            />
          </template>
        </div>
      </div>
      <div v-else-if="block.text" :class="block.taskTrigger ? 'mt-3' : ''">
        <div
          v-if="isOwnMessage(block)"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ block.text }}</div>
        <div ref="markdownContainerRef">
          <MarkdownRender
            class="ecall-markdown-content max-w-none"
            custom-id="chat-markstream"
            :nodes="markdownNodesForBlock(block)"
            :is-dark="markdownIsDark"
            :final="!block.isStreaming"
            :max-live-nodes="0"
            :batch-rendering="!!block.isStreaming"
            :initial-render-batch-size="block.isStreaming ? STREAM_INITIAL_RENDER_BATCH_SIZE : 0"
            :render-batch-size="block.isStreaming ? STREAM_RENDER_BATCH_SIZE : 0"
            :render-batch-delay="block.isStreaming ? STREAM_RENDER_BATCH_DELAY : 0"
            :render-batch-budget-ms="block.isStreaming ? STREAM_RENDER_BATCH_BUDGET_MS : 0"
            :code-block-props="markdownCodeBlockProps"
            :mermaid-props="markdownMermaidProps"
            :typewriter="true"
            @click="emit('assistantLinkClick', $event)"
          />
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
            @dblclick.stop="openResolvedImagePreview(img, idx)"
          />
          <div
            v-else-if="isImageMime(img.mime)"
            class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-[11px] text-base-content/55"
          >
            <span class="loading loading-spinner loading-xs mr-2"></span>
            <span>图片加载中</span>
          </div>
          <div v-else-if="isPdfMime(img.mime)" class="badge badge-ghost gap-1 py-3 w-fit">
            <FileText class="h-3.5 w-3.5" />
            <span class="text-[11px]">PDF</span>
          </div>
        </template>
      </div>
      <div v-if="block.audios.length > 0" :class="block.taskTrigger || block.text || block.images.length > 0 ? 'mt-2 flex flex-col gap-1' : 'flex flex-col gap-1'">
        <button
          v-for="(aud, idx) in block.audios"
          :key="`${block.id}-aud-${idx}`"
          class="btn btn-sm bg-base-100/70 w-fit"
          @click="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
        >
          <Pause v-if="playingAudioId === `${block.id}-aud-${idx}`" class="h-3 w-3" />
          <Play v-else class="h-3 w-3" />
          <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
        </button>
      </div>
      <div
        v-if="block.attachmentFiles.length > 0"
        :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap gap-1' : 'flex flex-wrap gap-1'"
      >
        <div
          v-for="(file, idx) in block.attachmentFiles"
          :key="`${block.id}-file-${idx}`"
          class="badge badge-ghost gap-1 py-3"
          :title="file.relativePath"
        >
          <FileText class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ file.fileName }}</span>
        </div>
            </div>
          </div>
          <div
            v-else
            class="chat-bubble self-start bg-base-100 text-base-content border border-base-300/70 assistant-markdown ecall-assistant-bubble ecall-assistant-loading-bubble max-w-full"
          >
            <span class="ecall-assistant-loading-dots" aria-hidden="true">
              <span></span>
              <span></span>
              <span></span>
            </span>
            <span class="sr-only">{{ streamingHeaderStatus || t("chat.statusWaitingReply") }}</span>
          </div>
          <div
            :class="[
              'chat-footer ecall-message-footer flex h-6 items-center gap-1.5 transition-opacity',
              selectionModeEnabled
                ? 'opacity-100 pointer-events-auto'
                : showRegenerateAction && canRegenerate
                  ? 'opacity-100 pointer-events-auto'
                  : !block.isStreaming
                    ? 'opacity-0 pointer-events-none group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto'
                    : 'opacity-0 pointer-events-none',
            ]"
          >
            <button
              v-if="!selectionModeEnabled"
              type="button"
              class="ecall-message-footer-action inline-flex h-6 min-w-6 items-center justify-center rounded px-1 text-[11px] text-base-content/55 hover:text-base-content"
              :title="'多选'"
              :class="!block.isStreaming ? '' : 'opacity-0 pointer-events-none'"
              :disabled="block.isStreaming"
              @click="emit('enterSelectionMode', selectionKey)"
            >
              <CircleCheckBig class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.copy')"
              :class="!selectionModeEnabled && !block.isStreaming ? '' : 'opacity-0 pointer-events-none'"
              :disabled="selectionModeEnabled || block.isStreaming"
              @click="emit('copyMessage', block)"
            >
              <Copy class="h-3.5 w-3.5" />
            </button>
            <button
              v-if="hideToggleEnabled && !selectionModeEnabled"
              type="button"
              class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="bubbleBackgroundHidden ? '显示气泡背景' : '隐藏气泡背景'"
              :disabled="block.isStreaming"
              @click="emit('toggleBubbleBackground', selectionKey)"
            >
              <EyeOff v-if="bubbleBackgroundHidden" class="h-3.5 w-3.5" />
              <Eye v-else class="h-3.5 w-3.5" />
            </button>
            <button
              v-if="showRegenerateAction"
              type="button"
              class="ecall-message-footer-action inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.regenerate')"
              :class="!selectionModeEnabled && !block.isStreaming && showRegenerateAction && canRegenerate ? '' : 'opacity-0 pointer-events-none'"
              :disabled="selectionModeEnabled || block.isStreaming || !showRegenerateAction || !canRegenerate || busy"
              @click="emit('regenerateTurn', { turnId: block.sourceMessageId || block.id })"
            >
              <RotateCcw class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>
    </template>
    <template v-else>
      <div v-if="!compactWithPrevious" class="chat-header mb-1 flex items-baseline gap-2">
        <span v-if="isOwnMessage(block)" class="text-xs leading-none">
          <time v-if="formattedCreatedAt && !block.isStreaming" class="text-base-content/40 leading-none">{{ formattedCreatedAt }}</time>
        </span>
        <span class="text-xs text-base-content opacity-80">{{ displayName }}</span>
        <span v-if="!isOwnMessage(block)" class="text-xs leading-none">
          <time v-if="formattedCreatedAt && !block.isStreaming" class="text-base-content/40 leading-none">{{ formattedCreatedAt }}</time>
        </span>
      </div>
      <div :class="[
        'chat-bubble',
        bubbleBackgroundHidden ? 'ecall-message-bubble-bg-hidden text-base-content' : '',
        isOwnMessage(block)
          ? ''
          : [
            'self-start text-base-content assistant-markdown ecall-assistant-bubble max-w-full',
            bubbleBackgroundHidden ? '' : 'bg-base-100 border border-base-300/70',
            blockNeedsWideBubble(block) ? 'ecall-assistant-bubble-wide' : '',
          ],
      ]">
        <div
          v-if="isOwnMessage(block) ? !!ownMessageDisplayText(block).trim() : !!block.text"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ isOwnMessage(block) ? ownMessageDisplayText(block) : block.text }}</div>
        <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
          <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
            <img
              v-if="isImageMime(img.mime) && resolvedImageSrc(img, idx)"
              :src="resolvedImageSrc(img, idx)"
              loading="lazy"
              decoding="async"
              class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
              @dblclick.stop="openResolvedImagePreview(img, idx)"
            />
            <div
              v-else-if="isImageMime(img.mime)"
              class="flex h-28 w-28 items-center justify-center rounded bg-base-200/70 text-[11px] text-base-content/55"
            >
              <span class="loading loading-spinner loading-xs mr-2"></span>
              <span>图片加载中</span>
            </div>
            <div v-else-if="isPdfMime(img.mime)" class="badge badge-ghost gap-1 py-3 w-fit">
              <FileText class="h-3.5 w-3.5" />
              <span class="text-[11px]">PDF</span>
            </div>
          </template>
        </div>
        <div v-if="block.audios.length > 0" :class="block.taskTrigger || block.text || block.images.length > 0 ? 'mt-2 flex flex-col gap-1' : 'flex flex-col gap-1'">
          <button
            v-for="(aud, idx) in block.audios"
            :key="`${block.id}-aud-${idx}`"
            class="btn btn-sm bg-base-100/70 w-fit"
            @click="emit('toggleAudioPlayback', { id: `${block.id}-aud-${idx}`, audio: aud })"
          >
            <Pause v-if="playingAudioId === `${block.id}-aud-${idx}`" class="h-3 w-3" />
            <Play v-else class="h-3 w-3" />
            <span>{{ t("chat.voice", { index: idx + 1 }) }}</span>
          </button>
        </div>
        <div
          v-if="block.attachmentFiles.length > 0"
          :class="block.taskTrigger || block.text || block.images.length > 0 || block.audios.length > 0 ? 'mt-2 flex flex-wrap gap-1' : 'flex flex-wrap gap-1'"
        >
          <div
            v-for="(file, idx) in block.attachmentFiles"
            :key="`${block.id}-file-${idx}`"
            class="badge badge-ghost gap-1 py-3"
            :title="file.relativePath"
          >
            <FileText class="h-3.5 w-3.5" />
            <span class="text-[11px]">{{ file.fileName }}</span>
          </div>
        </div>
      </div>
      <div
        v-if="isOwnMessage(block) && !compactWithPrevious"
        :class="[
          'flex justify-end transition-opacity',
          selectionModeEnabled
            ? 'opacity-0 pointer-events-none'
            : 'opacity-0 pointer-events-none group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto',
        ]"
      >
        <button
          v-if="hideToggleEnabled"
          type="button"
          class="ecall-message-recall-action inline-flex h-5 w-5 items-center justify-center rounded text-base-content/40 hover:text-base-content"
          :title="bubbleBackgroundHidden ? '显示气泡背景' : '隐藏气泡背景'"
          :disabled="selectionModeEnabled || block.isStreaming"
          @click="emit('toggleBubbleBackground', selectionKey)"
        >
          <EyeOff v-if="bubbleBackgroundHidden" class="h-3 w-3" />
          <Eye v-else class="h-3 w-3" />
        </button>
        <button
          type="button"
          class="ecall-message-recall-action inline-flex h-5 w-5 items-center justify-center rounded text-base-content/40 hover:text-base-content"
          :title="t('chat.recall')"
          :disabled="selectionModeEnabled || busy"
          @click="emit('recallTurn', { turnId: block.sourceMessageId || block.id })"
        >
          <Undo2 class="h-3 w-3" />
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watchEffect, watchPostEffect } from "vue";
import { useI18n } from "vue-i18n";
import { CircleCheckBig, Copy, Eye, EyeOff, FileText, Pause, Play, RotateCcw, Undo2 } from "lucide-vue-next";
import MarkdownRender, { enableKatex, enableMermaid, getMarkdown, parseMarkdownToStructure } from "markstream-vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessageBlock, MemeMessageSegment } from "../../../types/app";
import { formatIsoToLocalHourMinute } from "../../../utils/time";
import { registerChatMarkstreamComponents } from "../markdown/register-chat-markstream";
import { normalizeLocalLinkHref } from "../utils/local-link";

enableMermaid();
enableKatex();
registerChatMarkstreamComponents();

const STREAM_MARKDOWN_PARSE_THROTTLE_MS = 100;
const MARKDOWN_NODE_CACHE_LIMIT = 100;
const STREAM_INITIAL_RENDER_BATCH_SIZE = 20;
const STREAM_RENDER_BATCH_SIZE = 10;
const STREAM_RENDER_BATCH_DELAY = 24;
const STREAM_RENDER_BATCH_BUDGET_MS = 4;
const markstreamMarkdown = getMarkdown();
const markdownNodeCache = new Map<string, { text: string; final: boolean; nodes: any[]; lastParseTime: number }>();
const markdownCodeBlockProps = {
  showHeader: true,
  showCopyButton: true,
  showPreviewButton: false,
  showExpandButton: true,
  showCollapseButton: true,
  showFontSizeButtons: false,
  enableFontSizeControl: false,
  isShowPreview: false,
  showTooltips: false,
};
const markdownMermaidProps = {
  showHeader: true,
  showCopyButton: true,
  showExportButton: false,
  showFullscreenButton: true,
  showCollapseButton: false,
  showZoomControls: true,
  showModeToggle: false,
  enableWheelZoom: true,
  showTooltips: false,
};
const imageDataUrlCache = new Map<string, string>();
const imageDataUrlPromiseCache = new Map<string, Promise<string>>();

const props = defineProps<{
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
  streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
  markdownIsDark: boolean;
  playingAudioId: string;
  activeTurnUser: boolean;
  compactWithPrevious: boolean;
  canRegenerate: boolean;
  canConfirmPlan: boolean;
  bubbleBackgroundHidden: boolean;
  hideToggleEnabled: boolean;
}>();

const emit = defineEmits<{
  (e: "enterSelectionMode", selectionKey: string): void;
  (e: "toggleMessageSelected", selectionKey: string): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "confirmPlan", payload: { messageId: string }): void;
  (e: "copyMessage", block: ChatMessageBlock): void;
  (e: "openImagePreview", image: { mime: string; bytesBase64?: string; dataUrl?: string }): void;
  (e: "toggleAudioPlayback", payload: { id: string; audio: { mime: string; bytesBase64: string } }): void;
  (e: "assistantLinkClick", event: MouseEvent): void;
  (e: "toggleBubbleBackground", selectionKey: string): void;
}>();

const showRegenerateAction = false;

const { t } = useI18n();
const resolvedImageSrcMap = ref<Record<string, string>>({});
const markdownContainerRef = ref<HTMLElement | null>(null);
const reasoningStandardExpanded = ref(false);
const inlineReasoningExpanded = ref(false);
const toolCallsExpanded = ref(false);
let disposed = false;

const displayName = computed(() => messageName(props.block));
const avatarUrl = computed(() => messageAvatarUrl(props.block));
const formattedCreatedAt = computed(() => formattedBlockTime(props.block.createdAt));
const streamingHeaderStatus = computed(() => assistantStreamingHeaderStatus(props.block));

function detailsOpenFromEvent(event: Event): boolean {
  const target = event.target;
  return target instanceof HTMLDetailsElement ? target.open : false;
}

function onReasoningStandardToggle(event: Event): void {
  reasoningStandardExpanded.value = detailsOpenFromEvent(event);
}

function onInlineReasoningToggle(event: Event): void {
  inlineReasoningExpanded.value = detailsOpenFromEvent(event);
}

function onToolCallsToggle(event: Event): void {
  toolCallsExpanded.value = detailsOpenFromEvent(event);
}

function collapseDetailsFromContentClick(event: MouseEvent): void {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  if (target.closest('button, a, input, textarea, select, summary, label, [data-selection-ignore="true"]')) {
    return;
  }
  if (window.getSelection()?.toString()) {
    return;
  }
  const details = target.closest("details");
  if (details instanceof HTMLDetailsElement) {
    details.open = false;
  }
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
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
  if (block.remoteImOrigin) return false;
  const id = String(block.speakerAgentId || "").trim();
  return !id || id === "user-persona";
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

function hasRenderableMemeSegments(block: ChatMessageBlock): boolean {
  return !isOwnMessage(block) && Array.isArray(block.memeSegments) && block.memeSegments.length > 0;
}

function memeSegmentDataUrl(segment: MemeMessageSegment): string {
  if (segment.type !== "meme") return "";
  return `data:${segment.mime};base64,${segment.bytesBase64}`;
}

function showStreamingUi(block: ChatMessageBlock): boolean {
  return !!block.isStreaming && !isOwnMessage(block);
}

function assistantStreamingHeaderStatus(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return "";
  const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
  const preStreamingStatusText = String(providerMeta._preStreamingStatusText || "").trim();
  if (preStreamingStatusText) return preStreamingStatusText;
  const toolCalls = toolCallsForBlock(block);
  const doingTool = toolCalls.find((call) => call.status === "doing") || toolCalls[toolCalls.length - 1];
  if (doingTool?.name) return `正在执行 ${doingTool.name} 中`;
  if (hasStreamingSpeechContent(block)) {
    return t("chat.statusSpeaking");
  }
  if (String(block.reasoningStandard || resolvedInlineReasoning(block) || "").trim()) {
    return t("chat.statusThinking");
  }
  return t("chat.statusWaitingReply");
}

function showAssistantPreStreamingDots(block: ChatMessageBlock): boolean {
  if (!showStreamingUi(block)) return false;
  const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
  const preStreamingStatusText = String(providerMeta._preStreamingStatusText || "").trim();
  if (!preStreamingStatusText) return false;
  return !hasStreamingSpeechContent(block)
    && !String(block.reasoningStandard || "").trim()
    && !String(resolvedInlineReasoning(block) || "").trim()
    && toolCallsForBlock(block).length === 0
    && block.images.length === 0
    && block.audios.length === 0
    && block.attachmentFiles.length === 0;
}

function hasStreamingSpeechContent(block: ChatMessageBlock): boolean {
  if (String(block.text || "").trim()) return true;
  if (Array.isArray(block.streamSegments) && block.streamSegments.some((item) => String(item || "").trim())) return true;
  if (String(block.streamTail || "").trim()) return true;
  if (String(block.streamAnimatedDelta || "").trim()) return true;
  return false;
}

function shouldAnimateEnter(block: ChatMessageBlock): boolean {
  if (!isOwnMessage(block)) return false;
  const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
  return !!providerMeta._optimistic;
}

function toolCallsForBlock(block: ChatMessageBlock): Array<{ name: string; argsText: string; status?: "doing" | "done" }> {
  return block.toolCalls;
}

function toolStatusLabel(block: ChatMessageBlock): string {
  if (!showStreamingUi(block)) return "工具执行毕";
  return toolSummaryDoing(block) ? "工具执行中" : "工具执行毕";
}

function toolSummaryDoing(block: ChatMessageBlock): boolean {
  if (!showStreamingUi(block)) return false;
  return toolCallsForBlock(block).some((call) => String(call.status || "").trim() === "doing");
}

const internalToolNames = new Set<string>([
  "apply_patch",
  "exec",
  "shell_exec",
  "read_file",
  "write_file",
  "append_text",
  "delete_file",
  "create_file",
  "rename_file",
  "move_file",
  "list_dir",
  "read_dir",
  "find",
  "search",
  "todo",
  "plan",
  "task",
  "delegate",
  "remember",
  "recall",
  "fetch",
  "websearch",
  "operate",
  "screenshot",
  "wait",
  "akasha_search",
  "akasha_read",
  "akasha_catalog",
  "akasha_link",
  "tavily_search",
  "tavily_extract",
  "tavily_crawl",
  "tavily_map",
  "tavily_research",
]);

const compactListKeys = new Set<string>([
  "todos",
  "files",
  "urls",
  "queries",
  "lineRanges",
  "tags",
]);

const ignorableSummaryKeys = new Set<string>([
  "status",
  "reasoning",
  "background",
  "why",
  "max_length",
  "maxResults",
  "max_results",
  "tokens",
  "timeout_ms",
  "quality",
  "exact_match",
  "include_raw_content",
  "include_images",
  "include_image_descriptions",
  "include_favicon",
  "format",
  "topic",
  "country",
  "search_depth",
  "extract_depth",
]);

function normalizeToolCallArgs(argsText: string): unknown {
  const text = String(argsText || "").trim();
  if (!text) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function safeTextFromRecord(data: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = data[key];
    if (typeof value === "string") {
      const trimmed = value.trim();
      if (trimmed) return trimmed;
    }
    if (Array.isArray(value)) {
      const joined = value
        .map((item) => (typeof item === "string" ? item.trim() : ""))
        .filter(Boolean)
        .join(" ");
      if (joined) return joined;
    }
  }
  return "";
}

function compactText(text: string, maxLen = 180): string {
  const trimmed = text.replace(/\s+/g, " ").trim();
  if (trimmed.length <= maxLen) return trimmed;
  return `${trimmed.slice(0, maxLen - 3)}...`;
}

function joinNonEmpty(parts: string[], separator = " · "): string {
  return parts.map((part) => part.trim()).filter(Boolean).join(separator);
}

function safeStringValue(data: Record<string, unknown>, key: string): string {
  const value = data[key];
  return typeof value === "string" ? value.trim() : "";
}

function toolTimelineText(key: string, params?: Record<string, string | number>): string {
  return String(t(`status.toolTimeline.${key}`, params ?? {}));
}

function toolTimelineNameValue(name: string, value: string): string {
  return `${name}：${value}`;
}

function taskTriggerSummary(value: unknown): string {
  if (typeof value !== "object" || value === null) return "";
  const obj = value as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "runAtLocal"),
    obj.everyMinutes !== undefined ? toolTimelineText("everyMinutes", { minutes: String(obj.everyMinutes) }) : "",
    safeStringValue(obj, "endAtLocal") ? toolTimelineText("until", { time: safeStringValue(obj, "endAtLocal") }) : "",
  ]);
}

function compactObjectEntries(data: Record<string, unknown>, maxItems = 3): string {
  return Object.entries(data)
    .filter(([key, value]) => !ignorableSummaryKeys.has(key) && value !== undefined && value !== null && value !== "")
    .map(([key, value]) => {
      if (compactListKeys.has(key) && Array.isArray(value)) {
        return `${value.length} ${key}`;
      }
      const text = toCompactValue(value, 1);
      return text ? `${key}: ${text}` : "";
    })
    .filter(Boolean)
    .slice(0, maxItems)
    .join(" · ");
}

function toSingleLineJsonText(payload: unknown): string {
  if (payload === undefined || payload === null) return "";
  if (typeof payload === "string") return payload.trim() || "";
  try {
    return JSON.stringify(payload);
  } catch {
    return String(payload);
  }
}

function compactSingleLineJson(payload: unknown, maxLen = 180): string {
  const text = toSingleLineJsonText(payload);
  if (!text) return "";
  const oneLine = text.replace(/\s+/g, " ").trim();
  if (oneLine.length <= maxLen) return oneLine;
  return `${oneLine.slice(0, maxLen - 3)}...`;
}

function toCompactValue(value: unknown, depth = 0): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (depth > 1) return "";

  if (Array.isArray(value)) {
    const parts = value
      .map((item) => toCompactValue(item, depth + 1))
      .filter((item) => item !== "")
      .slice(0, 3);
    return parts.join(" | ");
  }

  if (typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const orderedKeys = [
      "path",
      "file",
      "target",
      "source",
      "destination",
      "from",
      "to",
      "command",
      "cmd",
      "url",
      "query",
      "name",
      "id",
      "text",
      "content",
      "input",
      "output",
      "method",
    ];

    for (const key of orderedKeys) {
      const valueText = toCompactValue(obj[key], depth + 1);
      if (valueText) return `${key}: ${valueText}`;
    }

    const pairs = Object.entries(obj)
      .map(([key, rawValue]) => {
        const compactValue = toCompactValue(rawValue, depth + 1);
        return compactValue ? `${key}: ${compactValue}` : "";
      })
      .filter(Boolean)
      .slice(0, 2);
    if (pairs.length > 0) {
      return pairs.join("；");
    }
  }

  return "";
}

function applyPatchOperationLabel(operation: string): string {
  if (operation === "add") return toolTimelineText("patchAdd");
  if (operation === "delete") return toolTimelineText("patchDelete");
  if (operation === "move") return toolTimelineText("patchMove");
  return toolTimelineText("patchUpdate");
}

function summarizeApplyPatchInput(input: string): string {
  const lines = input.split(/\r?\n/);
  const entries: Array<{ operation: string; path: string }> = [];
  let pendingUpdatePath = "";

  for (const line of lines) {
    const addMatch = line.match(/^\*\*\* Add File:\s+(.+)$/);
    if (addMatch?.[1]) {
      entries.push({ operation: "add", path: addMatch[1].trim() });
      pendingUpdatePath = "";
      continue;
    }

    const deleteMatch = line.match(/^\*\*\* Delete File:\s+(.+)$/);
    if (deleteMatch?.[1]) {
      entries.push({ operation: "delete", path: deleteMatch[1].trim() });
      pendingUpdatePath = "";
      continue;
    }

    const updateMatch = line.match(/^\*\*\* Update File:\s+(.+)$/);
    if (updateMatch?.[1]) {
      pendingUpdatePath = updateMatch[1].trim();
      entries.push({ operation: "update", path: pendingUpdatePath });
      continue;
    }

    const moveMatch = line.match(/^\*\*\* Move to:\s+(.+)$/);
    if (moveMatch?.[1] && pendingUpdatePath) {
      const last = entries[entries.length - 1];
      if (last && last.path === pendingUpdatePath) {
        last.operation = "move";
        last.path = `${pendingUpdatePath} → ${moveMatch[1].trim()}`;
      }
      pendingUpdatePath = "";
    }
  }

  if (entries.length === 0) return toolTimelineText("inlinePatch");
  return entries
    .slice(0, 5)
    .map((entry) => `${applyPatchOperationLabel(entry.operation)} ${entry.path}`)
    .join("，");
}

function summarizeApplyPatchTool(args: unknown): string {
  const argsText = toSingleLineJsonText(args);
  if (!argsText) return toolTimelineText("checkChanges");

  if (typeof args === "string") {
    if (!args.trim()) return toolTimelineText("checkChanges");
    return summarizeApplyPatchInput(args);
  }

  if (typeof args === "object" && args !== null) {
    const obj = args as Record<string, unknown>;
    const input = typeof obj.input === "string" ? obj.input.trim() : "";
    if (input) return summarizeApplyPatchInput(input);

    const patch = (typeof obj.patch === "string" ? obj.patch : typeof obj.diff === "string" ? obj.diff : "").trim();

    const fileFromArgs = safeTextFromRecord(obj, ["file", "target", "path", "files", "pathnames"]);
    if (fileFromArgs) {
      return `${toolTimelineText("patchUpdate")} ${fileFromArgs}`;
    }

    if (patch) {
      const files = Array.from(new Set(
        patch
          .split(/\r?\n/)
          .map((line) => {
            const match = line.match(/^diff --git\s+(?:a\/|\S+)\s+(?:b\/|\S+)(.+)$/);
            if (match && match[1]) {
              return String(match[1]).replace(/^b\//, "").trim();
            }
            const simpleMatch = line.match(/^---\s+([ab]\/)?(.+)$/);
            if (simpleMatch && simpleMatch[2]) {
              return String(simpleMatch[2]).trim();
            }
            return "";
          })
          .filter((file) => Boolean(file) && !file.includes("/dev/null")),
      ));

      const filtered = files.filter((file) => file.length > 0);
      if (filtered.length > 0) {
        return filtered.map((file) => `${toolTimelineText("patchUpdate")} ${file}`).join("，");
      }
    }

    return compactSingleLineJson(args, 180) || toolTimelineText("checkArgs");
  }

  return toolTimelineText("patchCall");
}

function summarizeCommandTool(args: unknown): string {
  if (!args) return toolTimelineText("notProvided");
  if (typeof args === "string") return args;
  if (typeof args !== "object") return String(args);

  const obj = args as Record<string, unknown>;
  const command = safeTextFromRecord(obj, ["command", "cmd", "shell", "input", "commandText"]);
  const fallback = safeTextFromRecord(obj, ["args", "arguments", "argv", "params"]);
  if (command) return command;
  if (fallback) return fallback;
  const compact = toCompactValue(obj);
  return compact || toolTimelineText("checkArgs");
}

function summarizeFileTool(args: unknown): string {
  if (!args) return toolTimelineText("missingArgs");
  if (typeof args === "string") {
    const text = args.trim();
    return text || toolTimelineText("missingArgs");
  }
  if (typeof args !== "object") {
    return String(args);
  }
  const obj = args as Record<string, unknown>;
  const path = safeTextFromRecord(obj, ["path", "file", "target", "source", "destination", "from", "to"]);
  return path || toCompactValue(obj) || toolTimelineText("missingArgs");
}

function summarizeTodoTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const todos = (args as Record<string, unknown>).todos;
  if (!Array.isArray(todos)) return toolTimelineText("missingArgs");
  const counts = todos.reduce((acc, item) => {
    const status = typeof item === "object" && item !== null ? String((item as Record<string, unknown>).status || "pending") : "pending";
    acc[status] = (acc[status] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);
  const active = todos
    .map((item) => (typeof item === "object" && item !== null ? item as Record<string, unknown> : null))
    .find((item) => String(item?.status || "") === "in_progress")
    ?? (typeof todos[0] === "object" && todos[0] !== null ? todos[0] as Record<string, unknown> : null);
  const activeText = active ? compactText(String(active.content || ""), 120) : "";
  return joinNonEmpty([
    toolTimelineText("todoItems", { count: todos.length }),
    counts.in_progress ? toolTimelineText("todoInProgress", { count: counts.in_progress }) : "",
    counts.pending ? toolTimelineText("todoPending", { count: counts.pending }) : "",
    counts.completed ? toolTimelineText("todoCompleted", { count: counts.completed }) : "",
    activeText,
  ]);
}

function summarizeTaskTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "action"),
    safeStringValue(obj, "goal"),
    taskTriggerSummary(obj.trigger),
  ]) || compactObjectEntries(obj);
}

function summarizePlanTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "action"),
    compactText(safeStringValue(obj, "context"), 160),
  ]) || compactObjectEntries(obj);
}

function summarizeDelegateTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "task_name"),
    safeStringValue(obj, "specific_goal"),
    safeStringValue(obj, "department_id"),
    safeStringValue(obj, "mode"),
  ]) || compactObjectEntries(obj);
}

function summarizeMemoryTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "memory_type"),
    safeStringValue(obj, "judgment"),
    safeStringValue(obj, "query"),
  ]) || compactObjectEntries(obj);
}

function summarizeWebTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "query"),
    safeStringValue(obj, "url"),
    Array.isArray(obj.urls) ? `${obj.urls.length} URLs` : "",
    safeStringValue(obj, "instructions"),
  ]) || compactObjectEntries(obj);
}

function summarizeAkashaTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const obj = args as Record<string, unknown>;
  return joinNonEmpty([
    safeStringValue(obj, "world"),
    safeStringValue(obj, "keyword"),
    safeStringValue(obj, "documentPath"),
    safeStringValue(obj, "documentTitle"),
    Array.isArray(obj.lineRanges) ? obj.lineRanges.join("，") : "",
  ]) || compactObjectEntries(obj);
}

function summarizeOperateTool(args: unknown): string {
  if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
  const script = safeStringValue(args as Record<string, unknown>, "script");
  if (!script) return compactObjectEntries(args as Record<string, unknown>);
  const lines = script.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  return compactText(lines.slice(0, 3).join("；"), 180);
}

function summarizeBuiltinTool(toolName: string, args: unknown): string {
  if (toolName === "todo") return summarizeTodoTool(args);
  if (toolName === "task") return summarizeTaskTool(args);
  if (toolName === "plan") return summarizePlanTool(args);
  if (toolName === "delegate") return summarizeDelegateTool(args);
  if (toolName === "remember" || toolName === "recall") return summarizeMemoryTool(args);
  if (toolName === "fetch" || toolName === "websearch" || toolName.startsWith("tavily_")) return summarizeWebTool(args);
  if (toolName.startsWith("akasha_")) return summarizeAkashaTool(args);
  if (toolName === "operate") return summarizeOperateTool(args);
  if (toolName === "screenshot") return summarizeFileTool(args);
  if (toolName === "wait") return compactObjectEntries((typeof args === "object" && args !== null ? args : { ms: args }) as Record<string, unknown>);
  return "";
}

function summarizeExternalTool(name: string, args: unknown): string {
  if (args === undefined || args === null) return toolTimelineNameValue(name, toolTimelineText("noArgs"));
  if (typeof args === "string") {
    const text = args.trim();
    return toolTimelineNameValue(name, text || toolTimelineText("missingArgs"));
  }
  if (typeof args !== "object") {
    return toolTimelineNameValue(name, String(args));
  }

  const compact = toCompactValue(args);
  if (compact) {
    return toolTimelineNameValue(name, compact);
  }

  const jsonText = compactSingleLineJson(args, 180);
  if (jsonText) return toolTimelineNameValue(name, jsonText);

  return toolTimelineNameValue(name, toolTimelineText("missingArgs"));
}

function toolCallSummaryText(toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
  const toolName = String(toolCall.name || "").trim() || "unknown";
  const args = normalizeToolCallArgs(toolCall.argsText);

  if (internalToolNames.has(toolName)) {
    if (toolName === "apply_patch") return summarizeApplyPatchTool(args);
    if (toolName === "exec" || toolName === "shell_exec") return summarizeCommandTool(args);
    if (toolName.includes("file")) {
      return summarizeFileTool(args);
    }
    const builtinSummary = summarizeBuiltinTool(toolName, args);
    if (builtinSummary) return builtinSummary;
    const compact = toCompactValue(args);
    return compact || toolTimelineText("missingArgs");
  }

  return summarizeExternalTool(toolCallDisplayName(toolName), args);
}

function toolCallTitle(toolCall: { name: string }, index: number): string {
  return `#${index} ${toolCallDisplayName(toolCall.name)}`;
}

function toolCallDisplayName(toolName: string): string {
  if (toolName === "shell_exec") return "exec";
  if (toolName === "read_dir") return "read_dir";
  if (toolName === "list_dir") return "list_dir";
  return String(toolName || toolTimelineText("unknownTool")).trim() || toolTimelineText("unknownTool");
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
    const name = String(call.name || "").trim() || toolTimelineText("unknownTool");
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

function formattedBlockTime(value?: string): string {
  return formatIsoToLocalHourMinute(value, "");
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

function splitThinkText(raw: string): { visible: string; inline: string } {
  const input = raw || "";
  const openTag = "<think>";
  const closeTag = "</think>";
  const blocks: string[] = [];
  let visible = "";
  let cursor = 0;

  while (cursor < input.length) {
    const openIdx = input.indexOf(openTag, cursor);
    if (openIdx < 0) {
      visible += input.slice(cursor);
      break;
    }

    visible += input.slice(cursor, openIdx);
    const afterOpen = openIdx + openTag.length;
    const closeIdx = input.indexOf(closeTag, afterOpen);
    if (closeIdx < 0) {
      const tail = input.slice(afterOpen).trim();
      if (tail) blocks.push(tail);
      cursor = input.length;
      break;
    }

    const inner = input.slice(afterOpen, closeIdx).trim();
    if (inner) blocks.push(inner);
    cursor = closeIdx + closeTag.length;
  }

  return {
    visible: visible.trim(),
    inline: blocks.join("\n\n"),
  };
}

function markdownNodesForText(
  block: ChatMessageBlock,
  rawText: string,
  final: boolean,
  cacheSuffix = "main",
): any[] {
  const text = splitThinkText(rawText).visible;
  const cacheKey = `${String(block.id || "")}::${cacheSuffix}`;
  const cached = markdownNodeCache.get(cacheKey);
  if (cached && cached.text === text && cached.final === final) {
    markdownNodeCache.delete(cacheKey);
    markdownNodeCache.set(cacheKey, cached);
    return cached.nodes;
  }

  const now = Date.now();
  if (!final && cached && now - cached.lastParseTime < STREAM_MARKDOWN_PARSE_THROTTLE_MS) {
    return cached.nodes;
  }

  const nodes = parseMarkdownToStructure(text, markstreamMarkdown, { final });
  if (markdownNodeCache.size >= MARKDOWN_NODE_CACHE_LIMIT) {
    const oldestKey = markdownNodeCache.keys().next().value;
    if (typeof oldestKey === "string") markdownNodeCache.delete(oldestKey);
  }
  markdownNodeCache.set(cacheKey, { text, final, nodes, lastParseTime: now });
  return nodes;
}

function markdownNodesForBlock(block: ChatMessageBlock): any[] {
  return markdownNodesForText(block, block.text, !block.isStreaming, "main");
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

function blockHasMermaid(block: ChatMessageBlock): boolean {
  const text = splitThinkText(block.text).visible;
  return /```(?:\s*)mermaid\b/i.test(text);
}

function blockHasCodeFence(block: ChatMessageBlock): boolean {
  const text = splitThinkText(block.text).visible;
  return /```[\w-]*\s*[\r\n]/i.test(text);
}

function blockNeedsWideBubble(block: ChatMessageBlock): boolean {
  return blockHasMermaid(block) || blockHasCodeFence(block);
}

function resolvedInlineReasoning(block: ChatMessageBlock): string {
  return splitThinkText(block.text).inline || block.reasoningInline || "";
}

function reasoningSummaryLabel(block: ChatMessageBlock): string {
  if (block.isStreaming) return "正在思考中";
  const elapsedMs = Number((block as ChatMessageBlock & { reasoningElapsedMs?: number }).reasoningElapsedMs ?? 0);
  if (!Number.isFinite(elapsedMs) || elapsedMs <= 0) return "思考完成";
  const elapsedSeconds = Math.max(1, Math.round(elapsedMs / 1000));
  return `思考了${elapsedSeconds}秒`;
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
  const task = invokeTauri<{ dataUrl: string }>("read_chat_image_data_url", {
    input: {
      mediaRef,
      mime,
    },
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

onBeforeUnmount(() => {
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
  });
}
</script>

<style scoped>
.ecall-chat-avatar-col {
  width: 1.75rem;
  min-width: 1.75rem;
}

.ecall-message-stack {
  min-height: 100%;
  flex: 1 1 auto;
  width: 100%;
}

.ecall-message-content {
  min-width: 0;
  flex: 0 1 auto;
}

.ecall-message-content-wide {
  width: 100%;
  max-width: none;
}

.ecall-message-simple {
  --ecall-chat-avatar-offset: calc(1.75rem + 0.75rem);
}

.ecall-message-simple .ecall-message-content {
  width: calc(100% + var(--ecall-chat-avatar-offset));
  max-width: calc(100% + var(--ecall-chat-avatar-offset));
  margin-left: calc(var(--ecall-chat-avatar-offset) * -1);
}

.ecall-message-simple .chat-header {
  margin-left: var(--ecall-chat-avatar-offset);
}

.ecall-message-simple .chat-bubble {
  width: 100%;
  max-width: none;
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
  --ms-font-sans: var(
    --app-font-family,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    Roboto,
    "Helvetica Neue",
    Arial,
    sans-serif
  );
  --ms-text-body: 0.875rem;
  --ms-leading-body: 1.5;
  --ms-text-h1: 1.02rem;
  --ms-leading-h1: 1.5;
  --ms-text-h2: 0.98rem;
  --ms-leading-h2: 1.5;
  --ms-text-h3: 0.94rem;
  --ms-leading-h3: 1.5;
  --ms-text-h4: 0.9rem;
  --ms-text-h5: 0.875rem;
  --ms-text-h6: 0.875rem;
  --ms-flow-paragraph-y: 0.25rem;
  --ms-flow-list-y: 0.25rem;
  --ms-flow-list-item-y: 0.12rem;
  --ms-flow-list-indent: 1.05rem;
  --ms-flow-list-indent-mobile: 1.05rem;
  --ms-flow-blockquote-y: 0.25rem;
  --ms-flow-blockquote-indent: 0.68rem;
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.5;
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
  font-family: inherit;
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
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-from) {
  opacity: 0;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-active) {
  transition: opacity 1000ms cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity;
}

.assistant-markdown :deep(.markstream-vue .typewriter-enter-to) {
  opacity: 1;
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
  font-weight: var(--ecall-md-heading-weight);
  line-height: 1.5;
  letter-spacing: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,.heading-node.heading-1)) {
  font-size: 1.02rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h2,.heading-node.heading-2)) {
  font-size: 0.98rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h3,.heading-node.heading-3)) {
  font-size: 0.94rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h4,.heading-node.heading-4)) {
  font-size: 0.9rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node)) {
  padding-left: 1.05rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item)) {
  margin: 0.12rem 0;
  padding-left: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item) > :where(p,ul,ol,.paragraph-node,.list-node)) {
  margin-top: 0.16rem;
  margin-bottom: 0.16rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(a,.link-node)) {
  text-decoration: underline;
  text-underline-offset: 0.18em;
  text-decoration-color: color-mix(in srgb, var(--color-base-content) 28%, transparent);
}

.assistant-markdown :deep(.ecall-markdown-content :where(a,.link-node):hover) {
  text-decoration-color: color-mix(in srgb, var(--color-base-content) 50%, transparent);
}

.assistant-markdown :deep(.ecall-markdown-content :where(strong,.strong-node)) {
  font-weight: var(--ecall-md-strong-weight);
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  border-left: 3px solid color-mix(in srgb, var(--color-base-content) 16%, transparent);
  padding-left: 0.68rem;
  color: color-mix(in srgb, var(--color-base-content) 82%, transparent);
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote) .markdown-renderer),
.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node,li,.list-item) .markdown-renderer) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content :where(hr,.hr-node)) {
  border: 0;
  border-top: 1px solid color-mix(in srgb, var(--color-base-content) 14%, transparent);
  margin: 0.65rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(:not(pre) > code,.inline-code)) {
  border: 1px solid color-mix(in srgb, var(--color-base-content) 12%, transparent);
  background: var(--color-base-200);
  border-radius: 0.4rem;
  padding: 0.08rem 0.3rem;
  font-family: var(
    --ms-font-mono,
    ui-monospace,
    "SFMono-Regular",
    "SF Mono",
    Menlo,
    Monaco,
    Consolas,
    "Liberation Mono",
    "Courier New",
    monospace
  );
  font-size: 0.86em;
  font-weight: var(--ecall-md-code-weight);
}


.assistant-markdown :deep(.ecall-markdown-content :where(table,.table-node)) {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(th,.table-node th)) {
  border-bottom: 1px solid color-mix(in srgb, var(--color-base-content) 16%, transparent);
  padding: 0.36rem 0.5rem;
  text-align: left;
  font-weight: var(--ecall-md-table-heading-weight);
}

.assistant-markdown :deep(.ecall-markdown-content :where(td,.table-node td)) {
  border-bottom: 1px solid color-mix(in srgb, var(--color-base-content) 10%, transparent);
  padding: 0.34rem 0.5rem;
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
}

:deep(.chat-bubble) {
  min-width: 0;
  min-height: 0;
}

.ecall-assistant-bubble {
  min-width: 3rem;
  min-height: 2.25rem;
  font-size: 0.875rem;
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

.ecall-assistant-loading-bubble {
  min-width: 3.25rem;
  min-height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding-inline: 0.9rem;
}

.ecall-assistant-loading-dots {
  display: inline-flex;
  align-items: center;
  gap: 0.28rem;
}

.ecall-assistant-loading-dots span {
  width: 0.36rem;
  height: 0.36rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-base-content) 62%, transparent);
  animation: ecall-assistant-loading-dot 1.1s ease-in-out infinite;
}

.ecall-assistant-loading-dots span:nth-child(2) {
  animation-delay: 0.14s;
}

.ecall-assistant-loading-dots span:nth-child(3) {
  animation-delay: 0.28s;
}

@keyframes ecall-assistant-loading-dot {
  0%, 80%, 100% {
    opacity: 0.35;
    transform: translateY(0);
  }
  40% {
    opacity: 1;
    transform: translateY(-0.16rem);
  }
}
.ecall-message-bubble-bg-hidden {
  min-width: 0 !important;
  min-height: 0 !important;
  padding-inline: 0 !important;
  background-color: transparent !important;
  border-color: transparent !important;
  box-shadow: none !important;
}
</style>
