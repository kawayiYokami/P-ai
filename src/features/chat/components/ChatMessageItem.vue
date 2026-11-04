<template>
  <div
    ref="messageRootRef"
    :data-message-id="String(block.id || '')"
    :data-message-role="isOwnMessage(block) ? 'user' : block.role"
    :data-active-turn-user="activeTurnUser ? 'true' : undefined"
    :class="[
      'ecall-chat-message-row group/user-turn relative rounded-2xl px-4 transition-colors',
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
      :wide="assistantNeedsWideBubble"
      :content-empty="bubbleContentEmpty(block)"
      :show-activity="showActivitySummary(block)"
    >
      <!-- activity 插槽必须常驻。流式结束那一帧 showActivitySummary 会从 false 翻 true
           （纯正文回答没有思考与工具，走的是 `return !block.isStreaming` 分支），
           用 template v-if 会让气泡容器在那一帧增删子节点，Blink 从公共祖先往下重算
           整棵子树样式，markdown 全文节点全要走一遍——18K 实测单帧 55.9ms。
           显隐交给 ChatBubbleShell 的 showActivity，只切 display。 -->
      <template #activity>
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
                <span v-if="hasActivityReasoning(block)" class="shrink-0">
                  {{ t("chat.messageItem.thought") }} <AnimatedCountText v-if="!activityReasoningRunning()" :target="block.activityReasoningCharCount || 0" />
                </span>
                <span v-else-if="!activityToolCountsLabel(block)" class="shrink-0">
                  {{ t("chat.messageItem.notThought") }}
                </span>
                <span v-if="hasActivityReasoning(block) && activityToolCountsLabel(block)" class="inline-flex h-3 items-center text-base-content/40">·</span>
                <span
                  v-if="activityToolCountsLabel(block)"
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
      <template #activity-panel>
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
              class="px-0 pb-1 pt-2 text-xs text-base-content"
            >
              <div class="flex flex-col">
                <TransitionGroup name="ecall-activity-item" tag="ul" class="ecall-activity-timeline" :appear="false">
                  <li
                    v-for="(item, itemIndex) in presentedActivityItems"
                    :key="`${block.id}-activity-${activityItemKey(item)}`"
                    v-memo="activityItemMemo(item, itemIndex === presentedActivityItems.length - 1)"
                    :class="[
                      activityItemNodeClass(item),
                      item.kind === 'reasoning' ? 'ecall-activity-reasoning-item' : '',
                      'relative flex gap-1.5 min-w-0',
                    ]"
                  >
                    <!-- ========== 思维链节点：左列圆点与竖线 / 右列折叠头、正文、留白行 ========== -->
                    <template v-if="item.kind === 'reasoning'">
                      <!-- 左列：圆点与竖线。竖线是结构内的一列，撑满整个条目（含右列末尾的留白行），
                           相邻条目在 li 边界处相接；不再用绝对定位与负 bottom 跨空隙 -->
                      <div class="relative flex w-4 shrink-0 flex-col items-center">
                        <span
                          v-if="itemIndex !== 0"
                          class="absolute top-0 h-1 w-px bg-current opacity-30"
                        />
                        <div class="flex h-6 shrink-0 items-center justify-center">
                          <svg viewBox="0 0 24 24" class="h-4 w-4">
                            <circle cx="12" cy="12" r="10" fill="currentColor" />
                            <path d="M12 4Q13 11 20 12Q13 13 12 20Q11 13 4 12Q11 11 12 4Z" class="text-base-200" fill="currentColor" />
                          </svg>
                        </div>
                        <span
                          v-if="itemIndex !== presentedActivityItems.length - 1"
                          class="w-px flex-1 bg-current opacity-30"
                        />
                      </div>

                      <!-- 右列：折叠头、正文、留白行 -->
                      <div class="flex min-w-0 flex-1 flex-col">
                        <!-- 展开态吸顶检测哨兵（贴在 Header 正上方） -->
                        <div
                          v-if="activityItemExpanded(item) && activityItemCanExpand(item)"
                          :ref="(el) => bindStickySentinel(activityItemKey(item), el as HTMLElement | null)"
                          class="ecall-reasoning-sticky-sentinel pointer-events-none h-px w-full -mb-px"
                        />

                        <!-- 标题行：展开时才吸顶（sticky -top-3 z-10 抵消容器 py-3，紧贴视口最顶端），未展开时不吸顶随文档滚动 -->
                        <div
                          class="ecall-reasoning-sticky-header group flex min-h-6 items-center gap-1.5 bg-base-200 py-1"
                          :class="[
                            activityItemExpanded(item) ? 'sticky -top-3 z-10' : '',
                            props.selectionModeEnabled || !activityItemCanExpand(item) ? '' : 'cursor-pointer select-none',
                            isReasoningItemStuck(item) ? 'ecall-reasoning-stuck' : '',
                          ]"
                          @click="onReasoningHeaderClick(item, $event)"
                        >
                          <!-- 首行标题摘要（宽度按内容，箭头紧跟在右肩） -->
                          <span class="ecall-activity-item-summary min-w-0" :class="activityItemTitleClass(item)">
                            <InlineMarkdownText :text="activityItemTitle(item)" />
                          </span>

                          <!-- 折叠箭头：能展开就常显，不再等悬停 -->
                          <button
                            v-if="activityItemCanExpand(item)"
                            type="button"
                            class="flex h-4 w-4 shrink-0 items-center justify-center rounded hover:bg-base-300/50 transition-colors"
                            :class="activityItemTitleClass(item)"
                            :title="activityItemExpanded(item) ? t('common.collapse') : t('common.expand')"
                            data-selection-ignore="true"
                            @click.stop="toggleReasoningItemExpanded(item)"
                          >
                            <ChevronDown
                              class="h-3.5 w-3.5 transition-transform duration-150"
                              :class="{ 'rotate-180': activityItemExpanded(item) }"
                            />
                          </button>

                          <!-- 底沿淡出：只有标题行真的贴住滚动区顶部才挂上，避免没贴顶时盖住正文 -->
                          <div
                            v-if="activityItemExpanded(item) && activityItemCanExpand(item) && isReasoningItemStuck(item)"
                            class="ecall-reasoning-sticky-fade absolute top-full inset-x-0 h-3 pointer-events-none"
                          />
                        </div>

                        <!-- 正文区：折叠态展示 4 行预览 + 底部 base-200 淡出；展开态完整展示且由 Header 吸顶 -->
                        <!-- 正文区：有无正文看 hasBody，能不能展开另由箭头决定 -->
                        <div
                          v-if="activityItemHasBody(item)"
                          class="min-w-0 pb-1.5"
                        >
                          <!-- 正文外壳：折叠态限高约 4 行预览并加底部 base-200 淡出；展开态限高滚动 -->
                          <div
                            class="ecall-reasoning-body-shell relative min-w-0 whitespace-pre-wrap wrap-break-word text-xs leading-relaxed"
                            :class="[
                              activityItemDetailClass(item),
                              activityItemExpanded(item) ? 'ecall-reasoning-body--expanded' : 'ecall-reasoning-body--clamped pr-3',
                              !activityItemExpanded(item) && activityItemCanExpand(item) ? 'cursor-pointer' : '',
                            ]"
                            @click="onReasoningClampedBodyClick(item)"
                          >
                            <OverlayScrollArea v-if="activityItemExpanded(item)" scroller-class="max-h-80 pr-3" overscroll="auto">
                              <InlineMarkdownText v-if="!activityItemPlainBody(item)" :text="activityItemBodyText(item)" />
                              <template v-else>{{ activityItemBodyText(item) }}</template>
                            </OverlayScrollArea>
                            <template v-else>
                              <InlineMarkdownText v-if="!activityItemPlainBody(item)" :text="activityItemBodyText(item)" />
                              <template v-else>{{ activityItemBodyText(item) }}</template>
                            </template>

                            <!-- 展开态：上下各一条 0.5rem 淡出，滚到边缘时压住内容 -->
                            <template v-if="activityItemExpanded(item)">
                              <div
                                class="ecall-reasoning-sticky-fade absolute inset-x-0 top-0 h-2 pointer-events-none"
                              />
                              <div
                                class="ecall-reasoning-clamped-fade absolute inset-x-0 bottom-0 h-2 pointer-events-none"
                              />
                            </template>
                            <!-- 折叠态底部 base-200 淡出遮罩 -->
                            <div
                              v-else
                              class="ecall-reasoning-clamped-fade absolute inset-x-0 bottom-0 h-2 pointer-events-none"
                            />
                          </div>
                        </div>

                        <!-- 条目间距：右列末尾的留白行提供，左列竖线照常穿过 -->
                        <div
                          v-if="itemIndex !== presentedActivityItems.length - 1"
                          class="h-4 shrink-0"
                        />
                      </div>
                    </template>

                    <!-- ========== 工具与正文节点：同一套骨架（左列圆点与竖线 / 右列内容与留白行） ========== -->
                    <template v-else>
                      <div class="relative flex w-4 shrink-0 flex-col items-center">
                        <span
                          v-if="itemIndex !== 0"
                          class="absolute top-0 h-1 w-px bg-current opacity-30"
                        />
                        <div class="flex h-6 shrink-0 items-center justify-center">
                          <span
                            v-if="item.kind === 'tool' && item.status === 'doing'"
                            class="loading loading-spinner loading-xs text-primary"
                          ></span>
                          <svg
                            v-else-if="item.kind === 'content'"
                            viewBox="0 0 24 24"
                            class="h-4 w-4 text-base-content/60"
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
                        </div>
                        <!-- 时间轴竖线：结构内的一列，撑满条目（含右列末尾留白行），相邻条目在 li 边界处相接 -->
                        <span
                          v-if="itemIndex !== presentedActivityItems.length - 1"
                          class="w-px flex-1 bg-current opacity-30"
                        />
                      </div>
                      <div class="flex min-w-0 flex-1 flex-col">
                        <details
                          v-if="item.kind === 'tool' && activityItemCanExpand(item)"
                          class="collapse rounded-none"
                          :open="activityItemExpanded(item)"
                          @toggle="onToolDetailsToggle(item, $event)"
                        >
                          <summary class="collapse-title group flex min-h-0 items-center gap-1.5 pl-0 pr-1 py-1 text-xs hover:bg-base-200">
                            <span
                              class="ecall-activity-item-summary min-w-0 inline-flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap"
                              :class="activityItemTitleClass(item)"
                            >
                              <template v-if="activityItemSemantic(item)">
                                <span class="font-medium shrink-0">{{ activityItemSemantic(item)!.action }}</span>
                                <span class="truncate">{{ activityItemSemantic(item)!.target }}</span>
                                <span
                                  v-if="activityItemSemantic(item)!.lineRange"
                                  class="font-mono text-xs opacity-75 shrink-0"
                                >{{ activityItemSemantic(item)!.lineRange }}</span>
                                <span
                                  v-if="activityItemSemantic(item)!.extra"
                                  class="opacity-75 shrink-0 truncate"
                                >{{ activityItemSemantic(item)!.extra }}</span>
                                <span
                                  v-if="activityItemStatusSuffix(item)"
                                  class="opacity-75 shrink-0 font-normal"
                                >{{ activityItemStatusSuffix(item) }}</span>
                              </template>
                              <template v-else>
                                <span>{{ activityItemDisplay(item).text }}</span>
                              </template>
                              <span
                                v-if="activityItemDisplay(item).adds > 0"
                                class="ml-1 shrink-0 font-mono text-success"
                              >+{{ activityItemDisplay(item).adds }}</span>
                              <span
                                v-if="activityItemDisplay(item).removes > 0"
                                class="ml-1 shrink-0 font-mono text-error"
                              >-{{ activityItemDisplay(item).removes }}</span>
                            </span>
                            <ChevronDown
                              class="ecall-activity-chevron mt-0.5 h-3.5 w-3.5 shrink-0 transition-transform duration-150"
                            />
                          </summary>
                          <div class="collapse-content pb-2 pr-1 pt-1 pl-0">
                            <div class="relative">
                              <OverlayScrollArea scroller-class="max-h-72 py-2 pr-2" overscroll="auto">
                                <div class="flex flex-col gap-2">
                                  <!-- 卡片一：参数（输入）。不设标题行，内容直接当卡片 -->
                                  <div
                                    v-if="toolDetailParts(item).args"
                                    :class="['rounded-lg', FROST_GLASS]"
                                  >
                                    <!-- 参数区：整块走工具绿，其中的键名单独跳到行内代码红 -->
                                    <ToolArgsView
                                      :text="toolDetailParts(item).args"
                                      :dark="markdownIsDark"
                                      class="px-2.5 py-2 font-mono text-xs leading-relaxed"
                                    />
                                  </div>

                                  <!-- 卡片二：结果（输出）。默认空态，点一下才把结果取回来 -->
                                  <div
                                    v-if="toolDetailParts(item).result || (item.contentOmitted && toolResultOverrides[activityItemKey(item)] === undefined)"
                                    :class="['rounded-lg', FROST_GLASS]"
                                  >
                                    <!-- 结果区：沿用条目那一档 context 灰 -->
                                    <pre
                                      v-if="toolDetailParts(item).result"
                                      class="m-0 whitespace-pre-wrap break-all px-2.5 py-2 text-xs leading-relaxed"
                                      :class="markdownIsDark ? 'ecall-activity-result-dark' : 'ecall-activity-result'"
                                    ><code>{{ toolDetailParts(item).result }}</code></pre>
                                    <div v-else class="px-2.5 py-2">
                                      <button
                                        type="button"
                                        class="btn btn-ghost btn-xs -ml-1 h-6 min-h-0 justify-start gap-1.5 px-1 text-xs font-normal text-base-content/60 hover:bg-base-300/60 hover:text-base-content"
                                        :disabled="!!toolResultLoadingKeys[activityItemKey(item)]"
                                        @click.stop="loadToolResult(item)"
                                      >
                                        <span
                                          v-if="toolResultLoadingKeys[activityItemKey(item)]"
                                          class="loading loading-spinner loading-xs"
                                        ></span>
                                        <ChevronRight v-else class="h-3 w-3 shrink-0" />
                                        {{ t("chat.toolReview.viewResult") || "查看结果" }}
                                      </button>
                                      <span
                                        v-if="toolResultErrorKeys[activityItemKey(item)]"
                                        class="ml-2 text-error"
                                      >{{ toolResultErrorKeys[activityItemKey(item)] }}</span>
                                    </div>
                                  </div>
                                </div>
                              </OverlayScrollArea>
                              <!-- 滚动区上下边缘各 0.5rem 淡出 -->
                              <div
                                class="ecall-activity-fade-top absolute inset-x-0 top-0 h-2 bg-base-200 pointer-events-none"
                              />
                              <div
                                class="ecall-activity-fade-bottom absolute inset-x-0 bottom-0 h-2 bg-base-200 pointer-events-none"
                              />
                            </div>
                          </div>
                        </details>
                        <div
                          v-else-if="item.kind === 'tool'"
                          class="flex min-h-0 items-center gap-1.5 pr-1 py-1 text-xs"
                          @click.stop
                        >
                          <span
                            class="ecall-activity-item-summary min-w-0 flex-1 inline-flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap"
                            :class="activityItemTitleClass(item)"
                          >
                            <template v-if="activityItemSemantic(item)">
                              <span class="font-medium shrink-0">{{ activityItemSemantic(item)!.action }}</span>
                              <span class="truncate">{{ activityItemSemantic(item)!.target }}</span>
                              <span
                                v-if="activityItemSemantic(item)!.lineRange"
                                class="font-mono text-xs opacity-75 shrink-0"
                              >{{ activityItemSemantic(item)!.lineRange }}</span>
                              <span
                                v-if="activityItemSemantic(item)!.extra"
                                class="opacity-75 shrink-0 truncate"
                              >{{ activityItemSemantic(item)!.extra }}</span>
                              <span
                                v-if="activityItemStatusSuffix(item)"
                                class="opacity-75 shrink-0 font-normal"
                              >{{ activityItemStatusSuffix(item) }}</span>
                            </template>
                            <template v-else>
                              <span>{{ activityItemDisplay(item).text }}</span>
                            </template>
                            <span
                              v-if="activityItemDisplay(item).adds > 0"
                              class="ml-1 shrink-0 font-mono text-success"
                            >+{{ activityItemDisplay(item).adds }}</span>
                            <span
                              v-if="activityItemDisplay(item).removes > 0"
                              class="ml-1 shrink-0 font-mono text-error"
                            >-{{ activityItemDisplay(item).removes }}</span>
                          </span>
                        </div>
                        <!-- 正文条目：只露 20 字，没有展开入口 -->
                        <div v-else-if="item.kind === 'content'" class="flex pr-1 py-1">
                          <PlainMarkdownRenderer
                            class="min-w-0 flex-1 whitespace-pre-wrap wrap-break-word text-xs leading-relaxed"
                            :text="activityItemContentText(item)"
                          />
                        </div>
                        <!-- 条目间距：右列末尾的留白行提供，左列竖线照常穿过 -->
                        <div
                          v-if="itemIndex !== presentedActivityItems.length - 1"
                          class="h-4 shrink-0"
                        />
                      </div>
                    </template>
                  </li>
                </TransitionGroup>
                <button
                  type="button"
                  class="btn btn-sm mt-2 sticky bottom-0 z-10 w-full border border-base-content/15 bg-base-100/90 backdrop-blur text-base-content/70 hover:bg-base-200 hover:text-base-content hover:border-base-content/25 transition-colors"
                  data-selection-ignore="true"
                  @pointerdown.stop.prevent="closeActivityDetails(true)"
                  @click.stop
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
          ref="assistantBubbleRef"
          class="assistant-markdown ecall-assistant-bubble max-w-full"
          :class="{ 'ecall-assistant-bubble-wide': assistantNeedsWideBubble }"
          :data-bubble-background="assistantBubbleBackgroundEnabled ? 'on' : 'off'"
          :data-segmented-markdown="segmentedMarkdownEnabled ? 'on' : 'off'"
          :data-process-folded="processSegmentsFolded ? 'on' : 'off'"
        >
          <div v-if="block.text">
            <div
              v-if="plainMarkdownDebugEnabled"
              :ref="(el) => { activeStreamingSegmentEl = (el as HTMLElement) || null; }"
              :style="props.block.isStreaming ? streamingBubbleStyle : undefined"
              @click="emit('assistantLinkClick', $event)"
            >
              <PlainMarkdownRenderer :text="assistantRenderedText" />
            </div>
            <div v-else ref="markdownContainerRef">
              <div class="ecall-assistant-segment-list">
                <button
                  v-if="collapsibleProcessPieceCount > 0"
                  type="button"
                  class="ecall-process-fold"
                  @click.stop="toggleProcessSegments"
                >
                  <span>{{ processSegmentsFolded ? t("chat.previousProcessMessages", { count: collapsibleProcessPieceCount }) : t("common.collapse") }}</span>
                  <ChevronRight
                    class="h-3.5 w-3.5 shrink-0 transition-transform duration-200"
                    :class="processSegmentsFolded ? '' : 'rotate-90'"
                  />
                </button>
                <div
                  v-for="(piece, visibleIndex) in visibleAssistantPieces"
                  :key="piece.key"
                  :ref="(el) => setStreamingSegmentRef(el, piece.index)"
                  class="ecall-assistant-segment ecall-assistant-segment-text"
                  :data-segment-divider="visibleIndex === visibleAssistantPieces.length - 1 && (visibleAssistantPieces.length > 1 || processSegmentsFolded) ? 'last' : undefined"
                  :style="isStreamingPiece(piece.index) ? streamingBubbleStyle : undefined"
                >
                  <AppMarkdownRenderer
                    class="ecall-markdown-content max-w-none"
                    :text="piece.text"
                    :is-dark="markdownIsDark"
                    :streaming="!!block.isStreaming && piece.index === assistantMarkdownPieces.length - 1"
                    :local-image-base-path="currentWorkspaceRootPath"
                    :toolcall-preview-map="toolcallPreviewMap"
                    @math-context-menu="openMathContextMenu"
                    @open-image-preview="handleOpenImagePreview"
                    @click="handleAssistantLinkClick"
                  />
                </div>
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, watchEffect, watchPostEffect, type StyleValue } from "vue";
import { useI18n } from "vue-i18n";
import { Braces, ChevronDown, ChevronRight, Copy, ImageIcon, ListCheck, Split, Undo2 } from "@lucide/vue";
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
import { AppMarkdownRenderer, initKatex } from "../markdown";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import { normalizeLocalLinkHref } from "../utils/local-link";
import { textContentSignature } from "../utils/text-signature";
import { createStreamingSizeLock, observeStreamingSize, streamingSizeLockStyle } from "../utils/streaming-size-lock";
import { shouldFoldProcessSegments, type ProcessSegmentFoldIntent } from "../utils/process-segment-fold";
import { createToolCallPresentation } from "../utils/tool-call-presentation";
import { buildToolcallPreviewMap, parseToolCallResultStatus, type ToolcallPreviewEntry } from "../utils/toolcall-preview";
import { parseToolArgs } from "../utils/tool-args-tree";
import { FROST_GLASS } from "./session-float-styles";
import { generateShareFromMessageIds } from "../utils/share-generator";
import { frontendDispatchElapsedByMessageId } from "../composables/use-chat-flow-frontend-dispatch";
import { useCollapseTransition } from "../composables/use-collapse-transition";
import { displayFileName, extraTextReferenceDisplayParts } from "../utils/chat-attachment-display";
import { formatRecentRelativeTime } from "../../shared/utils/relative-time";
import ChatBubbleShell from "./ChatBubbleShell.vue";
import ChatAttachmentItem from "./ChatAttachmentItem.vue";
import PlainMarkdownRenderer from "./PlainMarkdownRenderer.vue";
import AnimatedCountText from "./AnimatedCountText.vue";
import ToolArgsView from "./ToolArgsView.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";

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
  collapseActivitySignal?: string;
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
  (e: "activityToggle", payload: { blockId: string; open: boolean; expandedKey?: string }): void;
  (e: "reasoningStuckChange", payload: { key: string; stuck: boolean }): void;
}>();

