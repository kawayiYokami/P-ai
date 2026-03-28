<template>
  <div
    :data-message-id="String(block.id || '')"
    :data-message-role="isOwnMessage(block) ? 'user' : block.role"
    :data-latest-own-message="latestOwnMessage ? 'true' : undefined"
    :data-latest-message="latestMessage ? 'true' : undefined"
    :style="containerStyle"
    :class="[
      'chat group/user-turn mt-3',
      isOwnMessage(block) ? 'chat-end' : 'chat-start',
      latestMessage && !isOwnMessage(block) ? 'ecall-latest-message-container' : '',
    ]"
  >
    <div class="chat-image self-start ecall-chat-avatar-col">
      <div class="flex w-7 flex-col items-center gap-2">
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
      </div>
    </div>
    <template v-if="!isOwnMessage(block)">
      <div class="ecall-message-stack min-w-0 flex flex-col self-stretch">
        <div
          class="ecall-message-content min-w-0"
          :data-latest-assistant-content="latestMessage ? 'true' : undefined"
        >
          <div class="chat-header mb-1 flex items-center gap-2">
            <span class="text-xs text-base-content">{{ displayName }}</span>
            <span class="inline-flex h-4 items-center text-[10px] leading-none">
              <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
                <span class="loading loading-infinity loading-sm"></span>
              </span>
              <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
            </span>
          </div>
          <div :class="[
            'chat-bubble',
            'self-start bg-base-100 text-base-content border border-base-300/70 assistant-markdown ecall-assistant-bubble max-w-full',
            blockHasMermaid(block) ? 'ecall-assistant-bubble-wide' : '',
          ]">
            <div v-if="block.taskTrigger" class="space-y-2">
        <div class="flex items-center gap-2">
          <span class="badge badge-sm badge-outline">{{ t("chat.taskTrigger.badge") }}</span>
        </div>
        <div class="text-base font-medium leading-6">{{ block.taskTrigger.title }}</div>
        <div v-if="block.taskTrigger.cause" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.cause") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.cause }}</div>
        </div>
        <div v-if="block.taskTrigger.goal" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.goal") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.goal }}</div>
        </div>
        <div v-if="block.taskTrigger.flow" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.flow") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.flow }}</div>
        </div>
        <div v-if="block.taskTrigger.statusSummary" class="space-y-0.5">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.statusSummary") }}</div>
          <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.statusSummary }}</div>
        </div>
        <div v-if="block.taskTrigger.runAt || block.taskTrigger.endAt || block.taskTrigger.everyMinutes" class="grid gap-1 text-sm leading-6">
          <div v-if="block.taskTrigger.runAt">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.runAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.runAt) }}</span>
          </div>
          <div v-if="block.taskTrigger.endAt">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.endAt") }}</span>
            <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.endAt) }}</span>
          </div>
          <div v-if="block.taskTrigger.everyMinutes">
            <span class="text-[11px] opacity-55">{{ t("config.task.fields.everyMinutes") }}</span>
            <span class="ml-2">{{ block.taskTrigger.everyMinutes }}</span>
          </div>
        </div>
        <div v-if="(block.taskTrigger?.todos ?? []).length > 0" class="space-y-1">
          <div class="text-[11px] opacity-55">{{ t("config.task.fields.todos") }}</div>
          <ul class="space-y-1 text-sm leading-6">
            <li v-for="(todo, todoIdx) in block.taskTrigger?.todos ?? []" :key="`${block.id}-todo-${todoIdx}`" class="flex items-start gap-2">
              <span class="mt-2 inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-current opacity-60"></span>
              <span class="min-w-0 whitespace-pre-wrap">{{ todo }}</span>
            </li>
          </ul>
        </div>
      </div>
      <details
        v-if="!isOwnMessage(block) && block.reasoningStandard"
        class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none min-w-55"
      >
        <summary class="collapse-title py-0 px-0 min-h-0 text-xs flex items-center gap-1 text-base-content/80">
          <span
            :class="['block min-w-0 flex-1 truncate ecall-shimmer-text', reasoningSummaryClass(block)]"
            :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
          >
            {{ reasoningSummaryLabel(block) }}
          </span>
        </summary>
        <div class="collapse-content px-0 py-2 whitespace-pre-wrap text-[11px] leading-relaxed text-base-content/70">
          {{ block.reasoningStandard }}
        </div>
      </details>
      <details
        v-if="!isOwnMessage(block) && resolvedInlineReasoning(block)"
        class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none min-w-55"
      >
        <summary class="collapse-title py-0 px-0 min-h-0 text-[10px] flex items-center gap-1 text-base-content/60 cursor-pointer">
          <span
            :class="['block min-w-0 flex-1 truncate ecall-shimmer-text', reasoningSummaryClass(block)]"
            :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
          >
            {{ reasoningSummaryLabel(block) }}
          </span>
        </summary>
        <div class="collapse-content max-w-full px-0 py-2 whitespace-pre-wrap wrap-break-word text-[10px] leading-relaxed text-base-content/60" style="overflow-wrap: anywhere;">
          {{ resolvedInlineReasoning(block) }}
        </div>
      </details>
      <div v-if="toolCallsForBlock(block).length > 0" class="mb-2 flex flex-col gap-1 text-[11px] opacity-90">
        <details class="collapse bg-base-200 border-base-300 border">
          <summary class="collapse-title py-2 px-3 min-h-0 text-[11px] font-semibold flex items-center gap-1.5">
            <span class="inline-block h-2 w-2 rounded-full bg-success"></span>
            <span
              :class="['ecall-shimmer-text font-medium', toolSummaryClass(block)]"
              :data-shimmer-text="showStreamingUi(block) ? '工具执行中' : ''"
            >{{ toolStatusLabel(block) }}</span>
            <span v-if="toolNamesLabel(block)" class="truncate">{{ ` · ${toolNamesLabel(block)}` }}</span>
          </summary>
          <div class="collapse-content px-3 pb-2 pt-0 text-[10px] text-base-content/70">
            <div
              v-for="(toolCall, idx) in toolCallsForBlock(block)"
              :key="`${block.id}-tool-${idx}`"
              class="mb-2 last:mb-0"
            >
              <div class="mb-1 font-semibold opacity-80">#{{ idx + 1 }} {{ toolCall.name }}</div>
              <pre class="whitespace-pre-wrap break-all">{{ toolCall.argsText }}</pre>
            </div>
          </div>
        </details>
      </div>
      <div v-if="block.text" :class="block.taskTrigger ? 'mt-3' : ''">
        <div
          v-if="isOwnMessage(block)"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ block.text }}</div>
        <MarkdownRender
          v-else
          :class="[
            'ecall-markdown-content max-w-none',
            block.isStreaming ? 'ecall-stream-content' : 'ecall-stream-content-done',
          ]"
          custom-id="chat-markstream"
          :nodes="markdownNodesForBlock(block)"
          :is-dark="markdownIsDark"
          :final="!block.isStreaming"
          :max-live-nodes="0"
          :batch-rendering="true"
          :render-batch-size="16"
          :render-batch-delay="32"
          :code-block-props="markdownCodeBlockProps"
          :mermaid-props="markdownMermaidProps"
          :typewriter="false"
          @click="emit('assistantLinkClick', $event)"
        />
      </div>
      <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
        <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
          <img
            v-if="isImageMime(img.mime)"
            :src="`data:${img.mime};base64,${img.bytesBase64}`"
            loading="lazy"
            decoding="async"
            class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
            @dblclick.stop="emit('openImagePreview', img)"
          />
          <div v-else-if="isPdfMime(img.mime)" class="badge badge-outline gap-1 py-3 w-fit">
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
          class="badge badge-outline gap-1 py-3"
          :title="file.relativePath"
        >
          <FileText class="h-3.5 w-3.5" />
          <span class="text-[11px]">{{ file.fileName }}</span>
        </div>
            </div>
          </div>
          <div
            :class="[
              'chat-footer mt-1 flex h-6 items-center gap-1.5 transition-opacity',
              canRegenerate
                ? 'opacity-100 pointer-events-auto'
                : !chatting && !frozen
                  ? 'opacity-0 pointer-events-none group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto'
                  : 'opacity-0 pointer-events-none',
            ]"
          >
            <button
              type="button"
              class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.copy')"
              :class="!block.isStreaming && !chatting && !frozen ? '' : 'opacity-0 pointer-events-none'"
              :disabled="block.isStreaming || chatting || frozen"
              @click="emit('copyMessage', block)"
            >
              <Copy class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
              :title="t('chat.regenerate')"
              :class="!block.isStreaming && !chatting && !frozen && canRegenerate ? '' : 'opacity-0 pointer-events-none'"
              :disabled="block.isStreaming || chatting || frozen || !canRegenerate"
              @click="emit('regenerateTurn', { turnId: block.id })"
            >
              <RotateCcw class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
        <div
          v-if="latestMessage"
          class="ecall-message-spacer min-h-0 flex-1"
          aria-hidden="true"
        ></div>
      </div>
    </template>
    <template v-else>
      <div class="chat-header mb-1 flex items-center gap-2">
        <button
          v-if="isOwnMessage(block)"
          type="button"
          :class="[
            'inline-flex h-5 w-5 items-center justify-center rounded text-base-content/40 hover:text-base-content opacity-0 pointer-events-none transition-opacity',
            !chatting && !frozen ? 'group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto' : '',
          ]"
          :title="t('chat.recall')"
          :disabled="chatting || frozen"
          @click="emit('recallTurn', { turnId: block.id })"
        >
          <Undo2 class="h-3 w-3" />
        </button>
        <span v-if="isOwnMessage(block)" class="inline-flex h-4 items-center text-[10px] leading-none">
          <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
            <span class="loading loading-infinity loading-sm"></span>
          </span>
          <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
        </span>
        <span class="text-xs text-base-content">{{ displayName }}</span>
        <span v-if="!isOwnMessage(block)" class="inline-flex h-4 items-center text-[10px] leading-none">
          <span v-if="block.isStreaming" class="ecall-time-loading opacity-70">
            <span class="loading loading-infinity loading-sm"></span>
          </span>
          <time v-else-if="formattedCreatedAt" class="opacity-50 leading-none">{{ formattedCreatedAt }}</time>
        </span>
      </div>
      <div :class="[
        'chat-bubble',
        isOwnMessage(block)
          ? ''
          : [
            'self-start bg-base-100 text-base-content border border-base-300/70 assistant-markdown ecall-assistant-bubble max-w-full',
            blockHasMermaid(block) ? 'ecall-assistant-bubble-wide' : '',
          ],
      ]">
        <div v-if="block.taskTrigger" class="space-y-2">
          <div class="flex items-center gap-2">
            <span class="badge badge-sm badge-outline">{{ t("chat.taskTrigger.badge") }}</span>
          </div>
          <div class="text-base font-medium leading-6">{{ block.taskTrigger.title }}</div>
          <div v-if="block.taskTrigger.cause" class="space-y-0.5">
            <div class="text-[11px] opacity-55">{{ t("config.task.fields.cause") }}</div>
            <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.cause }}</div>
          </div>
          <div v-if="block.taskTrigger.goal" class="space-y-0.5">
            <div class="text-[11px] opacity-55">{{ t("config.task.fields.goal") }}</div>
            <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.goal }}</div>
          </div>
          <div v-if="block.taskTrigger.flow" class="space-y-0.5">
            <div class="text-[11px] opacity-55">{{ t("config.task.fields.flow") }}</div>
            <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.flow }}</div>
          </div>
          <div v-if="block.taskTrigger.statusSummary" class="space-y-0.5">
            <div class="text-[11px] opacity-55">{{ t("config.task.fields.statusSummary") }}</div>
            <div class="text-sm leading-6 whitespace-pre-wrap">{{ block.taskTrigger.statusSummary }}</div>
          </div>
          <div v-if="block.taskTrigger.runAt || block.taskTrigger.endAt || block.taskTrigger.everyMinutes" class="grid gap-1 text-sm leading-6">
            <div v-if="block.taskTrigger.runAt">
              <span class="text-[11px] opacity-55">{{ t("config.task.fields.runAt") }}</span>
              <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.runAt) }}</span>
            </div>
            <div v-if="block.taskTrigger.endAt">
              <span class="text-[11px] opacity-55">{{ t("config.task.fields.endAt") }}</span>
              <span class="ml-2">{{ formattedBlockTime(block.taskTrigger.endAt) }}</span>
            </div>
            <div v-if="block.taskTrigger.everyMinutes">
              <span class="text-[11px] opacity-55">{{ t("config.task.fields.everyMinutes") }}</span>
              <span class="ml-2">{{ block.taskTrigger.everyMinutes }}</span>
            </div>
          </div>
          <div v-if="(block.taskTrigger?.todos ?? []).length > 0" class="space-y-1">
            <div class="text-[11px] opacity-55">{{ t("config.task.fields.todos") }}</div>
            <ul class="space-y-1 text-sm leading-6">
              <li v-for="(todo, todoIdx) in block.taskTrigger?.todos ?? []" :key="`${block.id}-todo-${todoIdx}`" class="flex items-start gap-2">
                <span class="mt-2 inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-current opacity-60"></span>
                <span class="min-w-0 whitespace-pre-wrap">{{ todo }}</span>
              </li>
            </ul>
          </div>
        </div>
        <details
          v-if="!isOwnMessage(block) && block.reasoningStandard"
          class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none min-w-55"
        >
          <summary class="collapse-title py-0 px-0 min-h-0 text-xs flex items-center gap-1 text-base-content/80">
            <span
              :class="['block min-w-0 flex-1 truncate ecall-shimmer-text', reasoningSummaryClass(block)]"
              :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div class="collapse-content px-0 py-2 whitespace-pre-wrap text-[11px] leading-relaxed text-base-content/70">
            {{ block.reasoningStandard }}
          </div>
        </details>
        <details
          v-if="!isOwnMessage(block) && resolvedInlineReasoning(block)"
          class="collapse mb-2 border-l-2 border-base-content/20 pl-3 rounded-none min-w-55"
        >
          <summary class="collapse-title py-0 px-0 min-h-0 text-[10px] flex items-center gap-1 text-base-content/60 cursor-pointer">
            <span
              :class="['block min-w-0 flex-1 truncate ecall-shimmer-text', reasoningSummaryClass(block)]"
              :data-shimmer-text="block.isStreaming ? '正在思考中' : ''"
            >
              {{ reasoningSummaryLabel(block) }}
            </span>
          </summary>
          <div class="collapse-content max-w-full px-0 py-2 whitespace-pre-wrap wrap-break-word text-[10px] leading-relaxed text-base-content/60" style="overflow-wrap: anywhere;">
            {{ resolvedInlineReasoning(block) }}
          </div>
        </details>
        <div v-if="toolCallsForBlock(block).length > 0" class="mb-2 flex flex-col gap-1 text-[11px] opacity-90">
          <details class="collapse bg-base-200 border-base-300 border">
            <summary class="collapse-title py-2 px-3 min-h-0 text-[11px] font-semibold flex items-center gap-1.5">
              <span class="inline-block h-2 w-2 rounded-full bg-success"></span>
              <span
                :class="['ecall-shimmer-text font-medium', toolSummaryClass(block)]"
                :data-shimmer-text="showStreamingUi(block) ? '工具执行中' : ''"
              >{{ toolStatusLabel(block) }}</span>
              <span v-if="toolNamesLabel(block)" class="truncate">{{ ` · ${toolNamesLabel(block)}` }}</span>
            </summary>
            <div class="collapse-content px-3 pb-2 pt-0 text-[10px] text-base-content/70">
              <div
                v-for="(toolCall, idx) in toolCallsForBlock(block)"
                :key="`${block.id}-tool-${idx}`"
                class="mb-2 last:mb-0"
              >
                <div class="mb-1 font-semibold opacity-80">#{{ idx + 1 }} {{ toolCall.name }}</div>
                <pre class="whitespace-pre-wrap break-all">{{ toolCall.argsText }}</pre>
              </div>
            </div>
          </details>
        </div>
        <div v-if="block.text" :class="block.taskTrigger ? 'mt-3' : ''">
        <div
          v-if="isOwnMessage(block)"
          class="whitespace-pre-wrap break-all"
          style="overflow-wrap: anywhere;"
        >{{ block.text }}</div>
          <MarkdownRender
            v-else
            :class="[
              'ecall-markdown-content max-w-none',
              block.isStreaming ? 'ecall-stream-content' : 'ecall-stream-content-done',
            ]"
            custom-id="chat-markstream"
            :nodes="markdownNodesForBlock(block)"
            :is-dark="markdownIsDark"
            :final="!block.isStreaming"
            :max-live-nodes="0"
            :batch-rendering="true"
            :render-batch-size="16"
            :render-batch-delay="32"
            :code-block-props="markdownCodeBlockProps"
            :mermaid-props="markdownMermaidProps"
            :typewriter="false"
            @click="emit('assistantLinkClick', $event)"
          />
        </div>
        <div v-if="block.images.length > 0" :class="block.taskTrigger || block.text ? 'mt-2 grid gap-1' : 'grid gap-1'">
          <template v-for="(img, idx) in block.images" :key="`${block.id}-img-${idx}`">
            <img
              v-if="isImageMime(img.mime)"
              :src="`data:${img.mime};base64,${img.bytesBase64}`"
              loading="lazy"
              decoding="async"
              class="rounded max-h-28 object-contain bg-base-100/40 cursor-zoom-in"
              @dblclick.stop="emit('openImagePreview', img)"
            />
            <div v-else-if="isPdfMime(img.mime)" class="badge badge-outline gap-1 py-3 w-fit">
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
            class="badge badge-outline gap-1 py-3"
            :title="file.relativePath"
          >
            <FileText class="h-3.5 w-3.5" />
            <span class="text-[11px]">{{ file.fileName }}</span>
          </div>
        </div>
      </div>
      <div
        v-if="!isOwnMessage(block)"
        :class="[
          'chat-footer mt-1 flex h-6 items-center gap-1.5 transition-opacity',
          canRegenerate
            ? 'opacity-100 pointer-events-auto'
            : !chatting && !frozen
              ? 'opacity-0 pointer-events-none group-hover/user-turn:opacity-100 group-hover/user-turn:pointer-events-auto'
              : 'opacity-0 pointer-events-none',
        ]"
      >
        <button
          type="button"
          class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
          :title="t('chat.copy')"
          :class="!block.isStreaming && !chatting && !frozen ? '' : 'opacity-0 pointer-events-none'"
          :disabled="block.isStreaming || chatting || frozen"
          @click="emit('copyMessage', block)"
        >
          <Copy class="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          class="inline-flex h-6 w-6 items-center justify-center rounded text-base-content/55 hover:text-base-content"
          :title="t('chat.regenerate')"
          :class="!block.isStreaming && !chatting && !frozen && canRegenerate ? '' : 'opacity-0 pointer-events-none'"
          :disabled="block.isStreaming || chatting || frozen || !canRegenerate"
          @click="emit('regenerateTurn', { turnId: block.id })"
        >
          <RotateCcw class="h-3.5 w-3.5" />
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, FileText, Pause, Play, RotateCcw, Undo2 } from "lucide-vue-next";
import MarkdownRender, { enableKatex, enableMermaid, getMarkdown, parseMarkdownToStructure } from "markstream-vue";
import type { ChatMessageBlock } from "../../../types/app";