const { t } = useI18n();
const { animateEnter, animateLeave, cleanupAnimation } = useCollapseTransition();
const {
  assistantBubbleBackgroundEnabled,
  processMessagesFolded,
  segmentedMarkdownEnabled,
  chatTimeDisplayMode,
} = useChatMessageAppearance();
const {
  joinNonEmpty,
  compactText,
  normalizeToolCallArgs,
  toolCallDisplayName,
  toolCallSemanticPresentation,
  toolCallSummaryText,
  toolTimelineText,
} = createToolCallPresentation({
  t: (key, params) => String(t(key, params ?? {})),
  agentName: (agentId) => props.agentNameMap?.[agentId] || agentId,
});
const resolvedImageSrcMap = ref<Record<string, string>>({});
const markdownContainerRef = ref<HTMLElement | null>(null);
const activityDetailsRef = ref<HTMLDetailsElement | null>(null);
const activityExpanded = ref(false);
// 思维块展开态：只记用户手动改过的条目，没点过的一律收起
const activityItemExpandedOverrides = ref<Record<string, boolean>>({});
// 不同消息只能展开一个：活动面板打开时由父级记账，别的消息打开就收起这一条。
// expandedKey 只在条目级互斥上报时带，顶层面板开关不带，避免误收条目。
const collapseActivitySignal = computed(() => String(props.collapseActivitySignal || "").trim());
watch(collapseActivitySignal, (signal) => {
  if (!signal) return;
  const [blockId, expandedKey] = signal.split("|");
  if (String(blockId || "").trim() !== String(props.block.id || "").trim()) {
    closeActivityDetails(true);
    activityItemExpandedOverrides.value = {};
    return;
  }
  const key = String(expandedKey || "").trim();
  if (!key) return;
  // 同一条：只留上报的那个展开位
  const next: Record<string, boolean> = {};
  next[key] = true;
  activityItemExpandedOverrides.value = next;
});
// 工具结果按需加载：后端默认只下发占位文案（contentOmitted），
// 用户点「查看结果」后才把真实内容填进 toolResultOverrides。
const toolResultOverrides = ref<Record<string, string>>({});
const toolResultLoadingKeys = ref<Record<string, boolean>>({});
const toolResultErrorKeys = ref<Record<string, string>>({});
const copyMessageImageBusy = ref(false);
const planMarkdownText = ref("");
const planMarkdownError = ref("");
const planMarkdownLoading = ref(false);
const plainMarkdownDebugEnabled = debugPlainMarkdownRender;
const assistantRawRenderedText = computed(() => formatAssistantStreamingText(props.block));
const assistantNeedsWideBubble = computed(() => textNeedsWideBubble(assistantRawRenderedText.value));
const assistantRenderedText = computed(() =>
  assistantRawRenderedText.value.split(TOOL_TEXT_BREAK_PLACEHOLDER).join("\n\n"),
);
// 一段正文（工具调用之间的整段）即一个气泡；代码块 / 表格 / 图表内嵌其中，不切开气泡
// 渲染器只吃文本：增量解析与块身份由 AppMarkdownRenderer 内部负责
const assistantMarkdownPieces = computed<Array<{ key: string; text: string }>>(() => {
  if (plainMarkdownDebugEnabled) return [];
  const text = assistantRawRenderedText.value;
  if (!text) return [];
  const segmented = segmentedMarkdownEnabled.value;
  const pieces = segmented
    ? text.split(TOOL_TEXT_BREAK_PLACEHOLDER)
    : [assistantRenderedText.value];
  const result: Array<{ key: string; text: string }> = [];
  pieces.forEach((piece, pieceIndex) => {
    if (!piece.trim()) return;
    result.push({ key: `piece-${pieceIndex}`, text: piece });
  });
  return result;
});
/** auto 走默认；点过之后记成 expanded / collapsed，避免流式结束把手动展开收回去。 */
const processSegmentFoldIntent = ref<ProcessSegmentFoldIntent>("auto");
watch(() => props.block.id, () => {
  processSegmentFoldIntent.value = "auto";
});
const collapsibleProcessPieceCount = computed(() =>
  processMessagesFolded.value ? Math.max(0, assistantMarkdownPieces.value.length - 1) : 0,
);
const processSegmentsFolded = computed(() => shouldFoldProcessSegments({
  foldEnabled: processMessagesFolded.value,
  pieceCount: assistantMarkdownPieces.value.length,
  bubbleBackground: assistantBubbleBackgroundEnabled.value,
  streaming: !!props.block.isStreaming,
  intent: processSegmentFoldIntent.value,
}));
function toggleProcessSegments() {
  processSegmentFoldIntent.value = processSegmentsFolded.value ? "expanded" : "collapsed";
}
const visibleAssistantPieces = computed(() => {
  const pieces = assistantMarkdownPieces.value.map((piece, index) => ({ ...piece, index }));
  if (!processSegmentsFolded.value) return pieces;
  return pieces.slice(-1);
});
// ==================== 流式气泡尺寸防抖（单调非减尺寸锁定） ====================
// 锁只跟着当前正在输出的那一段。折叠收起上一段、或工具切开新段时，旧高度不能套到新段上。
const assistantBubbleRef = ref<HTMLElement | null>(null);
const activeStreamingSegmentEl = ref<HTMLElement | null>(null);
const streamingTargetEl = computed(() => activeStreamingSegmentEl.value || assistantBubbleRef.value);
const streamingSizeLock = ref(createStreamingSizeLock());
let streamingResizeObserver: ResizeObserver | null = null;
let releaseStreamingLockTimer: ReturnType<typeof setTimeout> | null = null;

function currentStreamingPieceIndex(): number {
  if (plainMarkdownDebugEnabled) return 0;
  const length = assistantMarkdownPieces.value.length;
  return length > 0 ? length - 1 : -1;
}

function isStreamingPiece(pieceIndex: number): boolean {
  return !!props.block.isStreaming && pieceIndex === assistantMarkdownPieces.value.length - 1;
}

function setStreamingSegmentRef(el: unknown, pieceIndex: number) {
  if (isStreamingPiece(pieceIndex)) {
    activeStreamingSegmentEl.value = (el as HTMLElement) || null;
  }
}

function isStreamingSegmentTarget(el: HTMLElement | null | undefined): el is HTMLElement {
  return !!el && el !== assistantBubbleRef.value;
}

const streamingBubbleStyle = computed<StyleValue | undefined>(() =>
  streamingSizeLockStyle(streamingSizeLock.value, currentStreamingPieceIndex()),
);

let streamingSizeRafId = 0;

function teardownStreamingObserver() {
  if (streamingSizeRafId) {
    window.cancelAnimationFrame(streamingSizeRafId);
    streamingSizeRafId = 0;
  }
  if (streamingResizeObserver) {
    streamingResizeObserver.disconnect();
    streamingResizeObserver = null;
  }
}

function clearStreamingReleaseTimer() {
  if (releaseStreamingLockTimer) {
    clearTimeout(releaseStreamingLockTimer);
    releaseStreamingLockTimer = null;
  }
}