enableMermaid();
enableKatex();

const STREAM_MARKDOWN_PARSE_THROTTLE_MS = 100;
const MARKDOWN_NODE_CACHE_LIMIT = 100;
const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});
const markstreamMarkdown = getMarkdown();
const markdownNodeCache = new Map<string, { text: string; final: boolean; nodes: any[]; lastParseTime: number }>();
const markdownCodeBlockProps = {
  showHeader: true,
  showCopyButton: true,
  showPreviewButton: false,
  showExpandButton: false,
  showCollapseButton: false,
  showFontSizeButtons: false,
  enableFontSizeControl: false,
  isShowPreview: false,
};
const markdownMermaidProps = {
  showHeader: true,
  showCopyButton: true,
  showExportButton: false,
  showFullscreenButton: false,
  showCollapseButton: false,
  showZoomControls: false,
  showModeToggle: false,
  enableWheelZoom: false,
};

const props = defineProps<{
  block: ChatMessageBlock;
  chatting: boolean;
  frozen: boolean;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  streamToolCalls: Array<{ name: string; argsText: string }>;
  markdownIsDark: boolean;
  playingAudioId: string;
  latestOwnMessage: boolean;
  latestMessage: boolean;
  latestMessageMinHeight: number;
  canRegenerate: boolean;
}>();