function rememberStreamingSegmentSize(el: HTMLElement, pieceIndex: number) {
  streamingSizeLock.value = observeStreamingSize(
    streamingSizeLock.value,
    pieceIndex,
    Math.ceil(el.offsetHeight),
    Math.ceil(el.offsetWidth),
    el.parentElement?.clientWidth ?? 0,
  );
}

function setupStreamingObserver(el: HTMLElement) {
  teardownStreamingObserver();
  if (typeof ResizeObserver === "undefined") return;
  streamingResizeObserver = new ResizeObserver((entries) => {
    if (!props.block.isStreaming) return;
    if (streamingSizeRafId) return;
    streamingSizeRafId = window.requestAnimationFrame(() => {
      streamingSizeRafId = 0;
      if (!props.block.isStreaming) return;
      const pieceIndex = currentStreamingPieceIndex();
      for (const entry of entries) {
        const target = entry.target as HTMLElement;
        if (target !== activeStreamingSegmentEl.value) continue;
        const currentHeight = Math.ceil(target.offsetHeight || entry.contentRect.height);
        const currentWidth = Math.ceil(target.offsetWidth || entry.contentRect.width);
        const parentWidth = target.parentElement ? target.parentElement.clientWidth : 0;
        streamingSizeLock.value = observeStreamingSize(
          streamingSizeLock.value,
          pieceIndex,
          currentHeight,
          currentWidth,
          parentWidth,
        );
      }
    });
  });
  streamingResizeObserver.observe(el);
}

watch(
  () => [props.block.id, props.block.isStreaming, currentStreamingPieceIndex(), streamingTargetEl.value] as const,
  ([messageId, isStreaming, pieceIndex, el], prev) => {
    const prevMessageId = prev?.[0];
    const prevStreaming = prev?.[1];
    const prevPieceIndex = prev?.[2];
    const prevEl = prev?.[3] ?? null;
    const messageChanged = prevMessageId !== undefined && messageId !== prevMessageId;
    if (messageChanged) {
      streamingSizeLock.value = createStreamingSizeLock();
      clearStreamingReleaseTimer();
    }
    if (isStreaming) {
      clearStreamingReleaseTimer();
      const pieceChanged = prevPieceIndex !== pieceIndex;
      const segmentBecameAvailable = isStreamingSegmentTarget(el) && !isStreamingSegmentTarget(prevEl);
      if (
        isStreamingSegmentTarget(el)
        && (!prevStreaming || pieceChanged || messageChanged || segmentBecameAvailable)
      ) {
        rememberStreamingSegmentSize(el, pieceIndex);
      }
      if (isStreamingSegmentTarget(el)) setupStreamingObserver(el);
      else teardownStreamingObserver();
    } else {
      teardownStreamingObserver();
      clearStreamingReleaseTimer();
      // 流式结束，留出短暂缓冲让最终渲染稳定后再平滑释放锁定
      releaseStreamingLockTimer = setTimeout(() => {
        streamingSizeLock.value = createStreamingSizeLock();
        releaseStreamingLockTimer = null;
      }, 120);
    }
  },
  { immediate: true, flush: "post" },
);

const teleportTheme = computed(() => {
  const documentTheme = typeof document === "undefined" ? "" : document.documentElement.getAttribute("data-theme");
  return String(props.currentTheme || documentTheme || "light").trim() || "light";
});
let disposed = false;