const emit = defineEmits<{
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "copyMessage", block: ChatMessageBlock): void;
  (e: "openImagePreview", image: { mime: string; bytesBase64: string }): void;
  (e: "toggleAudioPlayback", payload: { id: string; audio: { mime: string; bytesBase64: string } }): void;
  (e: "assistantLinkClick", event: MouseEvent): void;
}>();

const { t } = useI18n();

const displayName = computed(() => messageName(props.block));
const avatarUrl = computed(() => messageAvatarUrl(props.block));
const formattedCreatedAt = computed(() => formattedBlockTime(props.block.createdAt));
const containerStyle = computed<Record<string, string> | undefined>(() => {
  if (!props.latestMessage || isOwnMessage(props.block) || props.latestMessageMinHeight <= 0) return undefined;
  return { "--latest-message-target-height": `${props.latestMessageMinHeight}px` };
});

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

function showStreamingUi(block: ChatMessageBlock): boolean {
  return !!block.isStreaming && !isOwnMessage(block);
}

function toolCallsForBlock(block: ChatMessageBlock): Array<{ name: string; argsText: string }> {
  if (showStreamingUi(block) && props.streamToolCalls.length > 0) {
    return props.streamToolCalls;
  }
  return block.toolCalls;
}

function toolStatusLabel(block: ChatMessageBlock): string {
  return showStreamingUi(block) ? "工具执行中" : "工具执行毕";
}