const contextMenuOpen = ref(false);
const contextMenuRef = ref<HTMLElement | null>(null);
// 本条消息的根节点：选区归属与右键抑制都要按它判定，不能只看全局选区
const messageRootRef = ref<HTMLElement | null>(null);
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
  return formatRecentRelativeTime(props.block.createdAt, relativeTimeNowTick.value, t);
});
const assistantMetaText = assistantCreatedAtText;
const streamingHeaderStatus = computed(() => assistantStreamingHeaderStatus(props.block));
const toolcallPreviewMap = computed<Record<string, ToolcallPreviewEntry>>(() => {
  const previews = buildToolcallPreviewMap(props.block.activityItems, toolTimelineText("noArgs"));
  for (const item of props.block.activityItems) {
    if (item.kind !== "tool") continue;
    const toolCallId = String(item.toolCallId || "").trim();
    if (!toolCallId || !previews[toolCallId]) continue;
    previews[toolCallId].title = activityItemTitle(item);
    const semantic = toolCallSemanticPresentation(item);
    if (semantic) {
      previews[toolCallId].action = semantic.action;
      const targetWithRange = [semantic.target, semantic.lineRange].filter(Boolean).join(" ");
      previews[toolCallId].target = targetWithRange;
      previews[toolCallId].fileLabel = targetWithRange;
      previews[toolCallId].extra = semantic.extra;
    }
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

function retryStatusLabel(text: string): string {
  const value = String(text || "").trim();
  if (!value) return "";
  if (!(value.includes("重试") || value.includes("Retrying"))) return "";
  const match = value.match(/\((\d+)\s*\/\s*(\d+)\)/);
  if (match) {
    return t("chat.statusRetrying", { current: match[1], total: match[2] });
  }
  return t("chat.statusRetryingPlain");
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
  const toolStatusText = String(providerMeta._toolStatusText || "").trim();
  const retryLabel = retryStatusLabel(toolStatusText);
  if (retryLabel) {
    return retryLabel;
  }
  // 工具真在跑时报具体工具名：后端刻意把 _schedulingState 留在 executing_tool，
  // 只看阶段只能得到笼统的「正在执行工具」。streaming_tool 例外——那时参数还在生成，
  // 说「正在执行」不准确，仍按原分支报「正在生成工具调用」。
  if (schedulingState !== "streaming_tool") {
    const doingToolCall = toolCallsForBlock(block).find((call) => call.status === "doing");
    if (doingToolCall) {
      // 语义句会把整条命令塞进头部、长命令直接把这一行撑满，所以头部只取工具显示名，
      // 目标与行号范围留给「思考与工具」折叠块里的摘要行去报。
      const namedTool = compactText(toolCallDisplayName(doingToolCall.name), 32);
      return withWater(t("chat.statusExecutingNamedTool", { name: namedTool }));
    }
  }
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
        return t("chat.statusExecutingTools");
      case "idle":
        return "";
      default:
        break;
    }
  }
  // Fallback：旧后端无 schedulingState 时，仅读 tool_status，不再以 hasReasoning -> 已阅读消息 推断
  const preStreamingStatusText = String(providerMeta._preStreamingStatusText || "").trim();
  const toolStatusState = String(providerMeta._toolStatusState || "").trim();
  const hasSpeechContent = hasStreamingSpeechContent(block);

  const normalizeRequestPhaseText = (text: string): string => {
    if (!text) return "";
    const retryLabel = retryStatusLabel(text);
    if (retryLabel) {
      return retryLabel;
    }
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

  if (block.activityStatus === "running_tool") {
    return t("chat.statusExecutingTools");
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
  // 队伍消息（邮件与通知）都是服务端写死的站内信，没有思考也没有工具，
  // 「即答」这一行只是噪声。按 teamMessage 一刀切，将来加第三种队伍消息也不会漏。
  if (block.providerMeta?.teamMessage === true) return false;
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

/**
 * 转换层把同一批 running 标到每条上，不能据此区分已闭合与正在生长。
 * 只有最后一条思维或正文还可能继续变长，前面的条目一律视为已闭合。
 */
const presentedActivityItems = computed(() => {
  const items = resolvedActivityItems(props.block);
  if (!props.block.isStreaming || items.length === 0) return items;
  let liveIndex = -1;
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const kind = items[index].kind;
    if (kind === "reasoning" || kind === "content") {
      liveIndex = index;
      break;
    }
  }
  if (liveIndex < 0) return items;
  return items.map((item, index) => (
    index === liveIndex || item.kind === "tool" ? item : { ...item, running: false }
  ));
});

const collapsedActivityPreviewCache = new WeakMap<object, { source: string; preview: string }>();
const COLLAPSED_ACTIVITY_PREVIEW_CHARS = 240;

/** 思维链折叠态的可视行数（与 .ecall-reasoning-body--clamped 的 line-clamp 一致）。 */
const REASONING_COLLAPSED_LINES = 4;
/** 一行按约 36 个汉字估（12px 字号下的聊天窗宽度）。只用来决定要不要给展开箭头，不参与截断。 */
const REASONING_CHARS_PER_LINE = 36;
/** 超过这个字数才可能出现第 5 行，才值得给展开入口。估小不估大：漏给箭头会让内容打不开，多给只是多余。 */
const REASONING_EXPAND_MIN_CHARS = REASONING_COLLAPSED_LINES * REASONING_CHARS_PER_LINE;

type ToolDetailParts = { args: string; result: string };
const EMPTY_TOOL_DETAIL_PARTS: ToolDetailParts = { args: "", result: "" };
const toolDetailPartsCache = new WeakMap<ChatActivityItem, { stamp: string; parts: ToolDetailParts }>();
/** 思维链富文本最大渲染字数，超过此长度自动降级为原生纯文本，避免数十万字在正则解析和 VNode 树构建时阻塞主线程 */
const MAX_MARKDOWN_REASONING_CHARS = 8000;

/** 折叠预览按固定字数截断。没有换行时也不能把全文放进页面。 */
function collapsedActivityPreview(text: string, cacheKey: object): string {
  const source = String(text || "");
  const cached = collapsedActivityPreviewCache.get(cacheKey);
  if (cached && cached.source === source) return cached.preview;
  const preview = source.slice(0, COLLAPSED_ACTIVITY_PREVIEW_CHARS);
  collapsedActivityPreviewCache.set(cacheKey, { source, preview });
  return preview;
}

/** 生长中的正文只看长度和末尾字符，避免每个新字都扫描全文。 */
function liveActivityTextSignature(text: string): string {
  const length = text.length;
  return `${length}:${length > 0 ? text.charCodeAt(length - 1) : 0}`;
}

function activityItemPlainBody(item: ChatActivityItem): boolean {
  if (item.kind !== "reasoning") return false;
  if (item.running && activityItemExpanded(item)) return true;
  return activityItemText(item).length > MAX_MARKDOWN_REASONING_CHARS;
}

function activityItemBodyText(item: ChatActivityItem): string {
  const text = activityItemText(item);
  if (item.kind !== "reasoning") return text;
  if (!activityItemExpanded(item)) return collapsedActivityPreview(text, item);
  return text;
}

/** 已闭合条目只保留身份。正在生长的条目用常量级签名，不哈希全文。 */
function activityItemMemo(item: ChatActivityItem, isLast: boolean): unknown[] {
  const expanded = activityItemExpanded(item);
  // isLast 必须进依赖：末条追加新条目后会被翻转成非末条，要补上竖线与末尾留白行
  if (item.kind === "tool") {
    return [activityItemKey(item), item.status || "", textContentSignature(item.argsText), textContentSignature(item.resultText), isLast];
  }
  if (!item.running) return [activityItemKey(item), expanded, isLast];
  return [activityItemKey(item), expanded, liveActivityTextSignature(activityItemText(item)), isLast];
}

function activityOpenPanelSignature(items: ChatActivityItem[]): string {
  return items
    .map((item) => {
      const key = activityItemKey(item);
      if (item.kind === "tool") {
        return [
          key,
          String(item.toolCallId || "").trim(),
          String(item.name || "").trim(),
          String(item.status || "").trim(),
          textContentSignature(item.argsText),
          textContentSignature(item.resultText),
        ].join(":");
      }
      if (!item.running) return `${key}:closed`;
      return `${key}:live:${liveActivityTextSignature(activityItemText(item))}`;
    })
    .join("|");
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

function closeActivityDetails(silent = false): void {
  const details = activityDetailsRef.value;
  if (details instanceof HTMLDetailsElement) {
    details.open = false;
  }
  activityExpanded.value = false;
  if (silent) return;
  emit("activityToggle", { blockId: String(props.block.id || ""), open: false });
}

function hasActivityReasoning(block: ChatMessageBlock): boolean {
  return Number(block.activityReasoningCharCount || 0) > 0;
}

/**
 * 思考是否还在跑：只要还有一条 reasoning 条目 running，就认为思考没结束。
 * 折叠态读 activityItems、展开态读 contentBlocks 派生结果，两者都经由 presentedActivityItems 收敛。
 */
function activityReasoningRunning(): boolean {
  return presentedActivityItems.value.some(
    (item) => item.kind === "reasoning" && item.running,
  );
}

function activityReasoningCountLabel(block: ChatMessageBlock): string {
  const count = Number(block.activityReasoningCharCount || 0);
  if (count <= 0 || activityReasoningRunning()) return "";
  return `（${count.toLocaleString("zh-CN")}）`;
}

function activityStatusText(block: ChatMessageBlock): string {
  return hasActivityReasoning(block) ? t("chat.messageItem.thought") : "";
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
    // 展开时只跟踪条目结构和正在生长的那一条。已闭合条目的全文不进签名。
    ...(panelOpen ? [activityOpenPanelSignature(presentedActivityItems.value), activityItemExpandedOverrides.value] : []),
  ];
}

function activityItemKey(item: ChatActivityItem): string {
  return `${item.kind}:${String(item.id || "")}`;
}

function activityItemExpanded(item: ChatActivityItem): boolean {
  return activityItemExpandedOverrides.value[activityItemKey(item)] === true;
}

/** 最新一条正在流式的活动块（思维块或内容块）：流式生长时不设高度上限 */
const newestStreamingActivityKey = computed(() => {
  const items = resolvedActivityItems(props.block);
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item.kind === "reasoning" || item.kind === "content") return activityItemKey(item);
  }
  return "";
});

/** 展开且仍在流式中的最新思维或内容块不设高度上限，随内容自然生长 */
function activityItemFollowsStream(item: Extract<ChatActivityItem, { kind: "reasoning" | "content" }>): boolean {
  if (!item.running || !activityItemExpanded(item)) return false;
  return activityItemKey(item) === newestStreamingActivityKey.value;
}

function onActivityItemExpandedChange(item: ChatActivityItem, expanded: boolean): void {
  if (expanded) {
    // 同一条消息内只留一个展开位：点开谁就收起别的（思维/工具互斥）
    const key = activityItemKey(item);
    const next: Record<string, boolean> = {};
    next[key] = true;
    activityItemExpandedOverrides.value = next;
    // 跨消息只留一个：通知父级收起别的消息里的展开
    emit("activityToggle", {
      blockId: String(props.block.id || ""),
      open: true,
      expandedKey: key,
    });
    return;
  }
  activityItemExpandedOverrides.value = {
    ...activityItemExpandedOverrides.value,
    [activityItemKey(item)]: false,
  };
}

function onToolDetailsToggle(item: ChatActivityItem, event: Event): void {
  const details = event.target instanceof HTMLDetailsElement
    ? event.target
    : (event.currentTarget instanceof HTMLDetailsElement ? event.currentTarget : null);
  const open = details ? details.open : !activityItemExpanded(item);
  onActivityItemExpandedChange(item, open);
}

function toggleReasoningItemExpanded(item: ChatActivityItem): void {
  const current = activityItemExpanded(item);
  onActivityItemExpandedChange(item, !current);
}

function onReasoningHeaderClick(item: ChatActivityItem, event: MouseEvent): void {
  if (props.selectionModeEnabled || !activityItemCanExpand(item)) return;
  if (hasNativeTextSelection()) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest('button, a, input, textarea, select, [data-selection-ignore="true"]')) {
    return;
  }
  toggleReasoningItemExpanded(item);
}