function toolNamesLabel(block: ChatMessageBlock): string {
  const calls = toolCallsForBlock(block);
  if (calls.length === 0) return "";
  const counts = new Map<string, number>();
  const order: string[] = [];
  for (const call of calls) {
    const name = String(call.name || "").trim() || "未知工具";
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
  const raw = String(value || "").trim();
  if (!raw) return "";
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return raw;
  const parts = timeFormatter.formatToParts(parsed);
  const pick = (type: string) => parts.find((part) => part.type === type)?.value || "00";
  return `${pick("hour")}:${pick("minute")}`;
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

function markdownNodesForBlock(block: ChatMessageBlock): any[] {
  const text = splitThinkText(block.text).visible;
  const final = !block.isStreaming;
  const cacheKey = String(block.id || "");
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

function blockHasMermaid(block: ChatMessageBlock): boolean {
  const text = splitThinkText(block.text).visible;
  return /```(?:\s*)mermaid\b/i.test(text);
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

function reasoningSummaryClass(block: ChatMessageBlock): string {
  return block.isStreaming ? "ecall-reasoning-shimmer" : "";
}

function toolSummaryClass(block: ChatMessageBlock): string {
  return showStreamingUi(block) ? "ecall-reasoning-shimmer" : "";
}

function isImageMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase().startsWith("image/");
}

function isPdfMime(mime: string): boolean {
  return (mime || "").trim().toLowerCase() === "application/pdf";
}
</script>

<style scoped>
.ecall-latest-message-container {
  min-height: var(--latest-message-target-height, 0px);
}

.ecall-chat-avatar-col {
  width: 1.75rem;
  min-width: 1.75rem;
}

.ecall-message-stack {
  min-height: 100%;
}

.ecall-message-content {
  min-width: 0;
  flex-shrink: 0;
}

.ecall-time-loading {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  transform: scale(0.82);
  transform-origin: right center;
}

.ecall-stream-content {
  opacity: 0;
  animation: ecall-stream-fade-in 1s ease-out forwards;
}

.ecall-stream-content-done {
  opacity: 1;
}

.ecall-shimmer-text {
  position: relative;
  display: inline-block;
  color: currentColor;
}

.ecall-reasoning-shimmer::after {
  content: attr(data-shimmer-text);
  position: absolute;
  inset: 0;
  pointer-events: none;
  color: transparent;
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    transparent 44%,
    rgb(255 255 255 / 0.92) 50%,
    transparent 56%,
    transparent 100%
  );
  background-size: 280px 100%;
  background-position: 280px 0;
  will-change: background-position;
  transform: translateZ(0);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  animation: ecall-reasoning-shimmer 2.5s linear infinite;
}

.assistant-markdown :deep(.ecall-markdown-content.ecall-stream-content) > * {
  animation: ecall-stream-fade-in 0.5s ease-out forwards;
}

@keyframes ecall-stream-fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes ecall-reasoning-shimmer {
  from {
    background-position: 280px 0;
  }
  to {
    background-position: -280px 0;
  }
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4,h5,h6,p,ul,ol,pre,blockquote,figure)) {
  margin: 0.24em 0;
}

.assistant-markdown :deep(.ecall-markdown-content hr) {
  border: 0;
  border-top: 1px solid hsl(var(--bc) / 0.22);
  margin: 0.8rem 0;
  opacity: 1;
}

.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: currentColor;
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: hsl(var(--bc) / 0.5);
  --tw-prose-hr: hsl(var(--bc) / 0.15);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: hsl(var(--bc) / 0.2);
  --tw-prose-captions: hsl(var(--bc) / 0.75);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: hsl(var(--b2));
  --tw-prose-th-borders: hsl(var(--bc) / 0.2);
  --tw-prose-td-borders: hsl(var(--bc) / 0.15);
}

.assistant-markdown :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
}

.assistant-markdown :deep(.ecall-markdown-content pre) {
  overflow-x: auto;
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
}

.ecall-assistant-bubble-wide {
  width: 100%;
  max-width: 100%;
}
</style>