function onReasoningClampedBodyClick(item: ChatActivityItem): void {
  if (props.selectionModeEnabled || activityItemExpanded(item)) return;
  if (!activityItemCanExpand(item)) return;
  if (hasNativeTextSelection()) return;
  toggleReasoningItemExpanded(item);
}

const reasoningItemStuckKeys = ref<Set<string>>(new Set());

function isReasoningItemStuck(item: ChatActivityItem): boolean {
  return reasoningItemStuckKeys.value.has(activityItemKey(item));
}

function setReasoningItemStuck(key: string, stuck: boolean): void {
  const currentHas = reasoningItemStuckKeys.value.has(key);
  if (stuck === currentHas) return;
  const next = new Set(reasoningItemStuckKeys.value);
  if (stuck) next.add(key);
  else next.delete(key);
  reasoningItemStuckKeys.value = next;
  const blockId = String(props.block.id || "").trim();
  emit("reasoningStuckChange", { key: blockId ? `${blockId}::${key}` : key, stuck });
}

const stickySentinelCleanups = new Map<string, () => void>();

function findScrollContainer(element: HTMLElement | null): HTMLElement | null {
  if (!element) return null;
  return element.closest(".ecall-chat-scroll-container") as HTMLElement | null;
}

function bindStickySentinel(key: string, el: HTMLElement | null): void {
  const existing = stickySentinelCleanups.get(key);
  if (existing) {
    existing();
    stickySentinelCleanups.delete(key);
  }
  if (!el) {
    setReasoningItemStuck(key, false);
    return;
  }

  const scrollRoot = findScrollContainer(el);
  if (!scrollRoot) {
    setReasoningItemStuck(key, false);
    return;
  }

  let frame = 0;
  const update = () => {
    frame = 0;
    const header = el.nextElementSibling instanceof HTMLElement ? el.nextElementSibling : null;
    if (!header) {
      setReasoningItemStuck(key, false);
      return;
    }
    const stickyTop = Number.parseFloat(window.getComputedStyle(header).top);
    const pinTop = scrollRoot.getBoundingClientRect().top + (Number.isFinite(stickyTop) ? stickyTop : 0);
    const headerTop = header.getBoundingClientRect().top;
    const sentinelTop = el.getBoundingClientRect().top;
    // 哨兵滑到标题行上方，且标题行停在吸顶线上，才算贴顶。
    const stuck = sentinelTop < headerTop - 1 && Math.abs(headerTop - pinTop) <= 3;
    setReasoningItemStuck(key, stuck);
  };
  const schedule = () => {
    if (frame) return;
    frame = window.requestAnimationFrame(update);
  };

  scrollRoot.addEventListener("scroll", schedule, { passive: true });
  window.addEventListener("resize", schedule);
  schedule();
  stickySentinelCleanups.set(key, () => {
    scrollRoot.removeEventListener("scroll", schedule);
    window.removeEventListener("resize", schedule);
    if (frame) window.cancelAnimationFrame(frame);
  });
}

function activityItemText(item: ChatActivityItem): string {
  if (item.kind === "content") return stripToolcallMarkers(item.text);
  if (item.kind === "reasoning") return String(item.text || "");
  return "";
}

/** 条目有没有正文可显示。与「还有没有折叠起来的内容」是两件事：短思维链要显示正文，但不该给展开箭头。 */
function activityItemHasBody(item: ChatActivityItem): boolean {
  if (item.kind === "reasoning") return !!activityItemText(item).trim();
  if (item.kind === "tool") {
    return !!toolDetailParts(item).args
      || !!String(item.resultText || "").trim()
      || item.contentOmitted === true;
  }
  return false;
}

function activityItemCanExpand(item: ChatActivityItem): boolean {
  if (!activityItemHasBody(item)) return false;
  // 工具条目有正文就能展开：参数或结果总有一块可看
  if (item.kind === "tool") return true;
  if (item.kind === "reasoning") {
    const text = activityItemText(item).trim();
    // 折叠态是 line-clamp 4 行：文字没超过 4 行时，展开与不展开看到的是同一段，
    // 这种条目不该给箭头。换行直接占行，先按行数判，再按字数估。
    if (text.split("\n").length > REASONING_COLLAPSED_LINES) return true;
    return text.length > REASONING_EXPAND_MIN_CHARS;
  }
  // 正文条目不给展开入口：折叠块里的正文只露头一小截
  return false;
}

/** 正文条目在折叠块里只露出的字数。不提供展开，超出部分就写省略号。 */
const ACTIVITY_CONTENT_PREVIEW_CHARS = 40;

/** 正文条目的渲染文本：固定 20 字收尾，不随任何展开状态变化。 */
function activityItemContentText(item: ChatActivityItem): string {
  const text = activityItemText(item);
  if (text.length <= ACTIVITY_CONTENT_PREVIEW_CHARS) return text;
  return `${text.slice(0, ACTIVITY_CONTENT_PREVIEW_CHARS)}…`;
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

/**
 * 参数区实际要显示的文本。
 * 空对象 / 空数组摊平后一行字段都没有，展开只会看到一对括号，按无参数处理；
 * 不能直接判原文非空——`{}` 也是两个字符。
 */
function toolArgsDisplayText(item: ChatActivityItem): string {
  const text = activityToolArgsText(item);
  const trimmed = text.trim();
  if (!trimmed) return "";
  if (parseToolArgs(trimmed)) return text;
  if (trimmed === "{}" || trimmed === "[]") return "";
  return text;
}

function activityItemNodeClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.markdownIsDark ? "ecall-activity-reasoning-dark" : "ecall-activity-reasoning";
  }
  if (item.kind === "content") return "";
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "text-warning";
    if (status.isFailed) return "text-error";
  }
  return props.markdownIsDark ? "ecall-activity-tool-dark" : "ecall-activity-tool";
}

function activityItemTitleClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.markdownIsDark ? "ecall-activity-reasoning-dark" : "ecall-activity-reasoning";
  }
  if (item.kind === "content") return "";
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
  if (item.kind === "content") return "";
  return props.markdownIsDark ? "ecall-activity-tool-dark" : "ecall-activity-tool";
}

function activityItemStatusSuffix(item: ChatActivityItem): string {
  if (item.kind !== "tool" || !item.resultText) return "";
  const status = parseToolCallResultStatus(item.resultText);
  if (status.isDenied) return `(${t("chat.toolReview.denied") || "已拒绝"})`;
  if (status.isFailed) return `(${t("chat.toolReview.failed") || "失败"})`;
  return "";
}

function activityItemSemantic(item: ChatActivityItem) {
  if (item.kind !== "tool") return null;
  return toolCallSemanticPresentation(item);
}

function activityItemTitle(item: ChatActivityItem): string {
  // 思考块标题固定成文案：首行切分那套像素估算已经去掉，标题不再跟着正文首句变
  if (item.kind === "reasoning") return t("chat.messageItem.reasoningTitle");
  if (item.kind === "content") return activityItemText(item);
  const baseTitle = toolCallSummaryText(item);
  const suffix = activityItemStatusSuffix(item);
  return suffix ? `${baseTitle} ${suffix}` : baseTitle;
}

function toolCallDiffStats(toolCall: { name: string; argsText: string; resultText?: string }): { adds: number; removes: number } {
  if (toolCall.resultText) {
    const status = parseToolCallResultStatus(toolCall.resultText);
    if (status.isDenied || status.isFailed) {
      return { adds: 0, removes: 0 };
    }
  }
  const semantic = toolCallSemanticPresentation(toolCall);
  return {
    adds: semantic.adds || 0,
    removes: semantic.removes || 0,
  };
}

async function loadToolResult(item: ChatActivityItem): Promise<void> {
  if (item.kind !== "tool") return;
  const key = activityItemKey(item);
  const conversationId = String(props.activeConversationId || "").trim();
  const messageId = String(props.block.sourceMessageId || props.block.id || "").trim();
  const toolCallId = String(item.toolCallId || "").trim();
  if (!conversationId || !messageId || !toolCallId) return;
  if (toolResultLoadingKeys.value[key]) return;
  toolResultLoadingKeys.value = { ...toolResultLoadingKeys.value, [key]: true };
  toolResultErrorKeys.value = { ...toolResultErrorKeys.value, [key]: "" };
  try {
    const content = await invokeTauri<string>("conversation.toolResultContent", {
      input: { conversationId, messageId, toolCallId },
    });
    toolResultOverrides.value = { ...toolResultOverrides.value, [key]: String(content || "") };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toolResultErrorKeys.value = { ...toolResultErrorKeys.value, [key]: message };
  } finally {
    toolResultLoadingKeys.value = { ...toolResultLoadingKeys.value, [key]: false };
  }
}

// 参数与结果拆成两块：参数整块走工具绿、其中键名单独标红；结果保持原来的灰。
// 原来的 '\n\n---\n\n' 分隔线随之取消。
function buildToolDetailParts(item: ChatActivityItem, override: string | undefined): ToolDetailParts {
  if (item.kind !== "tool") return { args: "", result: "" };
  const args = toolArgsDisplayText(item);
  // 占位结果已被省略时，优先展示按需加载回来的真实结果；
  // 未加载时结果区由模板渲染「查看结果」按钮，这里只返回参数部分。
  if (item.contentOmitted) {
    return { args, result: override === undefined ? "" : String(override) };
  }
  const result = String(item.resultText || "").trim();
  if (!result) return { args, result: "" };
  const status = parseToolCallResultStatus(item.resultText);
  if (status?.isDenied) {
    const reason = status.blockedReason || status.message;
    const label = reason
      ? `[${t("chat.toolReview.denied") || "已拒绝"}]: ${reason}`
      : `[${t("chat.toolReview.denied") || "已拒绝"}]`;
    return { args, result: `${result}\n\n${label}` };
  }
  if (status?.isFailed) {
    const reason = status.message || status.blockedReason;
    const label = reason
      ? `[${t("chat.toolReview.failed") || "执行失败"}]: ${reason}`
      : `[${t("chat.toolReview.failed") || "执行失败"}]`;
    return { args, result: `${result}\n\n${label}` };
  }
  return { args, result };
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

/** 参数与结果都含长文本拼接和 JSON 解析，按 item 缓存；流式期间靠 stamp 比对失效。 */
function toolDetailParts(item: ChatActivityItem): ToolDetailParts {
  if (item.kind !== "tool") return EMPTY_TOOL_DETAIL_PARTS;
  const override = toolResultOverrides.value[activityItemKey(item)];
  const stamp = [
    String(item.argsText || ""),
    String(item.resultText || ""),
    item.contentOmitted ? "1" : "0",
    override ?? "\u0000",
  ].join("\u0001");
  const cached = toolDetailPartsCache.get(item);
  if (cached && cached.stamp === stamp) return cached.parts;
  const parts = buildToolDetailParts(item, override);
  toolDetailPartsCache.set(item, { stamp, parts });
  return parts;
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
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return false;
    if (!String(selection.toString() || "").trim()) return false;
    // 选区必须落在本条消息内：主会话与追问面板同时存在两份 ChatView，
    // 只看全局有没有选区会让别处的选区压住这里的右键菜单和推理块展开。
    const root = messageRootRef.value;
    if (!root) return false;
    const anchorNode = selection.anchorNode;
    const focusNode = selection.focusNode;
    return (!!anchorNode && root.contains(anchorNode)) || (!!focusNode && root.contains(focusNode));
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

// 传给 AppMarkdownRenderer 的事件处理器必须是稳定引用：写成模板里的内联箭头，
// 每次父组件渲染都会生成新函数，子组件 props 被判为变化，块级边界整片失效
function handleOpenImagePreview(image: { mime?: string; bytesBase64?: string; dataUrl?: string; localPath?: string; src?: string; alt?: string }): void {
  emit("openImagePreview", image);
}

function handleAssistantLinkClick(event: MouseEvent): void {
  emit("assistantLinkClick", event);
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
  return formatThinkAsMarkdown(String(block.text || ""));
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

function textNeedsWideBubble(text: string): boolean {
  return /```(?:\s*)mermaid\b/i.test(text)
    || /```[\w-]*\s*[\r\n]/i.test(text)
    || /\|[^\n\r]+\|\s*[\r\n]\s*\|(?:\s*:?-+:?\s*\|)+/m.test(text)
    || /^\s*\|.+?\|.+?\|/m.test(text);
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
  stickySentinelCleanups.forEach((cleanup) => cleanup());
  stickySentinelCleanups.clear();
  const blockId = String(props.block.id || "").trim();
  for (const key of reasoningItemStuckKeys.value) {
    emit("reasoningStuckChange", { key: blockId ? `${blockId}::${key}` : key, stuck: false });
  }
  reasoningItemStuckKeys.value = new Set();
  teardownStreamingObserver();
  clearStreamingReleaseTimer();
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
/* 浅色主题：思维链橙色、工具绿色（加深保证白底可读）；正文不用单独色，inherit 面板正文色 */
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

/* 工具结果：青色。与参数的工具绿（142°）拉开 45° 色相，输入/输出一眼分得开。
   亮度与饱和度按同一套规矩跟橙绿对齐：浅色偏深偏饱和，深色提亮降饱和 */
:deep(.ecall-activity-result) {
  color: #188a98;
}

:deep(.ecall-activity-result-dark) {
  color: #54c5d4;
}

.ecall-activity-item-summary {
  /* 单行截断：长命令/路径用 ellipsis 收尾，不允许多行撑开标题。
     完整内容在 <details> 展开后的详情区可见。 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 条目 details 原生开合：chevron 旋转由 details[open] 驱动，不进 Vue 状态 */
.ecall-activity-timeline details[open] .ecall-activity-chevron {
  transform: rotate(180deg);
}

.ecall-reasoning-sticky-fade {
  /* 用遮罩淡出，避免渐变插值到 transparent 时混进黑色 */
  background-color: var(--color-base-200);
  -webkit-mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
  mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
}

.ecall-reasoning-body--clamped {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
  overflow: hidden;
  position: relative;
  /* 双保底：基于 line-height: 1.625 (leading-relaxed) 随字号缩放，绝不在文字中间切断 */
  max-height: calc(4 * 1.625em);
}

.ecall-reasoning-body--expanded {
  max-height: none;
  overflow: visible;
}

.ecall-reasoning-clamped-fade {
  background-color: var(--color-base-200);
  -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
  mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
}

/* 工具参数区滚动边缘淡出：只做 mask 渐变，底色由 Tailwind 类给，
   跟着参数区那层半透明底走，不硬塞实色 base-200 */
.ecall-activity-fade-top {
  -webkit-mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
  mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
}

.ecall-activity-fade-bottom {
  -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
  mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
}

:deep(.ecall-activity-timeline .ecall-plain-markdown-markdown > :first-child) {
  margin-top: 0 !important;
}

/* 正文条目与工具、思维链同一个字号：渲染器根节点自带的是聊天正文那档字号
   （--app-chat-message-text-size），比折叠块里其他条目大一档，这里压回 text-xs。 */
:deep(.ecall-activity-timeline .ecall-plain-markdown-markdown) {
  font-size: var(--app-text-xs-size, 0.75rem) !important;
}

/* 正文条目落一档：思维链与工具是彩色（橙/绿），正文只要 60% 的 base-content，
   三档靠色相与深浅同时分开。用 color-mix 而不是 opacity，只淡文字不淡里面的代码块底色 */
:deep(.ecall-activity-timeline .ecall-plain-markdown-markdown),
:deep(.ecall-activity-timeline .ecall-plain-markdown-markdown p) {
  color: color-mix(in oklab, var(--color-base-content) 60%, transparent) !important;
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

.assistant-markdown :deep(.ecall-markdown-content :where(table,.table-node)),
.assistant-markdown :deep(.ecall-md-table) {
  font-size: var(--app-text-sm-size);
  margin: 0 !important;
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
}

.assistant-markdown {
  --ecall-chat-rich-block-bg: var(--color-base-100);
}

/* 有气泡背景且不分段：表格单元格 / 引用块 / 折叠块用 base-200 拉开层次；其余场景一律 base-100 */
.assistant-markdown[data-bubble-background="on"][data-segmented-markdown="off"] {
  --ecall-chat-rich-block-bg: var(--color-base-200);
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  background: var(--ecall-chat-rich-block-bg);
}

.assistant-markdown :deep(.ecall-markdown-content .ecall-md-details) {
  background: transparent;
  border: 0;
  padding: 0;
}

.ecall-assistant-segment-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.ecall-process-fold {
  display: inline-flex;
  align-self: flex-start;
  align-items: center;
  gap: 0.25rem;
  padding: 0.125rem 0;
  border: 0;
  background: transparent;
  color: color-mix(in srgb, var(--color-base-content) 45%, transparent);
  font-size: 0.875rem;
  line-height: 1.25rem;
  cursor: pointer;
}

.ecall-process-fold:hover {
  color: var(--color-base-content);
}

.ecall-assistant-bubble[data-bubble-background="on"] .ecall-process-fold {
  gap: 0.375rem;
  padding: 0.45rem 0.8rem;
  border-radius: var(--radius-box, 1rem);
  background: var(--color-base-100);
}

/* 无气泡模式：中间段不再画线，也不再留出线的位置 */
.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment-list {
  gap: 0;
}

.ecall-assistant-segment {
  min-width: 0;
}

/* 隐藏气泡背景：正文贴到容器左缘（与头像左缘对齐），不再按气泡内边距缩进 */
.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment {
  position: relative;
  width: 100%;
  padding-right: 0;
  padding-left: 0;
}

.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment::before {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  height: 1px;
  background: color-mix(in srgb, var(--color-base-content) 14%, transparent);
  content: "";
  pointer-events: none;
  transform: scaleY(0.5);
  transform-origin: center;
}

/* 展开时只在最后一段前面留线；折叠后页面上只剩一段，线留在「上 N 条」和这段之间。线自己占一段间距 */
.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment[data-segment-divider="last"] {
  margin-top: 1rem;
}

.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment[data-segment-divider="last"]::before {
  top: -0.5rem;
}

.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment:not([data-segment-divider="last"])::before {
  display: none;
}

/* 无气泡模式：首段上方没有气泡边界，那段顶部内边距就是凭空多出来的空白，去掉 */
.ecall-assistant-bubble[data-bubble-background="off"] .ecall-assistant-segment-list > .ecall-assistant-segment:first-child {
  padding-top: 0;
}

/* 段即一个气泡（计划卡等列表外的段也走这里） */
.ecall-assistant-segment-text {
  display: inline-block;
  width: fit-content;
  max-width: 100%;
  padding: 0.68rem 1rem;
}

/* 段里带富块（代码块 / 表格 / 图表 / 沙箱 / 公式）时回到整行宽度，否则 fit-content 会把内容压窄 */
.ecall-assistant-segment:has(
  .ecall-md-code-block,
  .ecall-md-table-wrap,
  .ecall-md-mermaid-shell,
  .ecall-sandbox-card,
  .ecall-md-sandbox-skeleton,
  .ecall-agent-embed-shell,
  .ecall-md-math-block
) {
  display: block;
  width: 100%;
}

/* ==================== 1. 气泡模式：富块化为横向通栏楼层（Band Floor） ==================== */
.ecall-assistant-bubble[data-bubble-background="on"] {
  --ecall-bubble-floor-bg: color-mix(in srgb, var(--color-base-200) 50%, var(--color-base-100));
  --ecall-bubble-floor-border: color-mix(in srgb, var(--color-base-content) 10%, transparent);
}

.ecall-assistant-bubble[data-bubble-background="on"] .ecall-assistant-segment-text {
  border-radius: var(--radius-box, 1rem);
  background: var(--color-base-100);
  overflow: hidden; /* 保证抵边楼层的负 margin 被气泡圆角自然裁切 */
}

/* 富块通栏抵边（左右 -1rem 负 margin 抵消 segment padding 1rem） */
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-code-block),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table-wrap),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-mermaid-shell),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-sandbox-card),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-sandbox-skeleton),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-agent-embed-shell),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-math-block) {
  margin-left: -1rem;
  margin-right: -1rem;
  width: calc(100% + 2rem);
  max-width: calc(100% + 2rem);
  border-radius: 0;
  border-left: 0;
  border-right: 0;
  border-top: 1px solid var(--ecall-bubble-floor-border);
  border-bottom: 1px solid var(--ecall-bubble-floor-border);
  background: var(--ecall-bubble-floor-bg);
  box-shadow: none;
}

/* 代码块楼层细节：语言栏与代码区对齐气泡正文基准线 */
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-code-block) {
  --ecall-md-code-bg: var(--ecall-bubble-floor-bg);
  margin-top: 0.6rem;
  margin-bottom: 0.6rem;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-code-title) {
  padding: 0.35rem 1rem;
  background: transparent;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-code-body) {
  padding: 0.6rem 1rem 0.75rem 1rem;
}

/* 表格楼层细节：去外框，首尾列对齐气泡正文基准线 */
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table-wrap) {
  margin-top: 0.6rem;
  margin-bottom: 0.6rem;
  overflow-x: auto;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table) {
  border: 0 !important;
  border-radius: 0;
  background: transparent;
  margin: 0 !important;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table th),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table td) {
  background: transparent !important;
  border-bottom: 1px solid var(--ecall-bubble-floor-border);
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table th:first-child),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table td:first-child) {
  padding-left: 1rem;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table th:last-child),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table td:last-child) {
  padding-right: 1rem;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-table th) {
  background: color-mix(in srgb, var(--color-base-content) 3%, transparent) !important;
}

/* 图表、沙箱与公式楼层细节 */
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-mermaid-shell) {
  margin-top: 0.6rem;
  margin-bottom: 0.6rem;
  padding: 0.75rem 1rem;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-sandbox-card) {
  margin-top: 0.6rem;
  margin-bottom: 0.6rem;
  border: 0;
  border-top: 1px solid var(--ecall-bubble-floor-border);
  border-bottom: 1px solid var(--ecall-bubble-floor-border);
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-sandbox-toolbar),
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-sandbox-stage) {
  padding-left: 1rem;
  padding-right: 1rem;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-md-math-block) {
  margin-top: 0.6rem;
  margin-bottom: 0.6rem;
  padding: 0.6rem 1rem;
}

/* 楼层位于段首或段尾时紧贴边缘，圆角由气泡容器裁切 */
.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-markdown-content > :first-child:is(
  .ecall-md-code-block,
  .ecall-md-table-wrap,
  .ecall-md-mermaid-shell,
  .ecall-sandbox-card,
  .ecall-md-sandbox-skeleton,
  .ecall-agent-embed-shell,
  .ecall-md-math-block
)) {
  margin-top: -0.68rem;
  border-top: 0;
}

.ecall-assistant-bubble[data-bubble-background="on"] :deep(.ecall-markdown-content > :last-child:is(
  .ecall-md-code-block,
  .ecall-md-table-wrap,
  .ecall-md-mermaid-shell,
  .ecall-sandbox-card,
  .ecall-md-sandbox-skeleton,
  .ecall-agent-embed-shell,
  .ecall-md-math-block
)) {
  margin-bottom: -0.68rem;
  border-bottom: 0;
}

/* ==================== 2. 非气泡模式：富块为独立卡片（Card） ==================== */
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-code-block),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-table-wrap),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-mermaid-shell),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-sandbox-card),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-sandbox-skeleton),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-agent-embed-shell),
.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-math-block) {
  margin: 0.75rem 0;
  border-radius: var(--ecall-md-block-radius, 0.5rem);
  border: 1px solid color-mix(in srgb, var(--color-base-content) 12%, transparent);
  background: var(--color-base-100);
  overflow: hidden;
  box-shadow: none;
}

.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-code-block) {
  --ecall-md-code-bg: var(--color-base-100);
}

.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-table) {
  border: 0 !important;
  border-radius: inherit;
  margin: 0 !important;
}

.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-table th:first-child) {
  border-top-left-radius: inherit;
}

.ecall-assistant-bubble[data-bubble-background="off"] :deep(.ecall-md-table th:last-child) {
  border-top-right-radius: inherit;
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
