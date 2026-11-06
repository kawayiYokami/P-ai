<template>
  <div
    ref="chatLayoutRoot"
    class="relative flex h-full min-h-0 flex-row overflow-hidden"
  >
    <Transition
      name="chat-pane-left"
      :css="leftPaneTransitionCssEnabled"
      :appear="false"
      @before-enter="handleLeftPaneBeforeTransition"
      @after-enter="handleLeftPaneAfterTransition"
      @before-leave="handleLeftPaneBeforeTransition"
      @after-leave="handleLeftPaneAfterTransition"
    >
      <div
        v-if="showSideConversationList"
        :class="leftPaneLayoutSnapshot ? 'relative flex h-full min-h-0 shrink-0 overflow-hidden' : 'absolute bottom-0 left-0 top-0 z-50 flex h-full min-h-0 shadow-2xl overflow-hidden ecall-frost-solid'"
        :style="{ width: `${leftPaneVisibleWidth}px` }"
      >
        <ChatConversationSidebar
          class="min-w-0 flex-1"
          :items="conversationItems || unarchivedConversationItems"
          :active-conversation-id="activeConversationId"
          :user-alias="userAlias"
          :user-avatar-url="userAvatarUrl"
          :persona-name-map="personaNameMap"
          :persona-avatar-url-map="personaAvatarUrlMap"
          :active-tab="chatLeftPanelMode"
          :chat-model-options="chatModelOptions"
          :tool-review-api-config-id="toolReviewApiConfigId"
          :current-workspace-root-path="currentProjectWorkspaceRoot"
          @update:active-tab="$emit('update:conversation-list-tab', $event)"
          @edit-task="openTaskEditDialog"
          @select="handleConversationListSelect"
          @rename="handleConversationRename"
          @toggle-pin-conversation="handleConversationPinToggle"
          @archive-conversation="handleConversationArchive"
          @export-conversation="handleConversationExport"
          @delete-conversation="handleConversationDelete"
          @batch-archive-completed="handleBatchArchiveCompleted"
          @open-settings="$emit('openSettings')"
          @open-conversation-search="openConversationSearch"
        />
        <div
          class="ecall-pane-splitter absolute bottom-0 top-0 right-0 z-10 w-1 cursor-col-resize"
          :class="{ 'ecall-pane-splitter-active': activePaneResizeSide === 'left' }"
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-valuemin="PANE_WIDTH_LIMITS.left.min"
          :aria-valuemax="PANE_WIDTH_LIMITS.left.max"
          :aria-valuenow="leftPaneVisibleWidth"
          @pointerdown="startPaneResize('left', $event)"
          @keydown.left.prevent="adjustPaneWidthByKeyboard('left', -24)"
          @keydown.right.prevent="adjustPaneWidthByKeyboard('left', 24)"
        ></div>
      </div>
    </Transition>

    <div class="flex min-h-0 min-w-0 flex-1 overflow-hidden">
      <div
        data-chat-center-pane="true"
        class="relative flex min-h-0 min-w-0 flex-1 flex-col bg-base-200"
        :class="{ 'rounded-tl-2xl': leftPaneLayoutSnapshot }"
      >
        <div
          v-if="mediaDragActive && !chatting && !frozen && !conversationInteractionBusy"
          class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
        >
          <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
            {{ t("chat.dropImageOrPdf") }}
          </div>
        </div>

        <ConversationSearchPanel
          v-show="conversationSearchOpen"
          :open="conversationSearchOpen"
          :agent-options="conversationSearchAgentOptions"
          :workspace-options="conversationSearchWorkspaceOptions"
          :notice="conversationSearchNotice"
          @close="closeConversationSearch"
          @select="handleConversationSearchSelect"
        />

        <div class="relative flex min-h-0 flex-1 overflow-hidden">
          <div class="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden" @mouseenter="chatScrollbarRef?.reveal()" @mouseleave="chatScrollbarRef?.hide()">
          <Transition name="todo-bar-slide">
            <div
              v-if="showConversationTodoBar"
              class="pointer-events-none absolute inset-x-0 top-0 z-20 flex justify-center px-3 pt-0"
            >
              <ConversationTodoDropdown :todos="normalizedConversationTodos" :persona-name="personaName" />
            </div>
          </Transition>
          <div
            ref="scrollContainer"
            class="ecall-chat-scroll-container ecall-fade-bottom relative flex flex-1 min-h-0 flex-col overflow-x-hidden overflow-y-auto px-0"
            :class="[
              chatting || frozen || conversationInteractionBusy ? 'pointer-events-auto' : '',
              conversationRevealHidden
                ? 'opacity-0'
                : conversationRevealPhase === 1
                  ? 'ecall-conversation-reveal-a'
                  : 'ecall-conversation-reveal-b',
            ]"
            :data-chat-interaction-locked="chatting || frozen || conversationInteractionBusy ? 'true' : undefined"
            @scroll="handleConversationScroll"
            @wheel="handleConversationWheelInput"
            @pointerdown="beginPointerScrollIntent"
            @keydown="handleConversationKeydownInput"
          >
          <div ref="chatContentRoot" class="flex min-w-0 shrink-0 flex-col py-3">
          <DraftRecipientCard
            v-if="activeConversationIsDraft"
            :options="props.createConversationAgentOptions"
            :recent-options="draftRecentRecipientOptions"
            :selected-agent-id="draftSelectedAgentId"
            :avatar-url-map="props.personaAvatarUrlMap"
            :title="draftConversationTitle"
            :workspace-options="draftWorkspaceOptions"
            :workspace-root-path="stripExtendedPathPrefix(currentWorkspaceRootPath)"
            :workspace-access="currentWorkspaceAccess"
            :workspace-work-mode="props.currentWorkspaceWorkMode || 'directory'"
            :workspace-branch="props.currentWorkspaceBranch || ''"
            :workspace-worktree-path="props.currentWorkspaceWorktreePath || ''"
            :workspaces="props.workspaces"
            :workspace-autonomous-mode="Boolean(props.currentWorkspaceAutonomousMode)"
            :save-workspace="props.saveDraftWorkspaces ? handleDraftWorkspaceSaveLegacy : undefined"
            :save-workspaces="props.saveDraftWorkspaces ? handleDraftWorkspaceSave : undefined"
            :git-root-check="props.draftWorkspaceGitRootCheck"
            :sync-workspace-branch="props.syncWorkspaceBranch"
            @change="handleDraftPersonaChange($event)"
            @update:title="handleDraftTitleChange($event)"
          />
          <div class="ecall-chat-history-flow flex min-w-0 shrink-0 flex-col">
            <Virtualizer
              v-if="scrollContainer"
              :data="turnContainers"
              :shift="virtuaShift"
              ref="virtuaRef"
              :key="virtuaKey"
              :scroll-ref="(scrollContainer as unknown as HTMLElement)"
              class="min-w-0 w-full shrink-0"
            >
              <template #default="{ item: turn }">
                <div
                  :key="turn.id"
                  :data-turn-id="turn.id"
                  class="ecall-turn-container flex min-w-0 w-full shrink-0 flex-col"
                  :style="{
                    minHeight: turn.isLatest && turnViewportMinHeight > 0 ? `${turnViewportMinHeight}px` : undefined,
                  }"
                >
                  <div
                    v-for="item in turn.items"
                    :key="item.id"
                    class="w-full ecall-chat-virtual-item"
                  >
                    <div
                      v-if="item.kind === 'compaction'"
                      class="divider mt-4 mb-0 text-xs text-base-content/45"
                    >
                      <button type="button" class="btn btn-ghost btn-xs shrink-0 gap-1.5 px-2 text-base-content/60 hover:text-base-content"
                        :title="t('chat.viewSummary')" @click="openConversationSummary(item.block, $event)"
                        @contextmenu.prevent.stop="openCompactionSummaryContextMenu(item.block, $event)">
                        <AppIcon name="history" class="h-3.5 w-3.5" />
                        <span>{{ t("chat.viewSummary") }}</span>
                      </button>
                    </div>
                    <div v-else-if="item.kind === 'plan_started'" class="divider mt-4 mb-0 text-xs text-base-content/45">
                      <span class="shrink-0 rounded-full border border-base-300 bg-base-100 px-3 py-1 text-base-content/55">{{ t("chat.planStartedDivider") }}</span>
                    </div>
                    <div v-else-if="item.kind === 'message'"
                      v-memo="[...messageMemoKey(item.block, item.renderId, item.blockIndex, item.compactWithPrevious), agentNameMapSignature]">
                      <div class="ecall-elastic-item-shell">
                        <ChatMessageItem
                          :active-conversation-id="activeConversationId" :block="item.block"
                          :selection-key="item.renderId" :selection-mode-enabled="messageSelectionModeEnabled"
                          :selected="selectedMessageRenderIdSet.has(item.renderId)"
                          :chatting="chatting" :busy="conversationInteractionBusy" :frozen="frozen"
                          :user-alias="userAlias" :user-avatar-url="userAvatarUrl"
                          :persona-name-map="personaNameMap" :persona-avatar-url-map="personaAvatarUrlMap"
                          :agent-name-map="agentNameMap"
                          :markdown-is-dark="markdownIsDark"
                          :playing-audio-id="playingAudioId" :active-turn-user="false"
                          :compact-with-previous="item.compactWithPrevious"
                          :can-regenerate="showConversationActions && canRegenerateBlock(item.block, item.blockIndex)"
                          :can-confirm-plan="canConfirmPlan(item.block)"
                          :current-workspace-root-path="currentWorkspaceRootPath"
                          :current-theme="currentTheme"
                          :disable-recall-and-branch-actions="activeConversationIsSystemNotification"
                          @create-conversation-branch-from-turn="$emit('createConversationBranchFromTurn', $event)"
                          @recall-turn="$emit('recallTurn', $event)" @regenerate-turn="$emit('regenerateTurn', $event)"
                          @confirm-plan="$emit('confirmPlan', $event)" @enter-selection-mode="handleEnterMessageSelectionMode"
                          @toggle-message-selected="toggleMessageSelected"
                          @copy-message="handleCopyMessage"
                          @copy-message-image-done="handleCopyMessageImageDone"
                          @copy-message-image-failed="handleCopyMessageImageFailed"
                          @open-image-preview="openChatMessageImagePreview"
                          @toggle-audio-playback="toggleAudioPlayback($event.id, $event.audio)"
                          @assistant-link-click="handleAssistantLinkClick"
                          @activity-toggle="handleActivityToggle"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </template>
            </Virtualizer>
            <!-- 滚动区底部固定留白：悬浮操作区是一片 absolute 覆盖层，压在滚动视口下沿，
                 不占布局高度。留出这段空间，让它盖在空白上，而不是盖住最后一条消息 -->
            <div
              class="shrink-0"
              :style="{ height: `${CHAT_SCROLL_BOTTOM_RESERVE_PX}px` }"
              aria-hidden="true"
              data-chat-scroll-bottom-reserve="true"
            ></div>
          </div>
          </div>
          </div>
          <FloatingScrollbar ref="chatScrollbarRef" :target="scrollContainer" />
          </div>
        </div>
        <!-- 会话悬浮操作区：下排工作区 bar（贴底时出现），上排预览条 + 时间线按钮（离底时出现），两排各自动画进出 -->
        <div
          data-session-float-dock="true"
          class="pointer-events-none absolute inset-x-0 z-30"
          :style="sessionFloatDockStyle"
        >
        <div
          v-if="supportsFloatingSessionToolbar"
          ref="toolbarContainer"
          class="absolute inset-x-0 bottom-0 z-20 transition-opacity duration-150 ease-out"
          :class="sessionToolbarRowVisible
            ? 'pointer-events-auto opacity-100'
            : 'pointer-events-none opacity-0'"
          :aria-hidden="sessionToolbarRowVisible ? undefined : 'true'"
        >
          <div class="ecall-chat-toolbar-shell w-full px-4">
            <ChatWorkspaceToolbar
                  :chatting="chatting" :frozen="frozen" :conversation-busy="conversationInteractionBusy"
                  :workspace-button-label="t('chat.allowedWorkspaceButton')" :workspace-button-name="currentWorkspaceName"
                  :workspace-button-disabled="!activeConversationId || activeConversationSummary?.kind === 'remote_im_contact'"
                  :workspace-work-mode="currentWorkspaceWorkMode || 'directory'"
                  :workspace-permission-kind="currentWorkspacePermissionKind"
                  :auto-push-active="!!String(activeConversationSummary?.autoPushRemoteContactId || '').trim()"
                  :hide-menu-button="activeConversationSummary?.kind === 'remote_im_contact'"
                  :hide-workspace-button="hideWorkspaceButton || activeConversationSummary?.kind === 'remote_im_contact'"
              :show-task-create-menu-item="showConversationActions && !activeConversationIsRemoteContact && !activeConversationIsSystemNotification"
              :show-forward-menu-item="showConversationActions"
              :show-auto-push-menu-item="showConversationActions && !activeConversationIsRemoteContact && !activeConversationIsSystemNotification"
              :show-share-menu-item="showConversationActions"
              :show-open-in-browser-button="showOpenInBrowserButton && !activeConversationIsSystemNotification"
              :open-in-browser-disabled="!activeConversationId || activeConversationIsSystemNotification"
              :side-chat-enabled="sideChatPanelEnabled"
              :delegate-statuses="delegateStatuses"
              :running-task-count="runningTaskCount"
              :running-shell-count="runningShellCount"
              :mention-entries="mentionEntries"
              :selected-mentions="selectedMentions"
              :show-timeline-slot="canShowTimeline"
              @lock-workspace="$emit('lockWorkspace')" @open-branch-selection="openBranchSelectionMenu"
              @open-task-create="openTaskCreateDialog"
              @open-delegate-selection="openDelegateSelectionMenu" @open-forward-selection="openForwardSelectionMenu"
              @open-auto-push="openAutoPushCard"
              @open-share-selection="openShareSelectionMenu"
              @open-conversation-in-browser="openActiveConversationInBrowser"
              @open-run-summary="openRunSummaryPanel"
              @open-branch-from-current="openBranchFromCurrentMessage"
              @open-side-chat="selectChatRightPanelMode('sideChat')"
              @add-mention="$emit('addMention', $event)"
              @remove-mention="$emit('removeMention', $event)"
            />
          </div>
        </div>
        <!-- 上排：离底时出现（预览条 + 时间线按钮）。它压在操作条之上（DOM 顺序在后，同 z 即胜出）：
             时间线按钮两排共用一个，贴底时它仍要能点到，不能被操作条盖住 -->
        <div
          class="pointer-events-none absolute inset-x-0 bottom-0 z-20"
        >
        <div class="flex w-full items-end justify-between gap-2 px-4">
          <div class="pointer-events-none min-w-0 flex-1">
            <ChatThinkingPreviewBar
              :blocks="thinkingPreviewBlocks"
              :idle-text="idlePreviewText"
              :avatar-url="previewAvatarUrl"
              :visible="previewBarVisible"
              :streaming="chatting"
              @jump-to-bottom="handleJumpToBottomWithFollow"
            />
          </div>
          <!-- 右侧竖列：上=回到底部，下=时间线按钮。整体位移使时间线按钮与下排操作条里的那个重合 -->
          <div
            ref="sessionTopColumnRef"
            class="flex shrink-0 flex-col items-end gap-2"
            :style="sessionTopColumnOffsetStyle"
          >
            <button
              v-if="jumpToBottomButtonVisible"
              type="button"
              :class="[SESSION_FLOAT_FROST_CIRCLE, 'pointer-events-auto']"
              :aria-label="t('chat.jumpToBottom')"
              @click="handleJumpToBottomWithFollow"
            >
              <AppIcon name="arrow-down-to-line" class="h-4 w-4 shrink-0" :stroke-width="2.5" />
            </button>
            <!-- 时间线按钮：两排共用一个，常驻在这里，只随排切换底座、不随排重建。
                 离底那排悬停即在原位向上展开蛇形时间线；贴底时它就是操作条里的那个按钮，点一下直接开概览 -->
            <div ref="sessionTopTimelineBoxRef" class="relative flex h-10 shrink-0 items-center">
              <TimelineSnakeBoard
                ref="timelineSnakeBoardRef"
                :visible="timelineFloatPanelVisible"
                :anchors="timelineAnchors"
                :active-index="activeTimelineIndex"
                :hovered-index="hoveredTimelineIndex"
                :anchor-el="timelineFloatButtonRef"
                :preview-enabled="true"
                :preview-label="t('chat.timelinePreviewButtonLabel')"
                @hover="hoveredTimelineIndex = $event"
                @enter-zone="handleTimelineFloatEnter"
                @leave-zone="handleTimelineFloatLeave"
                @jump="handleTimelineJump"
                @preview="openTimelinePanel"
              />
              <button
                v-if="timelineEntryButtonVisible"
                ref="timelineFloatButtonRef"
                type="button"
                :class="[timelineEntryButtonClass, timelineFloatPanelVisible ? 'invisible' : 'pointer-events-auto']"
                :aria-label="t('chat.timelineButtonAria')"
                :aria-expanded="timelineFloatPanelVisible ? 'true' : 'false'"
                @mouseenter="handleTimelineFloatEnter"
                @mouseleave="handleTimelineButtonLeave"
                @click="handleTimelineEntryButtonClick"
              >
                <AppIcon name="route" class="h-4 w-4 shrink-0" />
              </button>
            </div>
          </div>
        </div>
        </div>
        </div>

        <!-- 会话时间线：覆盖整个窗口的弹层（Teleport 到 body），背景压黑，卡片居中且宽度上限 max-w-3xl -->
        <Teleport to="body">
        <Transition
          enter-active-class="transition duration-150 ease-out"
          enter-from-class="opacity-0"
          leave-active-class="transition duration-100 ease-in"
          leave-to-class="opacity-0"
        >
          <div
            v-if="timelinePanelOpen"
            class="fixed inset-0 z-[1000] flex items-center justify-center"
          >
            <!-- 遮罩：压黑 + 毛玻璃，背后聊天内容变暗且模糊；点它关闭面板 -->
            <div
              class="absolute inset-0 bg-black/45 backdrop-blur-md backdrop-saturate-150"
              @click="closeTimelinePanel"
            ></div>
            <!-- 卡片：不透明 base-100 底（遮罩已在后面做压黑与模糊，卡片自身不再叠一层），宽度限死上限，高度占 92%，居中 -->
            <div
              class="relative flex h-[92%] w-[92%] max-w-3xl flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
              @click.stop
            >
              <OverlayScrollArea ref="timelineScrollerRef" class="min-h-0 flex-1" scroller-class="h-full px-4 py-4">
              <ul class="timeline timeline-snap-icon max-md:timeline-compact timeline-vertical">
                <li v-for="(entry, index) in visibleTimelineEntries" :key="entry.id">
                  <hr v-if="index > 0" class="bg-base-300" />
                  <div class="timeline-middle">
                    <img
                      v-if="entry.avatarUrl"
                      :src="entry.avatarUrl"
                      alt=""
                      class="h-6 w-6 rounded-full object-cover transition-shadow"
                      :class="entry.index === activeTimelineIndex ? 'ring-2 ring-primary' : ''"
                    />
                    <span
                      v-else
                      class="block h-6 w-6 rounded-full bg-base-300 transition-shadow"
                      :class="entry.index === activeTimelineIndex ? 'ring-2 ring-primary' : ''"
                    ></span>
                  </div>
                  <button
                    type="button"
                    class="cursor-pointer rounded-box px-2 py-1 transition-colors hover:bg-base-200/70 hover:text-primary"
                    :class="entry.isOwn ? 'timeline-end text-start md:mb-10' : 'timeline-start mb-10 text-start md:text-end'"
                    @click="handleTimelineEntryJump(entry)"
                  >
                    <time v-if="entry.time" class="font-mono text-xs italic opacity-50">{{ entry.time }}</time>
                    <div class="text-sm text-base-content/70">{{ entry.speaker }}</div>
                    <div class="text-xs">
                      <InlineMarkdownText
                        :text="entry.text"
                        :limit="TIMELINE_TEXT_LIMIT"
                        :head-ratio="TIMELINE_TEXT_HEAD_RATIO"
                      />
                    </div>
                  </button>
                  <hr v-if="index < visibleTimelineEntries.length - 1" class="bg-base-300" />
                </li>
              </ul>
              </OverlayScrollArea>
              <!-- 底栏：说明 + 返回 -->
              <div class="flex shrink-0 items-center justify-between gap-2 px-3 py-2">
                <span class="flex items-center gap-1 text-xs text-base-content/60">
                  <AppIcon name="circle-alert" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  {{ t("chat.timelineJumpHint") }}
                </span>
                <button type="button" class="btn btn-sm gap-1" @click="closeTimelinePanel">
                  <AppIcon name="arrow-left" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  {{ t("chat.timelinePanelBack") }}
                </button>
              </div>
            </div>
          </div>
        </Transition>
        </Teleport>
        <CompactionSummaryCard
          :visible="conversationSummaryCard.visible"
          :text="conversationSummaryCard.text"
          :is-dark="markdownIsDark"
          @close="closeConversationSummaryCard"
        />
        <Teleport to="body">
        <ul
          v-if="compactionSummaryContextMenu"
          class="menu fixed z-[1200] w-44 rounded-box border border-base-300 bg-base-100 p-1 text-base-content shadow-xl"
          :style="{ left: `${compactionSummaryContextMenu.x}px`, top: `${compactionSummaryContextMenu.y}px` }"
          @contextmenu.prevent.stop
          @pointerdown.stop
        >
          <li>
            <button type="button" class="text-error" @click="recallCompactionSummaryFromContextMenu">
              <AppIcon name="undo" class="h-4 w-4" />
              <span>{{ t("chat.recall") }}</span>
            </button>
          </li>
        </ul>
        </Teleport>
        <ConversationAutoPushCard
          :open="autoPushCardOpen"
          :saving="autoPushSaving"
          :enabled="autoPushEnabled"
          :selected-contact-id="autoPushSelectedContactId"
          :options="autoPushContactOptions"
          @close="closeAutoPushCard"
          @save="saveAutoPushCard"
          @update:enabled="autoPushEnabled = $event"
          @update:selected-contact-id="autoPushSelectedContactId = $event"
        />
        <div
          ref="composerContainer"
          class="relative shrink-0 bg-base-200 p-0"
        >
          <div class="w-full">
          <div
            v-if="activeConversationIsRemoteContact"
            class="absolute bottom-full left-1/2 z-20 mb-3 -translate-x-1/2"
          >
            <RemoteImContactEnergyDashboard :snapshot="remoteImContactDashboardSnapshot" />
          </div>
          <Transition name="chat-status-banner">
            <div
              v-if="chatStatusBanner"
              class="pointer-events-none absolute inset-x-0 top-0 z-30 flex -translate-y-full justify-center px-2 pb-2 pt-0"
            >
              <div
                class="alert pointer-events-auto w-fit max-w-full px-4 py-2 text-sm shadow-sm"
                :class="chatStatusBanner.tone === 'error'
                  ? 'alert-error alert-soft'
                  : chatStatusBanner.tone === 'success'
                    ? 'alert-success alert-soft'
                  : chatStatusBanner.tone === 'info' || chatStatusBanner.text === t('chat.statusCompactingContext')
                    ? 'alert-info alert-soft'
                    : 'bg-base-200 text-base-content'"
              >
                <div class="flex w-full min-w-0 flex-col gap-2">
                  <div v-if="chatStatusBanner.tone === 'error'" class="flex items-center justify-between gap-2">
                    <span class="font-bold">{{ requestErrorTitle }}</span>
                    <div class="flex shrink-0 items-center gap-1">
                      <button
                        v-if="chatStorageRepairAvailable"
                        type="button"
                        class="btn btn-ghost btn-sm gap-1 text-error hover:bg-error/15"
                        :disabled="chatStorageRepairing"
                        @click="void repairChatStorage()"
                      >
                        <AppIcon name="wrench" class="h-3.5 w-3.5" />
                        <span>{{ chatStorageRepairing ? t("chat.storageRepairing") : t("chat.storageRepair") }}</span>
                      </button>
                      <button
                        type="button"
                        class="btn btn-ghost btn-sm gap-1 text-error hover:bg-error/15"
                        @click="void copyStatusText(chatStatusBanner.text)"
                      >
                        <AppIcon name="copy" class="h-3.5 w-3.5" />
                        <span>{{ t("common.copy") }}</span>
                      </button>
                      <button
                        type="button"
                        class="btn btn-ghost btn-sm gap-1 text-error hover:bg-error/15"
                        @click="$emit('clearChatError')"
                      >
                        <AppIcon name="close" class="h-3.5 w-3.5" />
                        <span>{{ t("common.close") }}</span>
                      </button>
                    </div>
                  </div>
                  <span
                    class="block max-h-32 min-w-0 overflow-y-auto whitespace-pre-wrap break-words text-center leading-5"
                    :class="chatStatusBanner.tone === 'error' || chatStatusBanner.tone === 'success'
                      ? ''
                      : 'text-base-content/80 ecall-shimmer-text ecall-reasoning-shimmer'"
                    :data-shimmer-text="chatStatusBanner.tone === 'error' || chatStatusBanner.tone === 'success' ? '' : chatStatusBanner.text"
                  >{{ chatStatusBanner.text }}</span>
                </div>
              </div>
            </div>
          </Transition>
          <ChatQuestionPanel
            v-if="conversationQuestionItems.length > 0"
            :key="conversationQuestionKey"
            :items="conversationQuestionItems"
            v-model="conversationQuestionAnswers"
            :submitting="conversationQuestionSubmitting"
            submit-mode="manual"
            class="px-4 pb-2 pt-1"
            @submit="handleConversationQuestionSubmit"
            @reject="handleConversationQuestionReject"
          />
          <ChatQuestionPanel
            v-if="activeConversationTerminalApprovals.length > 0"
            :key="activeConversationTerminalApprovals.map((a) => a.requestId).join(',')"
            :items="approvalQuestionItems"
            v-model="approvalQuestionAnswers"
            :submitting="!!terminalApprovalResolving"
            class="px-4 pb-4 pt-1"
            @submit="handleApprovalQuestionSubmit"
            @approve-for-workspace="handleApprovalQuestionWorkspaceRemember"
          />
          <div
            v-else-if="activeConversationRecipientMissing"
            class="m-4 rounded-box border border-warning/30 bg-warning/10 p-4 text-sm"
          >
            <div class="flex flex-col gap-4">
              <!-- 信息区：图标锚点 + 标题 + 说明，独立成块不被操作区挤压 -->
              <div class="flex items-start gap-3">
                <div
                  class="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-warning/15 text-warning"
                >
                  <AppIcon name="circle-alert" class="h-5 w-5" />
                </div>
                <div class="min-w-0">
                  <div class="font-semibold">{{ t("chat.recipientMissingTitle") }}</div>
                  <div class="mt-1 text-xs leading-relaxed opacity-70">
                    {{ t("chat.recipientMissingHint") }}
                  </div>
                </div>
              </div>
              <!-- 操作区：选择器占剩余宽度，按钮组固定 -->
              <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
                <div class="min-w-0 flex-1">
                  <AgentPersonaSelect
                    v-model:agent-id="repairRecipientAgentId"
                    :options="repairRecipientOptions"
                    :persona-avatar-url-map="props.personaAvatarUrlMap"
                    :show-model="false"
                    :auto-select-first="true"
                    :preserve-current="false"
                  />
                </div>
                <div class="flex shrink-0 items-center justify-between gap-2 sm:justify-end">
                  <button
                    type="button"
                    class="btn btn-sm btn-primary gap-2"
                    :disabled="conversationInteractionBusy || !repairRecipientSelectedOption"
                    @click="handleRebindConversationRecipient"
                  >
                    <AppIcon name="check" class="h-3.5 w-3.5" />
                    <span>{{ t("chat.recipientMissingApply") }}</span>
                  </button>
                  <button
                    type="button"
                    class="btn btn-sm btn-error btn-outline gap-2"
                    :disabled="conversationInteractionBusy"
                    @click="handleConversationDelete(activeConversationId)"
                  >
                    <AppIcon name="trash" class="h-3.5 w-3.5" />
                    <span>{{ t("common.delete") }}</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
          <ChatComposerPanel
            v-else ref="composerPanelRef" :selection-mode-enabled="messageSelectionModeEnabled"
            :composer-scope="composerScope"
            :selected-message-count="selectedMessageBlocks.length"
            :chat-input="chatInput" :instruction-presets="instructionPresets" :mention-entries="mentionEntries"
            :selected-mentions="selectedMentions"
            :clipboard-images="clipboardImages" :queued-attachment-notices="queuedAttachmentNotices"
            :link-open-error-text="linkOpenErrorText"
            :conversation-call-primary-api-config-id="conversationCallPrimaryApiConfigId"
            :preferred-chat-model-id="preferredChatModelId"
            :chat-model-options="chatModelOptions"
            :plan-mode-enabled="planModeEnabled"
            :wheelchair-mode-enabled="wheelchairModeEnabled"
            :wheelchair-suggestion="wheelchairSuggestion"
            :workspace-access="workspaceAccess"
            :frontend-round-phase="frontendRoundPhase" :chat-usage-percent="chatUsagePercent"
            :chatting="chatting" :busy="conversationInteractionBusy"
            :submit-pending="submitPending"
            :frozen="frozen"
            :goal-active="goalActive"
            :goal-title="goalButtonTitle"
            :goal-disabled="activeConversationSummary?.kind === 'remote_im_contact'"
            :system-notification-mode="activeConversationIsSystemNotification"
            :remote-contact-mode="activeConversationIsRemoteContact"
            :show-side-conversation-list="showSideConversationList"
            :active-conversation-id="activeConversationId" :unarchived-conversation-items="unarchivedConversationItems"
            :remote-im-contact-conversations="remoteImContactConversations"
            :user-alias="userAlias" :user-avatar-url="userAvatarUrl"
            :persona-name="personaName" :persona-name-map="personaNameMap"
            :default-create-conversation-agent-id="defaultCreateConversationAgentId"
            :ide-context-groups="mergedVisibleIdeContextGroups" :attached-ide-context-references="attachedIdeContextReferences"
            :kanban-issues="kanbanIssues" :attached-issue-references="kanbanIssueReferences"
            :current-theme="currentTheme"
            :show-conversation-actions="showConversationActions"
            :show-attach-action="props.showAttachAction"
            @update:chat-input="$emit('update:chatInput', $event)" @add-mention="$emit('addMention', $event)"
            @remove-mention="$emit('removeMention', $event)" @remove-clipboard-image="$emit('removeClipboardImage', $event)"
            @remove-queued-attachment-notice="$emit('removeQueuedAttachmentNotice', $event)"
            @pick-attachments="$emit('pickAttachments')"
            @update:conversation-preferred-api-config-id="$emit('update:conversationPreferredApiConfigId', $event)"
            @update:workspace-access="$emit('updateWorkspaceAccess', $event)"
            @update:plan-mode-enabled="$emit('update:planModeEnabled', $event)"
            @update:wheelchair-mode-enabled="$emit('update:wheelchairModeEnabled', $event)"
            @dismiss-wheelchair-suggestion="$emit('dismissWheelchairSuggestion')"
            @attach-ide-context-reference="handleAttachIdeContextReference"
            @remove-ide-context-reference="handleRemoveIdeContextReference"
            @attach-kanban-issue-reference="attachKanbanIssueReference"
            @remove-kanban-issue-reference="removeKanbanIssueReference"
            @send-chat="handleSendChat" @stop-chat="$emit('stopChat')"
            @open-delegate-selection="openDelegateSelectionMenu"
            @open-task-create="openTaskCreateDialog"
            @open-goal-task="$emit('openGoalTask')"
            @exit-selection-mode="handleExitMessageSelectionMode"
            @selection-action-copy="copySelectedMessages"
            @selection-action-branch="emitSelectionAction('branch')"
            @selection-action-forward="emitSelectionAction('forward', $event)"
            @selection-action-share="emitSelectionAction('share', $event)"
            @trim-conversation="$emit('trimConversation')" @open-conversation-list="$emit('openConversationList')" @open-settings="$emit('openSettings')"
            @create-conversation="$emit('createConversation', $event)"
          />
          </div>
        </div>

        <ChatImagePreviewDialog
          :open="imagePreviewOpen" :data-url="imagePreviewDataUrl" :zoom="imagePreviewZoom"
          :min-zoom="IMAGE_PREVIEW_MIN_ZOOM" :max-zoom="IMAGE_PREVIEW_MAX_ZOOM"
          :offset-x="previewOffsetX" :offset-y="previewOffsetY" :dragging="previewDragging" :rotation="imagePreviewRotation"
          :local-path="imagePreviewLocalPath"
          :copy-status="imagePreviewCopyStatus as any"
          :save-status="imagePreviewSaveStatus as any"
          @close="closeImagePreview" @zoom-in="zoomInPreview" @zoom-out="zoomOutPreview"
          @reset="resetPreviewZoom" @wheel="onPreviewWheel" @pointer-down="onPreviewPointerDown"
          @pointer-move="onPreviewPointerMove" @pointer-up="onPreviewPointerUp"
          @rotate="rotatePreviewClockwise"
          @copy-image="handleCopyLocalImage" @save-image="handleSaveLocalImage"
          @open-in-right-panel="openPreviewImageInRightPanel"
        />

        <ChatGoalTaskDialog
          :open="goalDialogOpen" :saving="goalSaving" :error-text="goalError"
          :active-task="activeGoalTask" :recent-history="recentGoalTaskHistory"
          @close="$emit('closeGoalTask')" @save="$emit('saveGoalTask', $event)"
          @stop="$emit('stopGoalTask')"
        />
        <ChatBranchGuardDialog
          :open="!!branchGuardPrompt"
          :repo-branch="branchGuardPrompt?.repoBranch || ''"
          :recorded-branch="branchGuardPrompt?.recordedBranch || ''"
          @confirm="handleBranchGuardConfirm"
          @cancel="handleBranchGuardCancel"
        />
        <ToolReviewTargetDialog
          v-if="showConversationActions"
          ref="launchDialogRef"
          :open="codeReviewDialogOpen"
          :initial-panel="launchPanel"
          :submitting="!!toolReviewSubmittingBatchKey"
          :delegating="launchDelegating"
          :error-text="codeReviewErrorText"
          :current-agent-id="props.activeAgentId"
          :current-api-config-id="launchDialogDefaultApiConfigId"
          :agent-options="props.createConversationAgentOptions"
          :api-configs="props.chatModelOptions"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :commit-options="commitOptions"
          :commit-options-loading="commitOptionsLoading"
          :commit-total="commitTotal"
          :commit-page="commitPage"
          :commit-page-size="commitPageSize"
          :commit-source-key="codeReviewCommitSourceKey"
          @close="closeCodeReviewDialog"
          @panel-change="writeLaunchPanel"
          @pick-commit-review="loadCodeReviewCommitOptions"
          @review-code="handleSubmitCodeReview"
          @delegate="handleLaunchDelegate"
        />
        <TaskCreateCard
          v-if="showConversationActions"
          :open="taskDialogOpen"
          :mode="taskDialogMode"
          :conversation-id="activeConversationId"
          :task="taskDialogTask"
          @close="closeTaskDialog"
          @created="handleTaskCreated"
          @updated="handleTaskUpdated"
        />
      </div>

    <div
      v-if="leftPaneOverlay || rightPaneOverlay"
      class="absolute inset-0 z-40 bg-base-300/20 backdrop-blur-[1px]"
      @click="closeOverlayPanes"
    ></div>

    <div
      v-if="collapsePreviewSide === 'left'"
      class="pointer-events-none absolute bottom-0 left-0 top-0 z-58 flex items-center justify-center border-r border-error/20 bg-error/12 backdrop-blur-[1px]"
      :style="{ width: `${collapsePreviewWidth}px` }"
    >
      <div class="rounded-full border border-error/25 bg-base-100/90 px-3 py-1.5 text-sm font-semibold text-error shadow-sm">
        {{ t("common.collapse") }}
      </div>
    </div>

    <div
      v-if="collapsePreviewSide === 'right'"
      class="pointer-events-none absolute bottom-0 right-0 top-0 z-58 flex items-center justify-center border-l border-error/20 bg-error/12 backdrop-blur-[1px]"
      :style="{ width: `${collapsePreviewWidth}px` }"
    >
      <div class="rounded-full border border-error/25 bg-base-100/90 px-3 py-1.5 text-sm font-semibold text-error shadow-sm">
        {{ t("common.collapse") }}
      </div>
    </div>

      <Transition
        name="chat-pane-right"
        :css="rightPaneTransitionCssEnabled"
        :appear="false"
        @before-enter="handleRightPaneBeforeTransition"
        @after-enter="handleRightPaneAfterTransition"
        @before-leave="handleRightPaneBeforeTransition"
        @after-leave="handleRightPaneAfterTransition"
      >
        <div v-if="effectiveToolReviewPanelOpen"
          :class="rightPaneLayoutSnapshot ? 'relative flex h-full min-h-0 shrink-0 border-l border-base-300 bg-base-100 overflow-hidden' : 'absolute bottom-0 right-0 top-0 z-50 flex h-full min-h-0 border-l border-base-300 bg-base-100 shadow-2xl overflow-hidden'"
          :style="{ width: `${rightPaneVisibleWidth}px` }">
          <div
            class="ecall-pane-splitter absolute bottom-0 top-0 left-0 z-10 w-1 cursor-col-resize"
          :class="{ 'ecall-pane-splitter-active': activePaneResizeSide === 'right' }"
          role="separator"
          tabindex="0"
          aria-orientation="vertical"
          :aria-valuemin="PANE_WIDTH_LIMITS.right.min"
          :aria-valuemax="PANE_WIDTH_LIMITS.right.max"
          :aria-valuenow="rightPaneVisibleWidth"
          @pointerdown="startPaneResize('right', $event)"
          @keydown.left.prevent="adjustPaneWidthByKeyboard('right', 24)"
          @keydown.right.prevent="adjustPaneWidthByKeyboard('right', -24)"
        ></div>
        <ChatHomePanel
          v-if="chatRightPanelMode === 'home'"
          class="h-full w-full"
          :conversation-id="activeConversationId"
          :latest-plan="latestHomePlan"
          :workspace-root-path="currentWorkspaceRootPath"
          :worktree-path="props.currentWorkspaceWorktreePath"
          :worktree-branch="props.currentWorkspaceBranch"
          :work-mode="props.currentWorkspaceWorkMode"
          :git-repo-root="homeGitRepoRoot"
          :branch="homeGitBranch"
          :change-count="homeGitChangeCount"
          :recent-commits="homeGitRecentCommits"
          :open-files="homeFilePreview.openFiles"
          :side-chat-enabled="Boolean(sideChatPanelEnabled)"
          :team-visible="Boolean(props.teamVisible)"
          :team-members="teamMembers"
          :team-member-avatar-url-map="props.personaAvatarUrlMap"
          :leader-name="personaName"
          :leader-avatar-url="props.personaAvatarUrlMap?.[props.activeAgentId] || ''"
          :persona-name-map="props.personaNameMap"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :delegates="delegateStatuses"
          :running-tasks="homePanelRunningTasks"
          :shells="backgroundShells"
          :tool-batches="toolReviewBatches"
          :kanban-visible="kanbanVisible"
          :kanban-issues="kanbanIssues"
          :kanban-approving-number="kanbanApprovingNumber"
          @open-kanban="openKanbanBoard"
          @approve-kanban="approveKanbanIssue"
          @reference-kanban-issue="attachKanbanIssueReference"
          @select-panel="selectChatRightPanelMode"
          @open-file="openHomeFile"
          @close-file="closeHomeFile"
          @open-side-chat-panel="emit('openSideChatPanel')"
          @open-team-member="emit('openTeamMember', $event)"
          @open-team-management="emit('openTeamManagement')"
          @open-workspace="openHomeWorkspaceDirectory"
          @open-git-changes="openHomeGitChanges"
          @open-git-commits="openHomeGitCommits"
          @branch-switched="(path) => props.syncWorkspaceBranch?.(path)"
          @git-error="handleHomeGitError"
          @open-monitor-tab="openMonitorTabFromHome"
          @interrupt-shell="interruptShell"
        />
        <FileReaderPanel
          v-else-if="chatRightPanelMode === 'reader'"
          ref="chatReaderPanelRef"
          class="ecall-panel-enter h-full w-full"
          :narrow-overlay="rightPaneOverlay"
          :initial-root-path="effectiveSessionRootPath"
          :session-root-path="effectiveSessionRootPath"
          :session-key="chatFileReaderSessionKey"
          :legacy-session-key="legacyChatFileReaderSessionKey"
          :enable-global-drop="false"
          :show-pick-file-button="false"
          bare-html-preview
          :show-tab-local-file-actions="true"
          :markdown-is-dark="markdownIsDark"
          custom-markstream-id="chat-file-reader-markstream"
          :sync-workspace-branch="props.syncWorkspaceBranch"
          @capture-context-reference="handleCaptureFileReaderContextReference"
          @add-context-reference="handleAddFileReaderContextReference"
          @clear-selection-context-reference="handleClearFileReaderSelectionContextReference"
          @clear-context-references="clearFileReaderContextReferences"
        >
          <template #tabLeadingActions>
            <ChatRightPanelSwitcher @select-home="selectChatRightPanelMode('home')" />
          </template>
          <template #empty>
            <div class="space-y-2 px-5 text-center">
              <div class="font-medium text-base-content/70">选择文件开始阅读</div>
              <div class="text-xs leading-relaxed text-base-content/50">右侧目录会跟随当前会话工作区，也可以通过文件标签页同时阅读多个文件。</div>
            </div>
          </template>
        </FileReaderPanel>
        <div v-else-if="chatRightPanelMode === 'sideChat'" class="ecall-panel-enter flex h-full min-h-0 w-full flex-col bg-base-200">
          <slot name="side-chat-panel" />
        </div>
        <div v-else-if="chatRightPanelMode === 'teamMember'" class="ecall-panel-enter flex h-full min-h-0 w-full flex-col bg-base-200">
          <slot name="team-member-panel" />
        </div>
        <div v-else-if="chatRightPanelMode === 'kanban'" class="ecall-panel-enter flex h-full min-h-0 w-full flex-col bg-base-100">
          <KanbanBoardPanel
            :issues="kanbanIssues"
            :loading="kanbanLoading"
            :error-text="kanbanErrorText"
            :approving-number="kanbanApprovingNumber"
            :leader-name="personaName"
            :leader-avatar-url="assistantAvatarUrl"
            :persona-avatar-url-map="personaAvatarUrlMap"
            :persona-name-map="personaNameMap"
            :team-members="teamMembers"
            @back="selectChatRightPanelMode('home')"
            @refresh="reloadKanbanBoard"
            @approve="approveKanbanIssue"
            @open-plan="openHomeFile"
            @reference="attachKanbanIssueReference"
          />
        </div>
        <div v-else-if="chatRightPanelMode === 'monitor'" class="ecall-panel-enter flex h-full min-h-0 w-full flex-col bg-base-200">
          <PanelTabStrip
            :tabs="monitorPanelTabs"
            :active-key="chatMonitorPanelMode"
            :aria-label="t('chat.monitorPanelTab')"
            @select-tab="selectMonitorPanelTab"
          >
            <template #leading>
              <ChatRightPanelSwitcher @select-home="selectChatRightPanelMode('home')" />
            </template>
          </PanelTabStrip>
          <ToolReviewSidebar class="min-h-0 flex-1"
            :active-tab="toolReviewSidebarActiveTab"
            :batches="toolReviewBatches" :current-batch-key="toolReviewCurrentBatchKey"
            :detail-map="toolReviewDetailMap" :segment-map="toolReviewSegmentMap"
            :detail-loading-call-id="toolReviewDetailLoadingCallId"
            :reviewing-call-id="toolReviewReviewingCallId" :batch-reviewing-key="toolReviewBatchReviewingKey"
            :error-text="toolReviewErrorText"
            :markdown-is-dark="markdownIsDark"
            :active-conversation-id="activeConversationId"
            :current-workspace-name="currentWorkspaceName" :current-workspace-root-path="currentWorkspaceRootPath"
            :workspaces="workspaces"
            :agent-options="toolReviewAgentOptions"
            :delegate-statuses="delegateStatuses"
            :delegate-statuses-error-text="delegateStatusesErrorText"
            :persona-avatar-url-map="personaAvatarUrlMap"
            @select-batch="setToolReviewCurrentBatchKey" @load-item-detail="loadToolReviewItemDetail"
            @review-item="runToolReviewForCall" @review-batch="runToolReviewForBatch"
            @open-delegate-detail="openDelegateArchiveDetail"
            @abort-delegate="abortDelegate"
            @assistant-link-click="handleAssistantLinkClick"
            @task-mutated="refreshRunningTaskCount"
          />
        </div>
        </div>
      </Transition>
    </div>

    <FileLinkContextMenu
      :state="fileLinkMenuState"
      @open-in-sidebar="handleFileLinkOpenInSidebar"
      @close="closeFileLinkMenu"
    />

  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, provide, reactive, ref, shallowRef, toRef, watch, watchEffect, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme, isVscodeHost } from "../../shell/composables/use-app-theme";
import {
  useChatComposerAppearance,
  visibleChatComposerContextGroups,
} from "../../shell/composables/use-chat-composer-appearance";
import AppIcon from "../../shared/components/AppIcon.vue";
import {
  copyTransportChatImageToClipboard,
  getTransportHostContext,
  invokeTauri,
  onTransportNotification,
  onTransportRecovered,
  openTransportExternalUrl,
  openTransportLocalDirectory,
  openTransportLocalFileReference,
  readTransportChatImage,
  resolveLocalFileUrl,
  saveTransportChatImageAs,
} from "../../../services/tauri-api";
import type { ApiConfigItem, AssistantStreamBlock, ChatConversationOverviewItem, ChatMentionEntry, ChatMentionTarget, ChatMessageBlock, ChatPersonaPresenceChip, ChatTodoItem, ConversationDelegateStatusSummary, ConversationForwardTarget, IdeContextReferenceItem, IdeContextWorkspaceGroup, PromptCommandPreset, RemoteImContactConversationOption, ShellWorkspace, ShellWorkMode } from "../../../types/app";
import ChatMessageItem from "../components/ChatMessageItem.vue";
import ChatQuestionPanel from "../components/ChatQuestionPanel.vue";
import ChatBranchGuardDialog from "../components/dialogs/ChatBranchGuardDialog.vue";
import type { BranchGuardCheck } from "../composables/use-chat-branch-guard";
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatThinkingPreviewBar from "../components/ChatThinkingPreviewBar.vue";
import TimelineSnakeBoard from "../components/TimelineSnakeBoard.vue";
import { SESSION_FLOAT_FROST_CIRCLE, SESSION_GHOST_CIRCLE } from "../components/session-float-styles";
import RemoteImContactEnergyDashboard from "../components/RemoteImContactEnergyDashboard.vue";
import AgentPersonaSelect from "../../shared/components/AgentPersonaSelect.vue";
import FileLinkContextMenu from "../../shared/components/FileLinkContextMenu.vue";
import { useFileLinkContextMenu } from "../../shared/composables/use-file-link-context-menu";
import DraftRecipientCard from "../components/DraftRecipientCard.vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import ChatConversationSidebar from "../components/ChatConversationSidebar.vue";
import ChatWorkspaceToolbar from "../components/ChatWorkspaceToolbar.vue";
import ToolReviewSidebar from "../components/ToolReviewSidebar.vue";
import ChatHomePanel from "../components/ChatHomePanel.vue";
import type { LatestPlanSummary } from "../components/chat-home/HomePlanCard.vue";
import ChatRightPanelSwitcher from "../components/ChatRightPanelSwitcher.vue";
import ToolReviewTargetDialog from "../components/ToolReviewTargetDialog.vue";
import FileReaderPanel from "../../file-reader/components/FileReaderPanel.vue";
import { useWorkspaceGitStatus } from "../../file-reader/composables/use-workspace-git-status";
import {
  listSessionFilePaths,
  readFileReaderSessionState,
  removeFilePathFromSessionState,
  writeFileReaderSessionState,
} from "../../file-reader/file-reader-session";
import { normalizePath } from "../../file-reader/utils";
import PanelTabStrip from "../../shared/components/PanelTabStrip.vue";
import ChatImagePreviewDialog from "../components/dialogs/ChatImagePreviewDialog.vue";
import ChatGoalTaskDialog from "../components/dialogs/ChatGoalTaskDialog.vue";
import TaskCreateCard from "../components/dialogs/TaskCreateCard.vue";
import ConversationTodoDropdown from "../components/ConversationTodoDropdown.vue";
import CompactionSummaryCard from "../components/CompactionSummaryCard.vue";
import ConversationAutoPushCard from "../components/ConversationAutoPushCard.vue";
import { useChatImagePreview } from "../composables/use-chat-image-preview";
import { useChatMessageActions } from "../composables/use-chat-message-actions";
import { useChatScrollLayout } from "../composables/use-chat-scroll-layout";
import { probeChatScroll } from "../composables/chat-scroll-probe";
import type { TerminalApprovalConversationItem } from "../../shell/composables/use-terminal-approval";
import { isAbsoluteLocalPath, isAssistantSpacePath, normalizeLocalLinkHref, parseLocalFileReference } from "../utils/local-link";
import { buildConversationSections, buildWorkspaceConversationSections, canonicalWorkspaceRootForComparison, type ConversationSection } from "../utils/conversation-sections";
import { defaultWorkspaceNameFromPath, normalizeWorkspacePathKey, stripExtendedPathPrefix } from "../../../utils/shell-workspaces";
import { recentWorkspacePaths } from "../../../utils/recent-workspaces";
import { type ChatRenderItem, isRightAlignedMessage, isCompactionBlock, canOpenInFileReader, fileExtensionFromPath } from "../utils/chat-render";
import { CHAT_HTML_READER_KEY, type ChatHtmlReaderRequest } from "../utils/chat-html-reader";
import { isSessionPersonaBlock as isSessionPersonaBlockShared, isUserBlock, TIMELINE_TOOL_BREAK, buildTimelineAnchors, type TimelineAnchor } from "../utils/timeline-anchors";
import { computeTimelineSegmentStarts, resolveTimelineVisibleStartIndex } from "../utils/timeline-segments";
import { computeSessionTopColumnOffset, isSessionTopColumnOffsetSettled } from "../utils/session-top-column-offset";
import { isSessionToolbarBehindElasticSpace } from "../utils/session-toolbar-visibility";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import { clearFileReaderContextCandidates } from "../utils/file-reader-context-tags";
import {
  buildConversationQuestionAnswerPayload,
  buildConversationQuestionItems,
  pruneConversationQuestionAnswers,
  type ConversationQuestionAnswerMap,
  type ConversationQuestionView,
} from "../utils/conversation-question";
import { useIdeContext } from "../composables/use-ide-context";
import { useDelegateStatus } from "../composables/use-delegate-status";
import { useBackgroundShell } from "../composables/use-background-shell";
import { useRemoteImContactDashboard } from "../composables/use-remote-im-contact-dashboard";
import { useChatVirtualList } from "../composables/use-chat-virtual-list";
import { Virtualizer } from "virtua/vue";
import { useChatPanes, PANE_WIDTH_LIMITS, type UseChatPanesOptions } from "../composables/use-chat-panes";
import { useChatSelection } from "../composables/use-chat-selection";
import { useChatConversationCtx, type ChatStatusBanner, type ChatStatusBannerTone } from "../composables/use-chat-conversation-ctx";
import { useChatScrollOrchestration } from "../composables/use-chat-scroll-orchestration";
import { useChatToolReviewHandlers } from "../composables/use-chat-tool-review-handlers";
import type { ToolReviewCodeReviewScope, ToolReviewCommitOption } from "../composables/use-chat-tool-review";
import type { ChatMonitorPanelMode, ChatRightPanelMode } from "../composables/chat-ui-layout-storage";
import { useChatBlockTracking } from "../composables/use-chat-block-tracking";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import type { AgentPersonaOption } from "../../shared/agent-persona-options";
import type { AppIconName } from "../../shared/icons/icon-map";
import { clearNativeTextSelection } from "../../../utils/native-selection";
import ConversationSearchPanel from "../components/ConversationSearchPanel.vue";
import KanbanBoardPanel from "../components/kanban/KanbanBoardPanel.vue";
import { useKanbanBoard } from "../composables/use-kanban-board";
import { useKanbanIssueReferences } from "../composables/use-kanban-issue-references";

// ==================== props / emits ====================

const props = defineProps<{
  composerScope?: "main" | "side";
  userAlias: string; personaName: string; userAvatarUrl: string; assistantAvatarUrl: string;
  personaNameMap: Record<string, string>; personaAvatarUrlMap: Record<string, string>;
  mentionEntries: ChatMentionEntry[]; selectedMentions: ChatMentionTarget[];
  latestUserText: string; latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  frontendRoundPhase: "idle" | "queued" | "waiting" | "streaming";
  submitPending?: boolean;
  chatErrorText: string; clipboardImages: Array<{ mime: string; bytesBase64: string; previewDataUrl?: string }>;
  queuedAttachmentNotices: Array<{ id: string; fileName: string; path: string; mime: string; pending?: boolean }>;
  chatInput: string; instructionPresets: PromptCommandPreset[];
  conversationCallPrimaryApiConfigId: string; preferredChatModelId?: string; toolReviewApiConfigId?: string; toolReviewRefreshTick: number; chatModelOptions: ApiConfigItem[];
  planModeEnabled: boolean; chatUsagePercent: number;
  wheelchairModeEnabled?: boolean;
  wheelchairSuggestion?: { type: "suggest" | "options"; text: string; options: string[] } | null;
  mediaDragActive: boolean; chatting: boolean; trimming: boolean; trimmingConversationId?: string;
  compactingConversation: boolean; compactingConversationId?: string;
  conversationBusy: boolean; frozen: boolean; messageBlocks: ChatMessageBlock[];
  hasMoreHistory: boolean; loadingOlderHistory: boolean;
  latestOwnMessageAlignRequest: number; conversationScrollToBottomRequest: number; scrollToBottomBehavior: "auto" | "smooth" | "own_top" | "manual" | "follow";
  currentWorkspaceName: string; currentWorkspaceDisplayName?: string; currentWorkspaceRootPath: string; workspaces: ShellWorkspace[];
  currentWorkspaceAutonomousMode?: boolean;
  currentWorkspaceWorkMode?: ShellWorkMode;
  currentWorkspaceBranch?: string;
  currentWorkspaceWorktreePath?: string;
  currentWorkspaceWorktreeExists?: boolean;
  /** 发送前的分支确认：主动回读工作目录分支，不一致时返回 prompt，由调用方拦住这次发送 */
  checkBranchBeforeSend?: () => Promise<BranchGuardCheck>;
  /** 「继续对话」：把会话记录改成当前仓库分支 */
  acceptBranchGuardPrompt?: () => void;
  configShellWorkspaces?: ShellWorkspace[];
  saveDraftWorkspaces?: (items: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, shellWorkBranch?: string, shellWorktreePath?: string) => Promise<void>;
  /** 会话内自己切分支成功后按工作目录同步「本会话工作分支」记录，避免下次发送时拦自己 */
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
  draftWorkspaceGitRootCheck?: (path: string) => Promise<boolean>;
  activeAgentId: string; activeConversationId: string; currentTodos: ChatTodoItem[];
  goalActive: boolean; goalTitle: string; goalDialogOpen: boolean;
  goalSaving: boolean; goalError: string;
  activeGoalTask: { taskId: string; goal: string; why: string; todo: string; endAtLocal: string; remainingHours: number } | null;
  recentGoalTaskHistory: Array<{ goal: string; why: string; todo: string; durationHours: number }>;
  currentTheme: string; unarchivedConversationItems: ChatConversationOverviewItem[];
  remoteImContactConversations: RemoteImContactConversationOption[];
  conversationItems?: ChatConversationOverviewItem[]; sideConversationListVisible: boolean;
  initialToolReviewPanelOpen: boolean;
  conversationListTab: "local" | "contact" | "task";
  chatLeftPanelMode: "local" | "contact" | "task";
  chatRightPanelMode: ChatRightPanelMode;
  chatMonitorPanelMode: ChatMonitorPanelMode;
  sideChatPanelEnabled?: boolean;
  teamVisible?: boolean;
  /** 首页队员状态表数据：字段形状对齐队员磁贴，头像由磁贴按 agentId 查下表 */
  teamMembers?: Array<{
    conversationId: string;
    title: string;
    /** 标题降级链用：会话标题 → 摘要标题 → 时间 → 人格名 */
    summaryTitle?: string;
    updatedAt?: string;
    lastMessageAt?: string;
    teamName?: string;
    agentId?: string;
    agentName?: string;
    runtimeState?: string;
    /** 他手上在办的那条议题；没有就退回 memberState */
    activeIssue?: { number: number; title: string; status: string };
    /** 运行时三态：busy / idle / error */
    memberState?: string;
  }>;
  /** 队员磁贴的头像来源：按 agentId 查，不在这里按人格名反查 */
  teamMemberAvatarUrlMap?: Record<string, string>;
  createConversationAgentOptions: AgentPersonaOption[];
  recipientOptionsReady?: boolean;
  defaultCreateConversationAgentId: string;
  ideContextGroups: IdeContextWorkspaceGroup[];
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
  hideConversationControlPanel?: boolean;
  /** 追问面板没有会话操作，但仍要能添加附件；不传则跟随会话操作开关。 */
  showAttachAction?: boolean;
  showOpenInBrowserButton?: boolean;
  systemNotificationMode?: boolean;
  hideWorkspaceButton?: boolean;
  workspaceAccess?: "approval" | "full_access" | "";
}>();

const emit = defineEmits<{
  (e: "update:chatInput", value: string): void;
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", value: string | { agentId: string }): void;
  (e: "sideConversationListVisibleChange", value: boolean): void;
  (e: "toolReviewPanelOpenChange", value: boolean): void;
  (e: "openChatReaderFile", path: string, line?: number): void;
  (e: "openChatReaderDirectory", path: string, line?: number): void;
  (e: "openSideChatPanel"): void;
  (e: "openTeamMember", conversationId: string): void;
  (e: "openTeamManagement"): void;
  (e: "sidePanelWidthsChange", value: { leftWidth: number; rightWidth: number }): void;
  (e: "sidePanelWidthsCommit", value: { leftWidth: number; rightWidth: number }): void;
  (e: "update:conversation-list-tab", value: "local" | "contact" | "task"): void;
  (e: "update:chatLeftPanelMode", value: "local" | "contact" | "task"): void;
  (e: "update:chatRightPanelMode", value: ChatRightPanelMode): void;
  (e: "update:chatMonitorPanelMode", value: ChatMonitorPanelMode): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "pickAttachments"): void;
  (e: "update:conversationPreferredApiConfigId", value: string): void;
  (e: "updateWorkspaceAccess", value: "approval" | "full_access"): void;
  (e: "update:planModeEnabled", value: boolean): void;
  (e: "update:wheelchairModeEnabled", value: boolean): void;
  (e: "dismissWheelchairSuggestion"): void;
  (e: "sendChat", payload?: { extraTextBlocks?: string[] }): void;
  (e: "stopChat"): void; (e: "trimConversation"): void; (e: "openConversationList"): void; (e: "openSettings"): void;
  (e: "clearChatError"): void;
  (e: "refreshConversationAfterStorageRepair"): void;
  (e: "createConversationBranchFromTurn", payload: { turnId: string }): void;
  (e: "branchConversationFromCurrent"): void;
  (e: "recallTurn", payload: { turnId: string }): void;
  (e: "regenerateTurn", payload: { turnId: string }): void;
  (e: "confirmPlan", payload: { messageId: string }): void;
  (e: "lockWorkspace"): void; (e: "openGoalTask"): void; (e: "openCodeReview"): void;
  (e: "closeGoalTask"): void;
  (e: "saveGoalTask", payload: { durationHours: number; goal: string; why: string; todo: string }): void;
  (e: "stopGoalTask"): void;
  (e: "taskCreated", task: TaskEntry): void;
  (e: "taskUpdated", task: TaskEntry): void;
  (e: "switchConversation", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact" | "acp_session"; remoteContactId?: string }): void;
  (e: "renameConversation", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
  (e: "archiveConversation", conversationId: string): void;
  (e: "exportConversation", conversationId: string): void;
  (e: "deleteConversation", conversationId: string): void;
  (e: "rebindConversationRecipient", payload: { conversationId: string; agentId: string }): void;
  (e: "updateDraftConversation", payload: { conversationId: string; agentId?: string; preferredApiConfigId?: string | null; title?: string | null }): void;
  (e: "createConversation", input?: { title?: string; agentId?: string; copyCurrent?: boolean; importPath?: string; shellWorkspaces?: ShellWorkspace[]; shellWorkMode?: ShellWorkMode; shellAutonomousMode?: boolean }): void;
  (e: "loadOlderHistory"): void; (e: "loadOlderCompactionSegment"): void; (e: "reachedBottom"): void;
  (e: "jumpToConversationBottom"): void;
  (e: "refreshToolReviewMessage", payload: { conversationId: string; messageId: string }): void;
  (e: "selectionActionCopy", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string }): void;
  (e: "selectionActionCopyError", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; error: string }): void;
  (e: "selectionActionBranch", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string }): void;
  (e: "selectionActionForward", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; target: ConversationForwardTarget }): void;
  (e: "selectionActionDelegate", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; agentId: string; presetId: string; why: string; goal: string; todo: string; apiConfigId?: string }): void;
  (e: "selectionActionShare", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; exportFormat?: "html" | "png" | "copyPng" }): void;
  (e: "approveTerminalApproval", requestId: string, reason?: string): void;
  (e: "denyTerminalApproval", requestId: string, reason?: string): void;
  (e: "approveTerminalApprovalForSession", requestId: string): void;
  (e: "approveTerminalApprovalForWorkspace", requestId: string): void;
}>();

// ==================== basic state ====================

const { t, locale } = useI18n();
const chatReaderPanelRef = ref<InstanceType<typeof FileReaderPanel> | null>(null);
const chatScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
const linkOpenErrorText = ref("");
const conversationSummaryCard = ref<{ visible: boolean; text: string }>({ visible: false, text: "" });
const compactionSummaryContextMenu = ref<{ x: number; y: number; block: ChatMessageBlock } | null>(null);
const composerPanelRef = ref<{ focusInput: (opts?: FocusOptions) => void } | null>(null);
const taskDialogOpen = ref(false);
const taskDialogMode = ref<"create" | "edit">("create");
const taskDialogTask = ref<TaskEntry | null>(null);
const autoPushCardOpen = ref(false);
const autoPushSaving = ref(false);
const autoPushEnabled = ref(false);
const autoPushSelectedContactId = ref("");
const autoPushContactOptions = computed(() =>
  props.remoteImContactConversations.filter((item) => item.channelEnabled !== false),
);

type WebAccessInfo = {
  running: boolean;
  enabled: boolean;
  localUrl: string;
};

const codeReviewDialogOpen = ref(false);
const LAUNCH_PANEL_STORAGE_KEY = "easy_call.tool_launch_panel.v1";
/** 记住上一次选择的入口面板，下次打开沿用；首次默认自定义委托 */
function readLaunchPanel(): "delegate" | "review" {
  try {
    return window.localStorage.getItem(LAUNCH_PANEL_STORAGE_KEY) === "review" ? "review" : "delegate";
  } catch {
    return "delegate";
  }
}
function writeLaunchPanel(value: "delegate" | "review") {
  try {
    window.localStorage.setItem(LAUNCH_PANEL_STORAGE_KEY, value);
  } catch {
    // 记不住不影响本次使用
  }
}
const launchPanel = ref<"delegate" | "review">(readLaunchPanel());
const launchDelegating = ref(false);
const launchDialogRef = ref<InstanceType<typeof ToolReviewTargetDialog> | null>(null);
const codeReviewErrorText = ref("");
/** 对话卡片默认模型 = 当前会话模型：优先会话显式首选，回退本次调用主模型 */
const launchDialogDefaultApiConfigId = computed(() =>
  String(props.preferredChatModelId || "").trim()
  || String(props.conversationCallPrimaryApiConfigId || "").trim(),
);
const commitOptions = ref<ToolReviewCommitOption[]>([]);
const commitOptionsLoading = ref(false);
const commitTotal = ref(0);
const commitPage = ref(1);
const commitPageSize = ref(5);

type ToolReviewSidebarTab = "tools" | "delegates" | "tasks" | "fastRequests";

const monitorPanelTabs = computed<Array<{ key: ChatMonitorPanelMode; label: string; icon: AppIconName; closeable: false }>>(() => [
  { key: "delegate", label: t("chat.toolReview.delegatesTab"), icon: "network", closeable: false },
  { key: "tasks", label: t("chat.toolReview.tasksTab"), icon: "list-todo", closeable: false },
  { key: "tools", label: t("chat.toolReview.toolsTab"), icon: "wrench", closeable: false },
  { key: "fastRequests", label: t("chat.toolReview.overviewOthers"), icon: "inbox", closeable: false },
]);

const toolReviewSidebarActiveTab = computed<ToolReviewSidebarTab>(() => {
  if (props.chatMonitorPanelMode === "tools") return "tools";
  if (props.chatMonitorPanelMode === "tasks") return "tasks";
  if (props.chatMonitorPanelMode === "fastRequests") return "fastRequests";
  return "delegates";
});
// ==================== messages / audio ====================

const { playingAudioId, copyMessage, stopAudioPlayback, toggleAudioPlayback } = useChatMessageActions();
const transientNotice = ref<ChatStatusBanner | null>(null);
let transientNoticeTimer = 0;

function showTransientNotice(text: string, tone: ChatStatusBannerTone = "success") {
  const next = String(text || "").trim();
  if (!next) return;
  transientNotice.value = { text: next, tone };
  if (transientNoticeTimer) window.clearTimeout(transientNoticeTimer);
  transientNoticeTimer = window.setTimeout(() => {
    transientNotice.value = null;
    transientNoticeTimer = 0;
  }, 2200);
}

// ==================== 聊天索引自修复 ====================
const chatStorageRepairAvailable = ref(false);
const chatStorageRepairing = ref(false);
let chatStorageProbeToken = 0;

/**
 * 报错卡片一出现就探一次该会话的索引能否读出。
 * 探测只读，不改库，也不改会话状态。
 */
async function probeChatStorageHealth(): Promise<void> {
  // 令牌必须先失效再看早退：否则上一次探测在途时错误文案被清空，
  // 它的结果回来仍会点亮按钮，把一个已经不存在的损坏重新摆到界面上。
  const token = ++chatStorageProbeToken;
  const errorText = String(props.chatErrorText || "").trim();
  const conversationId = String(props.activeConversationId || "").trim();
  if (!errorText || !conversationId) {
    chatStorageRepairAvailable.value = false;
    return;
  }
  try {
    const health = await invokeTauri<{ corrupted: boolean }>("conversation.storageHealth", {
      input: { conversationId },
    });
    if (token !== chatStorageProbeToken) return;
    chatStorageRepairAvailable.value = !!health?.corrupted;
  } catch (error) {
    if (token !== chatStorageProbeToken) return;
    chatStorageRepairAvailable.value = false;
    console.warn("[聊天索引修复] 探活失败", { conversationId, error });
  }
}

async function repairChatStorage(): Promise<void> {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || chatStorageRepairing.value) return;
  chatStorageRepairing.value = true;
  try {
    const outcome = await invokeTauri<{
      rebuiltConversationIds: string[];
      rebuiltMessageCount: number;
      failedConversations: Array<{ conversationId: string; detail: string }>;
    }>("conversation.repairStorage", { input: { conversationId } });
    const failed = Array.isArray(outcome?.failedConversations) ? outcome.failedConversations : [];
    const thisConversationFailed = failed.some(
      (item) => String(item?.conversationId || "").trim() === conversationId,
    );
    // 修复耗时期间用户可能已经切走，这时清错误、重读都会作用到别的会话上。
    if (String(props.activeConversationId || "").trim() !== conversationId) {
      chatStorageRepairAvailable.value = false;
      return;
    }
    if (thisConversationFailed) {
      // 这个会话没救回来：保留按钮让用户能重试，不要清掉它的错误。
      showTransientNotice(
        t("chat.storageRepairFailed", {
          error: String(failed.find((item) => String(item?.conversationId || "").trim() === conversationId)?.detail || ""),
        }),
        "error",
      );
      return;
    }
    chatStorageRepairAvailable.value = false;
    emit("clearChatError");
    // 修好的库要重新读一遍，否则报错卡片消失了、内容还是旧的。
    emit("refreshConversationAfterStorageRepair");
    if (failed.length > 0) {
      showTransientNotice(
        t("chat.storageRepairPartial", { count: failed.length, error: failed[0]?.detail || "" }),
        "error",
      );
    } else {
      showTransientNotice(t("chat.storageRepairDone", { count: Number(outcome?.rebuiltMessageCount || 0) }));
    }
  } catch (error) {
    console.warn("[聊天索引修复] 失败", { conversationId, error });
    if (String(props.activeConversationId || "").trim() === conversationId) {
      showTransientNotice(
        t("chat.storageRepairFailed", { error: String((error as Error)?.message || error || "") }),
        "error",
      );
    }
  } finally {
    chatStorageRepairing.value = false;
  }
}

watch(
  () => [String(props.chatErrorText || "").trim(), String(props.activeConversationId || "").trim()] as const,
  () => void probeChatStorageHealth(),
  { immediate: true },
);

async function copyStatusText(text: string) {
  const content = String(text || "").trim();
  if (!content) return;
  try {
    await navigator.clipboard.writeText(content);
  } catch (error) {
    console.warn("[状态提示] 复制失败", error);
  }
}

async function handleCopyMessage(block: ChatMessageBlock) {
  const ok = await copyMessage(block);
  if (ok) {
    showTransientNotice(t("chat.copyDone"), "success");
    return;
  }
  showTransientNotice(t("chat.copyFailed"), "error");
}

function handleCopyMessageImageDone() {
  showTransientNotice(t("chat.copyImageDone"), "success");
}

function handleCopyMessageImageFailed() {
  showTransientNotice(t("chat.copyFailed"), "error");
}

function handleSelectionCopyDone(count: number) {
  showTransientNotice(t("chat.selection.copied", { count }), "success");
}

function handleSelectionCopyFailed() {
  showTransientNotice(t("chat.copyFailed"), "error");
}

// ==================== context computed ====================

const {
  markdownIsDark, normalizedConversationTodos,
  activeConversationSummary, isCurrentConversationCompacting,
  activeConversationTerminalApprovals, goalButtonTitle,
  isOrganizingContextBusy, chatStatusBanner: baseChatStatusBanner,
  latestPendingPlanMessageId,
} = useChatConversationCtx(props, isDarkAppTheme, t);

// 提问卡：终端审批转 QuestionItems，一次性提交
const approvalQuestionAnswers = ref<Record<string, { optionId: string; label: string; comment: string }>>({});

function getApprovalPersonaName(item: TerminalApprovalConversationItem): string {
  const convId = String(item.conversationId || item.sessionId || props.activeConversationId || "").trim();
  const conv = (props.conversationItems || props.unarchivedConversationItems || []).find(
    (c) => String(c.conversationId || "").trim() === convId,
  ) || activeConversationSummary.value;
  const agentId = conv?.agentId || props.activeAgentId;
  if (agentId && props.personaNameMap?.[agentId]) {
    return props.personaNameMap[agentId];
  }
  return String(props.personaName || "").trim() || t("archives.roleAssistant") || "助理";
}

const approvalQuestionItems = computed(() => {
  return activeConversationTerminalApprovals.value.map((item) => {
    const persona = getApprovalPersonaName(item);
    const des = String(item.description || item.summary || "").trim();
    let title = "";
    let desc: string | undefined = undefined;

    if (des) {
      title = `${persona}想要${des}`;
      const reason = String(item.reason || "").trim();
      if (reason && reason !== des) {
        desc = reason;
      }
    } else {
      const fallbackSummary = String(item.toolName || item.approvalKind || "").trim();
      title = fallbackSummary ? `${persona}想要${fallbackSummary}` : (t("chat.toolReview.title") || "终端审批");
      desc = String(item.message || item.reason || "").trim() || undefined;
    }

    const cmd = String(item.command || "").trim();
    const rawPreview = String(item.callPreview || "").trim();
    let preview = rawPreview || cmd;
    // 保证命令始终可见：callPreview 为 raw_json/评估意见时，命令另起一行拼接
    if (rawPreview && cmd && rawPreview !== cmd && !rawPreview.includes(cmd)) {
      preview = `${rawPreview}\n\n$ ${cmd}`;
    }
    const workspaceLabel = String(item.workspaceName || item.workspacePath || "").trim();
    return {
      id: item.requestId,
      title,
      description: desc,
      previewText: preview || undefined,
      canRememberWorkspace: !!item.canRememberWorkspace,
      workspaceLabel: workspaceLabel || undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" as const },
        { id: "deny", label: "拒绝", kind: "withInput" as const, placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    };
  });
});
watch(() => activeConversationTerminalApprovals.value.map((a) => a.requestId).join(","), () => {
  approvalQuestionAnswers.value = {};
});
function handleApprovalQuestionSubmit(answers: Array<{ id: string; optionId: string; label: string; comment: string }>) {
  for (const ans of answers) {
    const reason = String(ans.comment ?? "").trim() || undefined;
    if (ans.optionId === "deny") emit("denyTerminalApproval", ans.id, reason);
    else emit("approveTerminalApproval", ans.id, reason);
  }
}
function handleApprovalQuestionWorkspaceRemember(requestId: string) {
  const normalized = String(requestId || "").trim();
  if (!normalized) return;
  emit("approveTerminalApprovalForWorkspace", normalized);
}

// ==================== 会话问题队列（队长通过 question 工具写入） ====================
const conversationQuestions = ref<ConversationQuestionView[]>([]);
const conversationQuestionAnswers = ref<ConversationQuestionAnswerMap>({});
const conversationQuestionSubmitting = ref(false);
let unlistenConversationQuestions: (() => void) | undefined;

/**
 * 面板只按会话重建：同一会话里发掉一题后列表会变，但面板不能重建——
 * 重建会丢掉用户的当前题位置与已选待发的勾选。
 * 题目减少后的落点由面板自己处理（发送后停在剩下的第一题）。
 * 选中态不会残留：每次加载与提交回执都会清空 `conversationQuestionAnswers`。
 */
const conversationQuestionKey = computed(() => String(props.activeConversationId ?? ""));

const conversationQuestionItems = computed(() =>
  buildConversationQuestionItems(conversationQuestions.value),
);

/**
 * 用后端返回的题目列表刷新面板。
 *
 * `resetAnswers` 只给「切会话」与「发送成功」用：前者已选内容全部失效，后者已经发出去了。
 * 拒绝单题、以及工具自行增删题走的是保留分支——只丢已经不在列表里的题目勾选，
 * 其余题的选中态留着，否则用户拒一题就得把别的题重选一遍。
 * 编号不回收也不复用（后端有测试盯着），所以按编号对齐是安全的。
 */
function applyConversationQuestionItems(
  items: ConversationQuestionView[] | undefined,
  resetAnswers: boolean,
) {
  const next = Array.isArray(items) ? items : [];
  conversationQuestions.value = next;
  conversationQuestionAnswers.value = resetAnswers
    ? {}
    : pruneConversationQuestionAnswers(next, conversationQuestionAnswers.value);
}

async function loadConversationQuestions(conversationId?: string) {
  const cid = String(conversationId ?? props.activeConversationId ?? "").trim();
  if (!cid) {
    conversationQuestions.value = [];
    return;
  }
  try {
    const items = await invokeTauri<ConversationQuestionView[]>("question_list", {
      conversationId: cid,
    });
    // 请求返回时用户可能已切走，丢弃过期结果。
    if (String(props.activeConversationId || "").trim() !== cid) return;
    applyConversationQuestionItems(items, true);
  } catch {
    conversationQuestions.value = [];
  }
}

function handleConversationQuestionSubmit(
  answers: Array<{ id: string; optionId: string; label: string; comment: string; customText?: string }>,
) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || conversationQuestionSubmitting.value) return;
  const payload = buildConversationQuestionAnswerPayload(answers);
  if (payload.length === 0) return;
  conversationQuestionSubmitting.value = true;
  void invokeTauri<{ remaining: ConversationQuestionView[] }>("question_answer", {
    conversationId,
    answers: payload,
  })
    .then((result) => {
      if (String(props.activeConversationId || "").trim() !== conversationId) return;
      applyConversationQuestionItems(result?.remaining, true);
    })
    .catch((error) => {
      console.error("[会话问题] 提交答复失败", error);
      void loadConversationQuestions(conversationId);
    })
    .finally(() => {
      conversationQuestionSubmitting.value = false;
    });
}

/**
 * 拒绝回答某一题：只把这一题从问题列表里删掉，不产生任何会话消息、不触发调度。
 * 与答复共用 `conversationQuestionSubmitting`，同一时刻只允许一个问卷操作在飞。
 */
function handleConversationQuestionReject(number: number) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || conversationQuestionSubmitting.value) return;
  if (!Number.isInteger(number) || number < 1) return;
  conversationQuestionSubmitting.value = true;
  void invokeTauri<ConversationQuestionView[]>("question_reject", {
    conversationId,
    number,
  })
    .then((remaining) => {
      if (String(props.activeConversationId || "").trim() !== conversationId) return;
      applyConversationQuestionItems(remaining, false);
    })
    .catch((error) => {
      console.error("[会话问题] 拒绝回答失败", error);
      void loadConversationQuestions(conversationId);
    })
    .finally(() => {
      conversationQuestionSubmitting.value = false;
    });
}

watch(
  () => String(props.activeConversationId || "").trim(),
  (conversationId) => {
    void loadConversationQuestions(conversationId);
  },
  { immediate: true },
);

onMounted(() => {
  unlistenConversationQuestions = onTransportNotification<Record<string, unknown>>(
    "question.itemsUpdated",
    (payload) => {
      const conversationId = String((payload as any)?.conversationId || "").trim();
      if (!conversationId || conversationId !== String(props.activeConversationId || "").trim()) return;
      applyConversationQuestionItems((payload as any)?.items, false);
    },
  );
});

onBeforeUnmount(() => {
  unlistenConversationQuestions?.();
  unlistenConversationQuestions = undefined;
});
const chatStatusBanner = computed(() => {
  // 修复期间独占该库，所有会话调度必须让路，这里给一条常驻提示。
  if (chatStorageRepairing.value) {
    return { text: t("chat.storageRepairPaused"), tone: "info" as ChatStatusBannerTone };
  }
  if (transientNotice.value) return transientNotice.value;
  return baseChatStatusBanner.value;
});
const requestErrorTitle = computed(() => {
  const title = t("status.errorTitleRequest");
  return title === "status.errorTitleRequest" ? "请求发生错误" : title;
});
const conversationInteractionBusy = computed(() =>
  props.conversationBusy || isOrganizingContextBusy.value || chatStorageRepairing.value,
);
const activeConversationIsSystemNotification = computed(() =>
  !!props.systemNotificationMode || !!activeConversationSummary.value?.isSystemNotificationConversation,
);
const activeConversationIsRemoteContact = computed(() =>
  activeConversationSummary.value?.kind === 'remote_im_contact',
);

// ==================== 会话草稿：历史区人格选择卡 ====================

// 草稿判定：overview 标记为草稿即显示选择卡。
// 转正由后端写回存储 is_draft=false 并推送 overview 水位线，前端收到后自动消失。
// 但「用户按下回车发出消息」的那一刻前端就要立刻转正，不等后端水位线：
// 本地记录已发送消息的会话 id，该会话即使 overview 仍标记 isDraft=true 也不再显示选择卡。
const locallyPromotedConversationId = ref("");
const activeConversationIsDraft = computed(() => {
  const conversationId = String(props.activeConversationId || "").trim();
  const locallyPromoted =
    !!locallyPromotedConversationId.value &&
    locallyPromotedConversationId.value === conversationId;
  return !!activeConversationSummary.value?.isDraft && !locallyPromoted;
});
const draftSelectedAgentId = ref("");

// 草稿卡自定义标题：默认显示会话原标题（可能为空），用户修改后写入草稿字段
const draftConversationTitle = computed(() =>
  String(activeConversationSummary.value?.title || "").trim(),
);

const DRAFT_RECENT_RECIPIENT_LIMIT = 6;

const draftRecentRecipientOptions = computed<AgentPersonaOption[]>(() => {
  const allOptions = Array.isArray(props.createConversationAgentOptions)
    ? props.createConversationAgentOptions
    : [];
  const activeConversationId = String(props.activeConversationId || "").trim();
  const items = [...(props.unarchivedConversationItems || [])].sort((a, b) => {
    const timeOf = (item: ChatConversationOverviewItem) =>
      String(item.lastMessageAt || item.updatedAt || "").trim();
    return timeOf(b).localeCompare(timeOf(a));
  });
  const seen = new Set<string>();
  const recents: AgentPersonaOption[] = [];
  for (const item of items) {
    const agentId = String(item.agentId || "").trim();
    if (!agentId) continue;
    if (String(item.conversationId || "").trim() === activeConversationId) continue;
    // 按人格去重：每个目标人格各占一个行星卡片
    if (seen.has(agentId)) continue;
    seen.add(agentId);
    const option = allOptions.find((candidate) =>
      String(candidate.agentId || "").trim() === agentId
      && !candidate.unavailable
      && !candidate.personaMissing
    );
    if (!option) continue;
    recents.push(option);
    if (recents.length >= DRAFT_RECENT_RECIPIENT_LIMIT) break;
  }
  return recents;
});

watch(
  [activeConversationIsDraft, () => props.activeConversationId],
  ([isDraft]) => {
    if (!isDraft) return;
    draftSelectedAgentId.value = String(activeConversationSummary.value?.agentId || "").trim();
  },
  { immediate: true },
);

function handleDraftPersonaChange(payload: { agentId: string }) {
  draftSelectedAgentId.value = payload.agentId;
  emit("updateDraftConversation", {
    conversationId: String(props.activeConversationId || "").trim(),
    agentId: payload.agentId,
  });
}

function handleDraftTitleChange(title: string) {
  emit("updateDraftConversation", {
    conversationId: String(props.activeConversationId || "").trim(),
    title: String(title || "").trim() || null,
  });
}
const remoteImContactDashboardContactId = computed(() =>
  activeConversationIsRemoteContact.value
    ? String(activeConversationSummary.value?.remoteContactId || "").trim()
    : "",
);
const { snapshot: remoteImContactDashboardSnapshot } = useRemoteImContactDashboard({
  contactId: remoteImContactDashboardContactId,
  enabled: activeConversationIsRemoteContact,
});
const repairRecipientAgentId = ref("");
const repairRecipientOptions = computed(() =>
  (Array.isArray(props.createConversationAgentOptions) ? props.createConversationAgentOptions : [])
    .filter((option) =>
      !option.unavailable
      && !!String(option.agentId || "").trim()
    ),
);

function findRecipientOption(agentId: string): AgentPersonaOption | null {
  const normalizedAgentId = String(agentId || "").trim();
  if (!normalizedAgentId) return null;
  return repairRecipientOptions.value.find((option) =>
    String(option.agentId || "").trim() === normalizedAgentId
    && !option.personaMissing
  ) || null;
}

const activeConversationRecipientMissing = computed(() => {
  if (!props.recipientOptionsReady) return false;
  if (activeConversationIsSystemNotification.value || activeConversationIsRemoteContact.value) return false;
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return false;
  const summary = activeConversationSummary.value;
  if (!summary) return false;
  const agentId = String(summary.agentId || "").trim();
  return !findRecipientOption(agentId);
});
const repairRecipientSelectedOption = computed(() =>
  findRecipientOption(repairRecipientAgentId.value),
);

function defaultRepairRecipientOption(): AgentPersonaOption | null {
  const defaultAgentId = String(props.defaultCreateConversationAgentId || "").trim();
  const hasValidPersona = (option: AgentPersonaOption) => !option.personaMissing;
  return repairRecipientOptions.value.find((option) =>
    hasValidPersona(option)
    && defaultAgentId && String(option.agentId || "").trim() === defaultAgentId
  ) || repairRecipientOptions.value.find(hasValidPersona)
    || repairRecipientOptions.value[0] || null;
}

watch(
  () => [
    activeConversationRecipientMissing.value,
    props.activeConversationId,
    props.defaultCreateConversationAgentId,
    repairRecipientOptions.value.map((option) => option.agentId).join("|"),
  ] as const,
  () => {
    if (!activeConversationRecipientMissing.value) return;
    if (repairRecipientSelectedOption.value) return;
    const option = defaultRepairRecipientOption();
    repairRecipientAgentId.value = String(option?.agentId || "").trim();
  },
  { immediate: true },
);

const toolReviewAgentOptions = computed(() =>
  // 用户主动发起代码审查不受 AI delegate 工具的“直接下级人格”限制。
  (Array.isArray(props.createConversationAgentOptions) ? props.createConversationAgentOptions : []),
);

const agentNameMap = computed<Record<string, string>>(() => {
  const map: Record<string, string> = {};
  for (const option of props.createConversationAgentOptions || []) {
    const agentId = String(option.agentId || "").trim();
    if (!agentId || map[agentId]) continue;
    map[agentId] = String(option.agentName || option.name || agentId).trim() || agentId;
  }
  return map;
});

const agentNameMapSignature = computed(() =>
  Object.entries(agentNameMap.value)
    .map(([id, name]) => `${id}:${name}`)
    .sort()
    .join("|"),
);

const chatFileReaderSessionKey = computed(() => {
  const conversationId = String(props.activeConversationId || "").trim();
  return conversationId ? `easy_call.chat_file_reader_session.${conversationId}.v1` : "";
});

const legacyChatFileReaderSessionKey = computed(() => {
  const conversationId = String(props.activeConversationId || "").trim();
  return conversationId ? `easy-call.chat.file-reader-session.${conversationId}` : "";
});

// 文件阅读器项目根：优先取会话概览里的 workspaceRootPath（与 sessionKey 同步到达），
// 避免 currentWorkspaceRootPath（workspace.list 异步）滞后导致的 Home 闪现；
// 但在草稿态下优先采用当前选中的工作区，保证用户切换工作区时即时生效。
const effectiveFileReaderRootPath = computed(() => {
  if (activeConversationIsDraft.value && props.currentWorkspaceRootPath) {
    return String(props.currentWorkspaceRootPath || "").trim();
  }
  const conversationId = String(props.activeConversationId || "").trim();
  if (conversationId) {
    const listA = (props.unarchivedConversationItems || []) as Array<Record<string, unknown>>;
    const listB = (props.conversationItems || []) as Array<Record<string, unknown>>;
    const hit = listA.find((item) => String(item.conversationId || "").trim() === conversationId)
      || listB.find((item) => String(item.conversationId || "").trim() === conversationId);
    const workspacePath = String((hit as Record<string, unknown>)?.workspaceRootPath || "").trim();
    if (workspacePath) return workspacePath;
  }
  return String(props.currentWorkspaceRootPath || "").trim();
});

/**
 * 当前会话在用的真实工作区根：工作树模式且工作树已就绪时使用工作树目录，
 * 否则沿用主工作区路径。用于在 Git 面板工作树列表标出「本会话」、以及对齐阅读器初始根。
 */
const effectiveSessionRootPath = computed(() => {
  const isWorktree = props.currentWorkspaceWorkMode === "worktree";
  const worktreePath = String(props.currentWorkspaceWorktreePath || "").trim();
  const worktreeExists = Boolean(props.currentWorkspaceWorktreeExists);
  if (isWorktree && worktreeExists && worktreePath) {
    return worktreePath;
  }
  return effectiveFileReaderRootPath.value;
});

// ==================== messages / audio ====================

const showSideConversationList = computed(() => !!props.sideConversationListVisible);
const showConversationActions = computed(() => !props.hideConversationControlPanel);
const showOpenInBrowserButton = computed(() => props.showOpenInBrowserButton ?? true);

function canRegenerateBlock(block: ChatMessageBlock, blockIndex: number): boolean {
  if (block.role !== "assistant" || block.isExtraTextBlock) return false;
  for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
    const candidate = props.messageBlocks[idx];
    if (candidate.role !== "assistant" || candidate.isExtraTextBlock) continue;
    return idx === blockIndex;
  }
  return false;
}

function canConfirmPlan(block: ChatMessageBlock): boolean {
  if (block.role !== "assistant" || block.isExtraTextBlock) return false;
  if (block.planCard?.action !== "present") return false;
  const targetId = String(block.sourceMessageId || block.id || "").trim();
  if (targetId !== latestPendingPlanMessageId.value) return false;
  const blockIndex = props.messageBlocks.findIndex((item) => String(item.id || "").trim() === String(block.id || "").trim());
  if (blockIndex < 0) return false;
  return !props.messageBlocks.slice(blockIndex + 1).some((item) => !item.isExtraTextBlock && item.role === "user");
}

function openSelectionMenu(options: { allowWhenBusy?: boolean } = {}) {
  // 多选入口忙碌时禁用；分支/转发/分享属于子代理或纯读取操作，
  // 不影响主轮次，忙碌时允许进入选择模式（allowWhenBusy）。
  if (!options.allowWhenBusy && (props.chatting || props.frozen || conversationInteractionBusy.value)) return;
  clearNativeTextSelection();
  messageSelectionModeEnabled.value = true;
  selectedMessageRenderIds.value = [];
  void nextTick(() => composerPanelRef.value?.focusInput?.({ preventScroll: true }));
}
const openBranchSelectionMenu = () => openSelectionMenu({ allowWhenBusy: true });
const openDelegateSelectionMenu = () => {
  clearNativeTextSelection();
  codeReviewErrorText.value = "";
  launchPanel.value = readLaunchPanel();
  codeReviewDialogOpen.value = true;
};
const openForwardSelectionMenu = () => openSelectionMenu({ allowWhenBusy: true });
const openShareSelectionMenu = () => openSelectionMenu({ allowWhenBusy: true });

/** 从当前会话创建分支：后端纯复制会话快照，无需选择消息，忙碌中照常执行 */
function openBranchFromCurrentMessage() {
  emit("branchConversationFromCurrent");
}

function openTaskCreateDialog() {
  taskDialogMode.value = "create";
  taskDialogTask.value = null;
  taskDialogOpen.value = true;
}

function openTaskEditDialog(task: TaskEntry) {
  taskDialogMode.value = "edit";
  taskDialogTask.value = task;
  taskDialogOpen.value = true;
}

function closeTaskDialog() {
  taskDialogOpen.value = false;
}

function handleTaskCreated(task: TaskEntry) {
  taskDialogOpen.value = false;
  void refreshRunningTaskCount();
  emit("taskCreated", task);
}

function handleTaskUpdated(task: TaskEntry) {
  taskDialogOpen.value = false;
  void refreshRunningTaskCount();
  emit("taskUpdated", task);
}

function openConversationSummary(block: ChatMessageBlock, event?: MouseEvent) {
  event?.stopPropagation();
  const text = String(block?.text || "").trim();
  if (!text) return;
  conversationSummaryCard.value = { visible: true, text };
}

function openCompactionSummaryContextMenu(block: ChatMessageBlock, event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  const x = Math.max(8, Math.min(event.clientX, window.innerWidth - 184));
  const y = Math.max(8, Math.min(event.clientY, window.innerHeight - 48));
  compactionSummaryContextMenu.value = { x, y, block };
}

function recallCompactionSummaryFromContextMenu() {
  const block = compactionSummaryContextMenu.value?.block;
  compactionSummaryContextMenu.value = null;
  if (!block) return;
  const turnId = String(block.sourceMessageId || block.id || "").trim();
  if (!turnId) return;
  emit("recallTurn", { turnId });
}

function closeCompactionSummaryContextMenu() {
  compactionSummaryContextMenu.value = null;
}

function handleCompactionSummaryContextMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closeCompactionSummaryContextMenu();
}

function closeConversationSummaryCard() {
  conversationSummaryCard.value = { visible: false, text: "" };
}

function openAutoPushCard() {
  const currentTargetId = String(activeConversationSummary.value?.autoPushRemoteContactId || "").trim();
  autoPushEnabled.value = !!currentTargetId;
  autoPushSelectedContactId.value = currentTargetId;
  autoPushCardOpen.value = true;
}

function closeAutoPushCard() {
  autoPushCardOpen.value = false;
  autoPushSaving.value = false;
}

async function saveAutoPushCard() {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || autoPushSaving.value) return;
  autoPushSaving.value = true;
  try {
    await invokeTauri("conversation.autoPush", {
      input: {
        conversationId,
        remoteContactId: autoPushEnabled.value
          ? String(autoPushSelectedContactId.value || "").trim() || null
          : null,
      },
    });
    autoPushCardOpen.value = false;
  } finally {
    autoPushSaving.value = false;
  }
}

// ==================== ide context ====================

const {
  visibleIdeContextGroups, attachedIdeContextReferences,
  attachReference: handleAttachIdeContextReference,
  removeReference: handleRemoveIdeContextReference,
  clearAttachedReferences: clearAttachedIdeContextReferences,
} = useIdeContext({
  activeConversationId: toRef(props, "activeConversationId"),
  workspaces: toRef(props, "workspaces"),
  currentWorkspaceRootPath: toRef(props, "currentWorkspaceRootPath"),
  currentWorkspaceName: toRef(props, "currentWorkspaceName"),
  enabled: computed(() => true),
});

const fileReaderVisibleContextReference = ref<IdeContextReferenceItem | null>(null);
const fileReaderSelectionContextReference = ref<IdeContextReferenceItem | null>(null);
const fileReaderContextReferences = computed<IdeContextReferenceItem[]>(() => {
  const visible = fileReaderVisibleContextReference.value;
  const selection = fileReaderSelectionContextReference.value;
  if (!visible && !selection) return [];
  if (!visible) return selection ? [selection] : [];
  if (!selection) return [visible];
  const visibleFilePath = String(visible.filePath || "").trim();
  const selectionFilePath = String(selection.filePath || "").trim();
  return visibleFilePath && visibleFilePath === selectionFilePath ? [selection] : [visible];
});
const {
  sideFileTagsEnabled,
  ideBridgeFileTagsEnabled,
} = useChatComposerAppearance();
const mergedVisibleIdeContextGroups = computed<IdeContextWorkspaceGroup[]>(() => {
  const propGroups = Array.isArray(props.ideContextGroups) ? props.ideContextGroups : [];
  const baseGroups = propGroups.length > 0 ? propGroups : visibleIdeContextGroups.value;
  return visibleChatComposerContextGroups({
    sideReferences: fileReaderContextReferences.value,
    sideWorkspacePath: props.currentWorkspaceRootPath,
    sideWorkspaceName: String(props.currentWorkspaceName || "").trim() || t("chat.allowedWorkspaceButton"),
    ideBridgeGroups: baseGroups,
    sideFileTagsEnabled: sideFileTagsEnabled.value,
    ideBridgeFileTagsEnabled: ideBridgeFileTagsEnabled.value,
  });
});

function handleCaptureFileReaderContextReference(reference: IdeContextReferenceItem) {
  const source = String(reference.source || "").trim();
  if (source === "visible_range") {
    fileReaderVisibleContextReference.value = { ...reference };
  } else {
    fileReaderSelectionContextReference.value = { ...reference };
  }
}

function handleAddFileReaderContextReference(reference: IdeContextReferenceItem) {
  handleAttachIdeContextReference(reference);
  fileReaderSelectionContextReference.value = null;
  void nextTick(() => composerPanelRef.value?.focusInput?.({ preventScroll: true }));
}

function handleClearFileReaderSelectionContextReference() {
  fileReaderSelectionContextReference.value = null;
}

function clearFileReaderContextReferences(paths?: string[]) {
  const candidates = clearFileReaderContextCandidates({
    visible: fileReaderVisibleContextReference.value,
    selection: fileReaderSelectionContextReference.value,
  }, paths);
  fileReaderVisibleContextReference.value = candidates.visible;
  fileReaderSelectionContextReference.value = candidates.selection;
}

watch(() => props.activeConversationId, () => {
  fileReaderVisibleContextReference.value = null;
  fileReaderSelectionContextReference.value = null;
});

// ==================== selection state shared between virtual list & selection mode ====================

const messageSelectionModeEnabled = ref(false);
const selectedMessageRenderIds = ref<string[]>([]);
const selectedMessageRenderIdSet = computed(() => new Set(selectedMessageRenderIds.value));

// ==================== virtual list ====================

const { chatRenderItems, messageMemoKey } = useChatVirtualList({
  messageBlocks: toRef(props, "messageBlocks"), markdownIsDark, playingAudioId,
  userAlias: toRef(props, "userAlias"), userAvatarUrl: toRef(props, "userAvatarUrl"),
  personaNameMap: toRef(props, "personaNameMap"), personaAvatarUrlMap: toRef(props, "personaAvatarUrlMap"),
  chatting: toRef(props, "chatting"), conversationBusy: conversationInteractionBusy,
  frozen: toRef(props, "frozen"), messageSelectionModeEnabled,
  selectedMessageRenderIdSet,
  canRegenerateBlock, canConfirmPlan,
});

const virtualRenderItems = computed<ChatRenderItem[]>(() => [...chatRenderItems.value]);
const olderHistoryCorrectionAllowed = ref(false);

// ==================== 回合容器架构 (Path B 实验) ====================
export type TurnContainer = {
  id: string;
  items: ChatRenderItem[];
  isLatest: boolean;
};

const turnContainers = computed<TurnContainer[]>(() => {
  const list = chatRenderItems.value;
  if (list.length === 0) return [];
  const turns: TurnContainer[] = [];
  let currentTurn: TurnContainer = {
    id: "turn-intro",
    items: [],
    isLatest: false,
  };

  for (const item of list) {
    const isUserMessage = item.kind === "message" && item.block && isUserBlock(item.block);
    if (isUserMessage) {
      if (currentTurn.items.length > 0) {
        turns.push(currentTurn);
      }
      const turnId = item.block?.sourceMessageId || item.block?.id || item.renderId || `turn-${turns.length}`;
      currentTurn = {
        id: `turn-${turnId}`,
        items: [item],
        isLatest: false,
      };
    } else {
      currentTurn.items.push(item);
    }
  }
  if (currentTurn.items.length > 0) {
    currentTurn.isLatest = true;
    turns.push(currentTurn);
  }
  return turns;
});

// virtua 最简：shift 在顶部 prepend 时保持视口锚定，无需隐藏预量 staged 容器
const virtuaRef = ref<InstanceType<typeof Virtualizer> | null>(null);
const virtuaShift = computed(() => {
  if (turnContainers.value.length === 0) return false;
  return !!props.loadingOlderHistory || olderHistoryCorrectionAllowed.value;
});
// 初始测高覆盖层已删除：virtua 内部用 ResizeObserver 实时测量，首帧不再出现
// 估计高度导致的行重叠，这层遮挡没有可挡的对象（停用于 9d3429a35 迁移 virtua 时）。

// Virtualizer 只在 scrollContainer 就绪与会话切换时换实例：外层滚动容器与 chatContentRoot
// 保持常驻（历史区不再重建），避免离场阶段高度塌陷把 scrollTop 夹到 0；同时清掉 virtua 的
// 索引型测量缓存，防止新会话尾部继承旧会话的实测高度。
const virtuaKey = computed(() =>
  `${scrollContainer.value ? "virtua-ready" : "virtua-pending"}-${String(props.activeConversationId || "conversation-empty").trim()}`,
);

// ==================== block tracking ====================

const { isOwnMessage, latestOwnMessageId, latestOwnElasticItemId } =
  useChatBlockTracking(toRef(props, "messageBlocks"), chatRenderItems);
let lastTrackedElasticItemId = "";

// ==================== selection mode ====================

const {
  selectedMessageBlocks, enterMessageSelectionMode, toggleMessageSelected,
  exitMessageSelectionMode: resetMessageSelectionMode, copySelectedMessages, emitSelectionAction,
} = useChatSelection({
  chatRenderItems: computed(() => chatRenderItems.value.flatMap((item) => {
    if (item.kind === "message") return [{ renderId: item.renderId, block: item.block }];
    return [];
  })),
  messageSelectionModeEnabled,
  selectedMessageRenderIds,
  personaNameMap: props.personaNameMap, userAlias: props.userAlias,
  conversationId: toRef(props, "activeConversationId"),
  t,
  onEmit: {
    selectionActionCopy: (payload) => {
      handleSelectionCopyDone(payload.count);
      emit("selectionActionCopy", payload);
    },
    selectionActionCopyError: (payload) => {
      handleSelectionCopyFailed();
      emit("selectionActionCopyError", payload);
    },
    selectionActionBranch: (payload) => emit("selectionActionBranch", payload),
    selectionActionForward: (payload) => emit("selectionActionForward", payload),
    selectionActionShare: (payload) => emit("selectionActionShare", payload),
  },
});

function handleEnterMessageSelectionMode(selectionKey: string) {
  enterMessageSelectionMode(selectionKey);
}

function handleExitMessageSelectionMode() {
  resetMessageSelectionMode();
}

defineExpose({
  exitMessageSelectionMode: handleExitMessageSelectionMode,
  finishLaunchDelegate,
  showTransientNotice,
  openFileInReader,
  openDirectoryInReader,
});

const {
  scrollContainer, composerContainer, toolbarContainer, chatLayoutRoot,
  atConversationBottom, userScrollingUp,
  followBottom, startFollowBottom, stopFollowBottom,
  sessionControlPanelVisible, sessionFloatDockStyle, scrollIdle, onScroll,
  noteWheelScrollIntent, noteKeyboardScrollIntent, beginPointerScrollIntent,
  setFollowAnimating, prepareBottomAlignmentLayout,
} = useChatScrollLayout({
  activeConversationId: toRef(props, "activeConversationId"),
  chatting: toRef(props, "chatting"), busy: conversationInteractionBusy,
  frozen: toRef(props, "frozen"),
  timelineItemCount: computed(() => virtualRenderItems.value.length),
  onReachedBottom: () => emit("reachedBottom"),
  focusComposerInput: (options) => composerPanelRef.value?.focusInput(options),
});

const scrollContainerHeight = ref(0);
function updateScrollContainerHeight() {
  const el = scrollContainer.value;
  if (!el) return;
  if (scrollContainerHeight.value !== el.clientHeight) {
    scrollContainerHeight.value = el.clientHeight;
  }
}
/** 最新一轮回合容器撑开的视口占比。留一截余量，不顶满整个可视高。 */
const TURN_VIEWPORT_HEIGHT_RATIO = 0.92;

/**
 * 滚动区底部固定留白，用来避让悬浮操作区。
 * 贴底时压在下沿的是工作条（btn-sm 32px）与时间线/回到底部竖列，操作区底边距视口底还有 8px，
 * 合计约 40px；再留 8px 余量，让最后一条消息与按钮之间不是零距离。
 */
const CHAT_SCROLL_BOTTOM_RESERVE_PX = 48;

const turnViewportMinHeight = computed(() => {
  // 净可视高度 = clientHeight - 内容区上下留白（py-3: 12px + 12px = 24px）。
  // 这段留白挂在 chatContentRoot 上而不是滚动容器上，是为了让 sticky top:0 能真正贴顶。
  const netViewportHeight = Math.max(0, scrollContainerHeight.value - 24);
  // 再减掉滚动区底部的固定留白：那段空间要让给悬浮操作区，不能算进容器最小高度
  return Math.max(0, Math.round(netViewportHeight * TURN_VIEWPORT_HEIGHT_RATIO) - CHAT_SCROLL_BOTTOM_RESERVE_PX);
});

watch(scrollContainer, (el) => {
  if (el) updateScrollContainerHeight();
});
onMounted(() => {
  updateScrollContainerHeight();
  window.addEventListener("resize", updateScrollContainerHeight);
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", updateScrollContainerHeight);
});

// ==================== 会话切换渐入 ====================

// 老写法（story 的 page-switch）之所以舒服，是因为它切会话时消息常驻内存、同步可读，元素一插入
// 就有内容，180ms 淡入全程都有东西可看。我们这边数据也在手边，但首屏要经过虚拟列表测量才真正
// 画出来。所以不追求「等画稳再淡入」——那会先空一拍；改成切过去两帧后就开始渐显，
// 内容在这段渐显里自己画出来，画面始终连续。
const conversationRevealPhase = ref<1 | 2>(1);
const conversationRevealHidden = ref(true);
let conversationRevealFrame = 0;

function playConversationReveal() {
  if (conversationRevealFrame) {
    cancelAnimationFrame(conversationRevealFrame);
    conversationRevealFrame = 0;
  }
  // 两个内容相同的动画名轮换：class 真变了，浏览器才肯重播同一段 animation。
  conversationRevealPhase.value = conversationRevealPhase.value === 1 ? 2 : 1;
  conversationRevealHidden.value = false;
}

function holdConversationReveal() {
  // 先把消息区压回透明：切过去那一帧虚拟列表还停在顶部，直接露出来就是「先看到顶部再跳下去」。
  conversationRevealHidden.value = true;
  if (conversationRevealFrame) cancelAnimationFrame(conversationRevealFrame);
  // 两帧后开始渐显，够落位把视口挪到底，又不至于空出一段能感知的等待。
  conversationRevealFrame = requestAnimationFrame(() => {
    conversationRevealFrame = requestAnimationFrame(() => {
      conversationRevealFrame = 0;
      playConversationReveal();
    });
  });
}

watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    holdConversationReveal();
  },
);

// 侧面板是按会话 id 整棵重建的，新实例挂载时 activeConversationId 已经是新值，上面的 watch
// 不会触发；这里补一次，让它同样先压透明再渐显。
onMounted(() => {
  if (conversationRevealHidden.value) holdConversationReveal();
});

onBeforeUnmount(() => {
  if (conversationRevealFrame) cancelAnimationFrame(conversationRevealFrame);
});

// ==================== virtual scroll (virtua) ====================

// 远近分界：与当前首项索引差超过10项算远（约3屏），走瞬移保性能；以内走平滑
const VIRTUAL_SMOOTH_DISTANCE = 10;

function currentFirstVisibleVirtualIndex(): number {
  const el = scrollContainer.value;
  const top = el ? el.scrollTop : 0;
  const v = virtuaRef.value as unknown as { findItemIndex?: (offset: number) => number } | null;
  if (v && typeof v.findItemIndex === "function") {
    try {
      const idx = v.findItemIndex(top);
      if (Number.isFinite(idx) && (idx as number) >= 0) return idx as number;
    } catch {}
  }
  return Math.max(0, Math.floor(top / 300));
}

function resolveVirtualSmooth(targetIndex: number, requested: ScrollBehavior | undefined): boolean {
  if (requested !== "smooth") return false;
  return Math.abs(targetIndex - currentFirstVisibleVirtualIndex()) <= VIRTUAL_SMOOTH_DISTANCE;
}

// virtua 的 shift 在顶部插入时自动保持视口位置，无需手工锚定与隐藏预量
function scrollVirtualizerToIndex(
  index: number,
  options?: { align?: "auto" | "start" | "center" | "end"; behavior?: ScrollBehavior },
) {
  // 跳转会把视口挪离底部，先锁定跟随；时间线跳转与跳转到用户消息共用这一出口
  stopFollowBottom();
  let targetTurnIndex = index;
  if (index >= 0 && index < virtualRenderItems.value.length) {
    const targetItem = virtualRenderItems.value[index];
    const found = turnContainers.value.findIndex((t) => t.items.some((it) => it.id === targetItem.id));
    if (found >= 0) targetTurnIndex = found;
  }
  const len = turnContainers.value.length;
  const clampedTarget = len > 0 ? Math.max(0, Math.min(targetTurnIndex, len - 1)) : targetTurnIndex;
  const smooth = resolveVirtualSmooth(clampedTarget, options?.behavior);
  if (virtuaRef.value) {
    try {
      // virtua 不认 behavior，只认 smooth 布尔，这里做翻译，否则平滑永远不生效
      virtuaRef.value.scrollToIndex(clampedTarget, { align: options?.align, smooth } as any);
      return;
    } catch {}
  }
  // 兜底：直接按索引估算偏移（每项约 320px）滚动外层容器，保证跳转可用
  const el = scrollContainer.value;
  if (!el) return;
  const approxOffset = clampedTarget * 320;
  el.scrollTo({ top: approxOffset, behavior: smooth ? "smooth" : "auto" });
}

let ownTopAlignGen = 0;
const ownTopAlignPinnedTop = ref<number | null>(null);
// elastic 对齐只管同会话内的新用户消息：会话刚切时只记基线，对齐交给编排层的 own_top，避免两路抢。
let lastElasticAlignConversationId = "";
// 「切会话后的第一次对齐」标记：切换会话时置位，被对齐消费一次即清。
// 只有这一次走瞬间落位，其余（发送新消息等）走平滑。
let pendingSwitchAlign = true;

function consumeConversationSwitchAlign(): boolean {
  if (!pendingSwitchAlign) return false;
  pendingSwitchAlign = false;
  return true;
}

watch(latestOwnElasticItemId, (next, prev) => {
  const cid = String(props.activeConversationId || "").trim();
  if (lastElasticAlignConversationId !== cid) {
    lastElasticAlignConversationId = cid;
    lastTrackedElasticItemId = String(next || "").trim();
    return;
  }
  const nextId = String(next || "").trim();
  const prevId = String(prev || "").trim();
  if (!nextId || !prevId || nextId === prevId) return;
  lastTrackedElasticItemId = nextId;
  alignLatestOwnMessageToTop();
});

watch(() => String(props.activeConversationId || "").trim(), () => {
  ownTopAlignGen += 1;
  ownTopAlignPinnedTop.value = null;
  lastTrackedElasticItemId = "";
  pendingSwitchAlign = true;
});

// 最新用户消息对齐到视口顶部（Path B 回合容器架构）：
// 容器自身通过 turnViewportMinHeight 在最新轮次撑开视口；换轮次时上一轮自动回归内容真实高，无需留白计算
// 切会话时 Virtualizer 按会话 id 重建，新实例内部还没有实测高度，scrollToIndex 会把偏移算成 0
// 而滚不上去；这里按目标回合在 DOM 里的真实位置定位，并用 rAF 重试吃掉测量前的抖动。
//
// 瞬间还是平滑：只有「切会话后的第一次对齐」走瞬间，其余（发送新消息等）走平滑。
// 切会话时新实例还没测高，真实位置要重试几帧才收敛，平滑动画与重试会互相踩——
// 动画期间 scrollTop 每帧都在变，重试判据（偏差 > 2px）恒为真，于是每帧重新 scrollTo，
// 动画永远走不完，只能表现成一抖一抖。所以切会话直接落位。
// 发送消息时位置是确定的，一次位移就够，不需要重试，交给浏览器原生平滑。
function alignLatestOwnMessageToTop() {
  stopFollowBottom();
  updateScrollContainerHeight();
  const gen = (ownTopAlignGen += 1);
  const smooth = !consumeConversationSwitchAlign();
  // 对齐落空时的兜底：宁可在底部看到最新消息，也不要停在顶部。
  // 侧面板（追问 / 队员）按会话 id 整棵重建，虚拟列表首帧可能还没渲染出目标回合，
  // 按节点定位会落空；主会话列表常驻，切换时 DOM 里已经有上一轮，几乎走不到这里。
  const fallbackToBottom = () => {
    if (gen !== ownTopAlignGen) return;
    resetVirtualizerAtConversationBottom("auto");
  };

  const attempt = (retries: number) => {
    if (gen !== ownTopAlignGen) return;
    const el = scrollContainer.value;
    if (!el) {
      if (retries > 0) requestAnimationFrame(() => attempt(retries - 1));
      else fallbackToBottom();
      return;
    }
    // 目标回合要每帧重读：数据与 DOM 都可能比这次调用更晚就绪，
    // 在函数入口算一次就固定住的话，晚到的那一轮永远对不上。
    const turns = turnContainers.value;
    const targetTurnId = String(turns[turns.length - 1]?.id || "");
    const turnEl = targetTurnId
      ? (el.querySelector(`[data-turn-id="${targetTurnId}"]`) as HTMLElement | null)
      : null;
    if (!turnEl) {
      if (retries > 0) requestAnimationFrame(() => attempt(retries - 1));
      else fallbackToBottom();
      return;
    }
    const desired = Math.max(
      0,
      Math.round(el.scrollTop + turnEl.getBoundingClientRect().top - el.getBoundingClientRect().top),
    );
    el.scrollTo({ top: desired, behavior: smooth ? "smooth" : "auto" });
    chatScrollbarRef.value?.updateThumb();
    // 平滑动画期间不做重试：位置一次就够，重试只会打断动画
    if (!smooth && retries > 0 && Math.abs(el.scrollTop - desired) > 2) {
      requestAnimationFrame(() => attempt(retries - 1));
    }
  };
  // 帧数按老写法给到 6：虚拟列表首帧常常还没渲染出目标回合，
  // 4 帧（约 66ms）在侧面板整棵重建时会落空。
  void nextTick(() => requestAnimationFrame(() => attempt(6)));
}
// virtua 的 cache 不是响应式的，只能主动读一次快照。
// 这两条判据来自老写法：列表还没测完高度时 scrollHeight 是估算值，
// 滚到底后测量结果回填会让高度变长，而 scrollTop 与 targetTop 会一起被钳到上限，
// 偏差判据看到的是 0 —— 只信它就会以为已经到底，提前收手停在中间。
function readVirtualCacheSizes(): number[] | null {
  const handle = virtuaRef.value as unknown as { cache?: unknown } | null;
  const snapshot = handle?.cache as unknown;
  if (Array.isArray(snapshot) && Array.isArray(snapshot[0])) {
    return snapshot[0] as number[];
  }
  return null;
}

function isMeasuredVirtualSize(size: number | undefined): boolean {
  return typeof size === "number" && Number.isFinite(size) && size > 0 && size !== -1;
}

/** 末尾几项还没测出真实高度：此时贴底结果不可信，要再等一帧。 */
function virtualTailUnmeasured(): boolean {
  if (!virtuaRef.value) return false;
  const sizes = readVirtualCacheSizes();
  if (!sizes) return true;
  return sizes.slice(-4).some((size) => !isMeasuredVirtualSize(size));
}

function resetVirtualizerAtConversationBottom(behavior: "auto" | "smooth" = "auto") {
  const el = scrollContainer.value;
  if (el) {
    const attempt = (retries = 6) => {
      const targetTop = Math.max(0, el.scrollHeight - el.clientHeight);
      el.scrollTo({ top: targetTop, behavior });
      chatScrollbarRef.value?.updateThumb();
      if (retries > 0 && (virtualTailUnmeasured() || Math.abs(el.scrollTop - targetTop) > 2)) {
        requestAnimationFrame(() => attempt(retries - 1));
      }
    };
    void nextTick(() => requestAnimationFrame(() => attempt()));
    return;
  }
  const len = turnContainers.value.length;
  if (len <= 0 || !virtuaRef.value) return;
  try { virtuaRef.value.scrollToIndex(len - 1, { align: "end", smooth: resolveVirtualSmooth(len - 1, behavior) } as any); } catch {}
}
function scheduleVirtualMeasure() {
  // virtua 内部 ResizeObserver 自动测量，无需手动触发
}
function syncViewportMetrics() {
  void nextTick(() => chatScrollbarRef.value?.updateThumb());
}

// 会话时间线：每条完成态消息（用户与助理）各占一个节点，标注发言人与发言时间。
// 时间线按钮只在有足够节点时出现；点击弹出几乎盖满聊天区的面板，节点点击跳转到该条消息。
type TimelineEntry = {
  id: string;
  index: number;
  speaker: string;
  avatarUrl: string;
  time: string;
  text: string;
  // 己方（用户）为真：决定节点落在时间线哪一侧——用户固定右侧、助理固定左侧
  isOwn: boolean;
  // 压缩标记：时间线按此把节点切成「一段」，一段对应服务端一个物理块文件
  isCompaction: boolean;
};

// 每句只留 150 字：前 30% + 后 70%
const TIMELINE_TEXT_LIMIT = 150;
const TIMELINE_TEXT_HEAD_RATIO = 0.3;

function cleanTimelineText(raw: string): string {
  return String(raw || "")
    .split(TIMELINE_TOOL_BREAK).join(" ")
    .replace(/\s*\[toolcall:[^\]\n]+\]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** 当前会话人格 id：先按 activeConversationId 在会话列表里精确取，回退会话摘要，再回退宿主传入的 activeAgentId。 */
const currentSessionAgentId = computed(() => {
  const convId = String(props.activeConversationId || "").trim();
  const conv = (props.conversationItems || props.unarchivedConversationItems || []).find(
    (item) => String(item.conversationId || "").trim() === convId,
  ) || activeConversationSummary.value;
  return String(conv?.agentId || props.activeAgentId || "").trim();
});

// 锚点与节点归属的判定收在 utils/timeline-anchors，主会话与追问面板共用同一套语义
const isSessionPersonaBlock = (block: ChatMessageBlock): boolean =>
  isSessionPersonaBlockShared(block, currentSessionAgentId.value);

function timelineSpeakerName(block: ChatMessageBlock): string {
  if (isUserBlock(block)) return String(props.userAlias || "").trim() || "我";
  const speakerId = String(block.speakerAgentId || "").trim();
  if (speakerId && props.personaNameMap?.[speakerId]) return props.personaNameMap[speakerId];
  return String(props.personaName || "").trim() || "助理";
}

function timelineSpeakerAvatar(block: ChatMessageBlock): string {
  if (isUserBlock(block)) return String(props.userAvatarUrl || "").trim();
  const speakerId = String(block.speakerAgentId || "").trim();
  const personaAvatar = speakerId ? String(props.personaAvatarUrlMap?.[speakerId] || "").trim() : "";
  return personaAvatar || String(props.assistantAvatarUrl || "").trim();
}

function formatTimelineTime(value?: string): string {
  const raw = String(value || "").trim();
  if (!raw) return "";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hour12: false });
}

// 蛇形时间线锚点：每条「当前会话人格之外的发言」一个点，附上紧随其后的人格回复尾段，供悬停预览卡显示。
// 与上面按「每条消息一个节点」的垂直时间线各管一摊：蛇板负责定位与挪动，垂直面板负责逐条浏览。
const timelineAnchors = computed<TimelineAnchor[]>(() => {
  const idToVirtual = new Map<string, number>();
  virtualRenderItems.value.forEach((item, idx) => {
    if (item.kind === "message" && item.block) {
      const bid = String(item.block.id || "");
      if (bid) idToVirtual.set(bid, idx);
    }
  });
  return buildTimelineAnchors((props.messageBlocks || []) as ChatMessageBlock[], idToVirtual, currentSessionAgentId.value);
});

const timelineEntries = computed<TimelineEntry[]>(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  // map to virtual index
  const idToVirtual = new Map<string, number>();
  virtualRenderItems.value.forEach((item, idx) => {
    if (item.kind === "message" && item.block) {
      const bid = String(item.block.id || "");
      if (bid) idToVirtual.set(bid, idx);
    }
  });
  const entries: TimelineEntry[] = [];
  let pendingCompactionBoundary = false;
  for (const block of blocks) {
    if (block.isStreaming) continue;
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    const compaction = isCompactionBlock(block);
    const text = cleanTimelineText(String(block.text || ""));
    const index = idToVirtual.get(String(block.id || ""));
    // 压缩块即使正文空也要留下分段边界，否则会把它后面一段并进上一段
    if (!text || index === undefined) {
      if (compaction) pendingCompactionBoundary = true;
      continue;
    }
    entries.push({
      id: String(block.id || `timeline-${entries.length}`),
      index,
      speaker: timelineSpeakerName(block),
      avatarUrl: timelineSpeakerAvatar(block),
      time: formatTimelineTime(block.createdAt),
      text,
      // 左右对齐跟消息气泡一致（共享气泡的 isRightAlignedMessage）；名字与头像另按 isUserBlock 判真实说话人
      isOwn: isRightAlignedMessage(block),
      isCompaction: compaction || pendingCompactionBoundary,
    });
    pendingCompactionBoundary = false;
  }
  return entries;
});

// 按钮显隐必须与蛇形时间线浮层同源：浮层渲染靠 timelineAnchors，按钮就不能只看逐条时间线的条目数，
// 否则会出现「按钮在、浮层算不出节点」，悬停瞬间按钮即被置为不可见而消失。
const canShowTimeline = computed(() => timelineAnchors.value.length >= 2);

const prefersReducedMotionTimeline = ref(false);
onMounted(() => {
  try {
    const mql = window.matchMedia("(prefers-reduced-motion: reduce)");
    prefersReducedMotionTimeline.value = mql.matches;
    const onChange = (e: MediaQueryListEvent) => { prefersReducedMotionTimeline.value = e.matches; };
    if (typeof (mql as any).addEventListener === "function") (mql as any).addEventListener("change", onChange);
    else (mql as any).addListener?.(onChange);
  } catch {}
});

const activeTimelineIndex = computed<number | null>(() => {
  const entries = timelineEntries.value;
  if (entries.length === 0) return null;
  const scrollEl = scrollContainer.value;
  const top = scrollEl ? scrollEl.scrollTop : 0;
  let firstVisible = entries[0]!.index;
  // virtua 提供 findItemIndex，可直接定位顶部可见索引
  if (virtuaRef.value && typeof (virtuaRef.value as any).findItemIndex === "function") {
    try {
      const idx = (virtuaRef.value as any).findItemIndex(top);
      if (Number.isFinite(idx) && idx >= 0) firstVisible = idx;
    } catch {}
  } else if (scrollEl) {
    // 回退：线性查找近似
    firstVisible = Math.max(0, Math.floor(top / 300));
  }
  // 找到最后一个 index <= firstVisible 的节点
  let active = entries[0]!.index;
  for (const entry of entries) {
    if (entry.index <= firstVisible + 1) active = entry.index;
    else break;
  }
  return active;
});

function handleTimelineJump(virtualIndex: number) {
  if (virtualIndex < 0) return;
  scrollVirtualizerToIndex(virtualIndex, { align: "start", behavior: prefersReducedMotionTimeline.value ? "auto" : "smooth" });
}

// 蛇形时间线：按钮悬停即在原位向上展开；展开期间按钮位置换成「预览」按钮，点它才打开垂直面板。
const hoveredTimelineIndex = ref<number | null>(null);
const timelineFloatButtonRef = ref<HTMLElement | null>(null);
// 蛇板实例：外点关闭按它判归属，避免主会话与追问面板的蛇板互相关闭
const timelineSnakeBoardRef = ref<{ containsNode: (node: Node | null) => boolean } | null>(null);
const timelineFloatOpen = ref(false);
// 悬停展开的微防抖：避免光标扫过按钮误触弹开
const TIMELINE_FLOAT_OPEN_DELAY_MS = 120;
// 收起前的逗留时间：光标在按钮与卡片之间移动时靠它兜住
const TIMELINE_FLOAT_CLOSE_DELAY_MS = 320;
let timelineFloatOpenTimer: ReturnType<typeof setTimeout> | null = null;
let timelineFloatCloseTimer: ReturnType<typeof setTimeout> | null = null;

// 时间线面板：点击「预览」按钮弹出，几乎盖满聊天区；点节点跳转到该条消息并收起面板。
const timelinePanelOpen = ref(false);

// 悬停展开与排无关：贴底时按钮在操作条里、离底时在上排，两处都要能悬停展开
const timelineButtonVisible = computed(() =>
  canShowTimeline.value && !timelinePanelOpen.value,
);
// 按钮本身：两排共用一个，常驻不重建，切换时只换底座——离底那排是不透明圆钮，贴底时并入操作条走 ghost
const timelineEntryButtonVisible = computed(() =>
  canShowTimeline.value && !timelinePanelOpen.value,
);
const timelineEntryButtonClass = computed(() =>
  displayedSessionRow.value === "toolbar" ? SESSION_GHOST_CIRCLE : SESSION_FLOAT_FROST_CIRCLE,
);
const timelineFloatPanelVisible = computed(() => timelineFloatOpen.value && timelineButtonVisible.value);

function clearTimelineFloatOpenTimer() {
  if (!timelineFloatOpenTimer) return;
  clearTimeout(timelineFloatOpenTimer);
  timelineFloatOpenTimer = null;
}
function clearTimelineFloatCloseTimer() {
  if (!timelineFloatCloseTimer) return;
  clearTimeout(timelineFloatCloseTimer);
  timelineFloatCloseTimer = null;
}
function closeTimelineFloat() {
  clearTimelineFloatOpenTimer();
  clearTimelineFloatCloseTimer();
  timelineFloatOpen.value = false;
  hoveredTimelineIndex.value = null;
}
function openTimelinePanel() {
  closeTimelineFloat();
  if (!canShowTimeline.value) return;
  timelinePanelOpen.value = true;
}
function handleTimelineFloatEnter() {
  clearTimelineFloatCloseTimer();
  if (timelineFloatPanelVisible.value) return;
  if (!timelineButtonVisible.value) return;
  if (timelineFloatOpenTimer) return;
  timelineFloatOpenTimer = setTimeout(() => {
    timelineFloatOpenTimer = null;
    // 延时窗口里触发源可能已经消失（锚点不足、切回下排、面板打开），执行前重新校验
    if (!timelineButtonVisible.value) return;
    timelineFloatOpen.value = true;
  }, TIMELINE_FLOAT_OPEN_DELAY_MS);
}
function handleTimelineFloatLeave() {
  clearTimelineFloatOpenTimer();
  clearTimelineFloatCloseTimer();
  timelineFloatCloseTimer = setTimeout(() => {
    timelineFloatCloseTimer = null;
    timelineFloatOpen.value = false;
    hoveredTimelineIndex.value = null;
  }, TIMELINE_FLOAT_CLOSE_DELAY_MS);
}
// 展开后按钮已被卡片整个盖住，此时的 mouseleave 只代表光标进了卡片，不排程收起；
// 真正离开交给卡片的 leave-zone，不依赖浏览器在按钮隐藏时补发事件
function handleTimelineButtonLeave() {
  if (timelineFloatPanelVisible.value) return;
  handleTimelineFloatLeave();
}
function handleTimelineButtonClick() {
  // 点按钮只是把蛇板钉住展开；「预览」在展开后卡片右下角那个按钮上
  clearTimelineFloatCloseTimer();
  if (!timelineButtonVisible.value) return;
  timelineFloatOpen.value = true;
}
function handleTimelineEntryButtonClick() {
  // 贴底时这个按钮就是操作条里的那个：点一下直接开概览，不必先展开蛇板；
  // 离底那排沿用原来的「钉住蛇板」语义
  if (displayedSessionRow.value === "toolbar") {
    openTimelinePanel();
    return;
  }
  handleTimelineButtonClick();
}
function handleTimelineFloatDocumentPointerDown(event: MouseEvent | TouchEvent) {
  if (!timelineFloatPanelVisible.value) return;
  const target = event.target as Node | null;
  // 按本实例的蛇板判定归属：同一页面可能同时有主会话与追问面板两块蛇板
  if (timelineSnakeBoardRef.value?.containsNode(target)) return;
  const anchorEl = timelineFloatButtonRef.value;
  if (anchorEl && target && anchorEl.contains(target)) return;
  closeTimelineFloat();
}
function handleTimelineFloatKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  // 面板打开时交给面板自己的 Esc 处理，这里不抢
  if (timelinePanelOpen.value) return;
  if (!timelineFloatOpen.value && !timelineFloatOpenTimer) return;
  closeTimelineFloat();
}
// 按钮所在的那一排消失（贴底换成工作区 bar、锚点不足、面板打开）时，蛇板一并收起
watch(timelineButtonVisible, (visible) => {
  if (visible) return;
  closeTimelineFloat();
});
onMounted(() => {
  window.addEventListener("pointerdown", handleTimelineFloatDocumentPointerDown, true);
  window.addEventListener("keydown", handleTimelineFloatKeydown);
});
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", handleTimelineFloatDocumentPointerDown, true);
  window.removeEventListener("keydown", handleTimelineFloatKeydown);
  closeTimelineFloat();
});
function closeTimelinePanel() {
  timelinePanelOpen.value = false;
}
function handleTimelineEntryJump(entry: TimelineEntry) {
  handleTimelineJump(entry.index);
  timelinePanelOpen.value = false;
}

// ========== 时间线分段加载 ==========
// 分段依据是压缩标记：一段 = 压缩边界到下一个压缩边界之间的消息，服务端一个物理块文件就是一段。
// 面板默认只渲染最后一段；滚到顶部先放开内存里更早的段，内存放空了才向后端要更早的一整段。
const TIMELINE_TOP_TRIGGER_PX = 48;

const timelineScrollerRef = ref<{ scrollerRef?: HTMLElement | null } | null>(null);
const revealedTimelineSegmentCount = ref(1);
let timelineScrollEl: HTMLElement | null = null;
let timelineRevealBusy = false;
let timelinePendingRevealMode: "anchor" | "fill" | null = null;
let timelineScrollAnchor: { height: number; top: number } | null = null;

// 每段首节点的下标；首节点恒为一段起点，压缩标记节点另起一段
const timelineSegmentStarts = computed<number[]>(() => computeTimelineSegmentStarts(timelineEntries.value));

const timelineVisibleStartIndex = computed(() =>
  resolveTimelineVisibleStartIndex(timelineSegmentStarts.value, revealedTimelineSegmentCount.value),
);

const visibleTimelineEntries = computed(() => timelineEntries.value.slice(timelineVisibleStartIndex.value));

async function waitTimelineFrame() {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

function captureTimelineScrollAnchor() {
  const el = timelineScrollerRef.value?.scrollerRef ?? null;
  timelineScrollAnchor = el ? { height: el.scrollHeight, top: el.scrollTop } : null;
}

// 顶部前插内容后视口会被顶下去，按内容高度差把 scrollTop 补回来，保持原来看到的位置不动
function restoreTimelineScrollAnchor() {
  const anchor = timelineScrollAnchor;
  timelineScrollAnchor = null;
  const el = timelineScrollerRef.value?.scrollerRef ?? null;
  if (!anchor || !el) return;
  const delta = el.scrollHeight - anchor.height;
  if (delta > 0) el.scrollTop = anchor.top + delta;
}

async function revealTimelineSegments(count: number, options?: { stickToBottom?: boolean }) {
  timelineRevealBusy = true;
  // 补齐首屏时内容本来就没溢出，视口锚点没有意义，直接贴着底部继续加
  const stickToBottom = options?.stickToBottom === true;
  if (!stickToBottom) captureTimelineScrollAnchor();
  revealedTimelineSegmentCount.value += count;
  await nextTick();
  await waitTimelineFrame();
  if (stickToBottom) {
    const el = timelineScrollerRef.value?.scrollerRef ?? null;
    if (el) el.scrollTop = el.scrollHeight;
  } else {
    restoreTimelineScrollAnchor();
  }
  timelineRevealBusy = false;
}

async function revealEarlierTimelineBlock() {
  if (timelineRevealBusy || timelinePendingRevealMode || !timelinePanelOpen.value) return;
  if (revealedTimelineSegmentCount.value < timelineSegmentStarts.value.length) {
    await revealTimelineSegments(1);
    return;
  }
  if (!props.hasMoreHistory || props.loadingOlderHistory) return;
  // 拿回来的段先落在已加载消息里但不渲染，等加载完成后再放开，定位在放开那一刻量更准
  timelinePendingRevealMode = "anchor";
  emit("loadOlderCompactionSegment");
}

// 面板内容还撑不满可视区时，滚动事件永远不会触发，只能自己往前补齐；
// 补到能滚动为止，或者更早的内容取空了为止。
async function fillTimelineIfNotScrollable() {
  if (!timelinePanelOpen.value) return;
  for (let guard = 0; guard < 50; guard += 1) {
    await nextTick();
    await waitTimelineFrame();
    const el = timelineScrollerRef.value?.scrollerRef ?? null;
    if (!el || !timelinePanelOpen.value) return;
    // 量不到高度（面板还在布局或处于隐藏态）时不能判断是否撑满，直接放弃这一轮，避免把历史一次抽干
    if (el.clientHeight <= 0) return;
    if (el.scrollHeight > el.clientHeight + 1) return;
    if (timelineRevealBusy || timelinePendingRevealMode) return;
    if (revealedTimelineSegmentCount.value < timelineSegmentStarts.value.length) {
      await revealTimelineSegments(1, { stickToBottom: true });
      continue;
    }
    if (!props.hasMoreHistory || props.loadingOlderHistory) return;
    timelinePendingRevealMode = "fill";
    emit("loadOlderCompactionSegment");
    return;
  }
}

function onTimelinePanelScroll() {
  if (!timelinePanelOpen.value) return;
  const el = timelineScrollerRef.value?.scrollerRef ?? null;
  if (!el || el.scrollTop > TIMELINE_TOP_TRIGGER_PX) return;
  void revealEarlierTimelineBlock();
}

function detachTimelineScrollListener() {
  if (!timelineScrollEl) return;
  timelineScrollEl.removeEventListener("scroll", onTimelinePanelScroll);
  timelineScrollEl = null;
}

function attachTimelineScrollListener() {
  const el = timelineScrollerRef.value?.scrollerRef ?? null;
  if (!el || el === timelineScrollEl) return;
  detachTimelineScrollListener();
  timelineScrollEl = el;
  el.addEventListener("scroll", onTimelinePanelScroll, { passive: true });
}

// 一段加载完成后放行：新到的段落在旧内容之前，用户正停在顶部，把它一起显出来
watch(
  () => props.loadingOlderHistory,
  async (loading, wasLoading) => {
    if (loading || !wasLoading) return;
    const mode = timelinePendingRevealMode;
    if (!mode) return;
    timelinePendingRevealMode = null;
    if (!timelinePanelOpen.value) {
      revealedTimelineSegmentCount.value += 1;
      return;
    }
    await revealTimelineSegments(1, { stickToBottom: mode === "fill" });
    if (mode === "fill") await fillTimelineIfNotScrollable();
  },
);

watch(timelinePanelOpen, async (open) => {
  timelinePendingRevealMode = null;
  timelineScrollAnchor = null;
  timelineRevealBusy = false;
  if (!open) {
    detachTimelineScrollListener();
    return;
  }
  revealedTimelineSegmentCount.value = 1;
  await nextTick();
  attachTimelineScrollListener();
  const el = timelineScrollerRef.value?.scrollerRef ?? null;
  if (el) el.scrollTop = el.scrollHeight;
  // 首屏撑不满就滚不动，滚动事件也就永远不会来，这里先自己补齐
  await fillTimelineIfNotScrollable();
});

function handleTimelinePanelKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  closeTimelinePanel();
}
onMounted(() => {
  window.addEventListener("keydown", handleTimelinePanelKeydown);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleTimelinePanelKeydown);
  detachTimelineScrollListener();
});

// 节点不足或切换会话时收起面板：面板是基于当前会话消息构建的，不能跨会话保留
watch(
  () => [String(props.activeConversationId || "").trim(), timelineEntries.value.length] as const,
  ([, count]) => {
    if (count < 2) timelinePanelOpen.value = false;
  },
);
watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    timelinePanelOpen.value = false;
  },
);



const currentWorkspacePermissionKind = computed<"approval" | "full_access" | "autonomous">(() => {
  if (props.currentWorkspaceAutonomousMode) return "autonomous";
  const targetPath = String(props.currentWorkspaceRootPath || "").trim().toLowerCase();
  const workspaceList = Array.isArray(props.workspaces) ? props.workspaces : [];
  const matched = workspaceList.find((item) => String(item.path || "").trim().toLowerCase() === targetPath);
  if (matched?.access === "approval" || matched?.access === "full_access") {
    return matched.access;
  }
  const mainWorkspace = workspaceList.find((item) => String(item.level || "").trim() === "main");
  if (mainWorkspace?.access === "approval" || mainWorkspace?.access === "full_access") {
    return mainWorkspace.access;
  }
  return "approval";
});

// 草稿卡片回显用：不含 autonomous 分支的纯权限值
const currentWorkspaceAccess = computed<"approval" | "full_access">(() => {
  const targetPath = String(props.currentWorkspaceRootPath || "").trim().toLowerCase();
  const workspaceList = Array.isArray(props.workspaces) ? props.workspaces : [];
  const matched = workspaceList.find((item) => String(item.path || "").trim().toLowerCase() === targetPath);
  if (matched?.access === "approval" || matched?.access === "full_access") {
    return matched.access;
  }
  const mainWorkspace = workspaceList.find((item) => String(item.level || "").trim() === "main");
  if (mainWorkspace?.access === "approval" || mainWorkspace?.access === "full_access") {
    return mainWorkspace.access;
  }
  return "approval";
});

const supportsFloatingSessionToolbar = computed(() =>
  !props.hideConversationControlPanel
  && !activeConversationIsSystemNotification.value
  && !activeConversationIsRemoteContact.value,
);

const sessionToolbarBehindElasticSpace = computed(() => false);

// 上排占位声明：预览条要用时，操作条让位。用 ref 中转而不是直接读 previewBarVisible，
// 是因为它定义在后面，直接读会在首次求值时撞上未初始化。
const sessionTopRowClaimed = ref(false);

// 会话悬浮操作区：下排工作区 bar 在贴底或背靠弹性空间（不遮挡消息）时显性，离底且消息进入背后时收起。
// 判断顺序：预览条先——留白区里预览条该显示时操作条让位；预览条不显示时操作条照旧在留白区出现。
const showFloatingSessionToolbar = computed(() => {
  if (!supportsFloatingSessionToolbar.value) return false;
  if (sessionTopRowClaimed.value) return false;
  return sessionControlPanelVisible.value || sessionToolbarBehindElasticSpace.value;
});

// 上下两排互斥，且共用同一个位置：切换时先让旧的一排淡出、再让新的一排淡入（顺切），
// 避免两排同时在场交叠淡入淡出，看起来像内容在左右跳动
const SESSION_ROW_FADE_MS = 150;
type SessionFloatRow = "toolbar" | "top" | "none";
const displayedSessionRow = ref<SessionFloatRow>("none");
let sessionRowSwitchTimer: ReturnType<typeof setTimeout> | null = null;

function clearSessionRowSwitchTimer() {
  if (sessionRowSwitchTimer) {
    clearTimeout(sessionRowSwitchTimer);
    sessionRowSwitchTimer = null;
  }
}

watch(
  showFloatingSessionToolbar,
  (visible) => {
    const target: SessionFloatRow = visible ? "toolbar" : "top";
    if (displayedSessionRow.value === target) return;
    clearSessionRowSwitchTimer();
    // 首次落位（或上一次切换还没落位）直接显示，避免初次进场多一次空档
    if (displayedSessionRow.value === "none") {
      displayedSessionRow.value = target;
      return;
    }
    displayedSessionRow.value = "none";
    sessionRowSwitchTimer = setTimeout(() => {
      sessionRowSwitchTimer = null;
      displayedSessionRow.value = target;
    }, SESSION_ROW_FADE_MS);
  },
  { immediate: true },
);

onBeforeUnmount(clearSessionRowSwitchTimer);

// 蛇板展开期间贴底那排的操作条整体淡出：只改透明度与命中，不卸载、不 display:none——
// 操作条里的时间线占位一旦消失，偏移量归零，两排共用的那个按钮会瞬间弹回右上角再弹回来
const sessionToolbarRowVisible = computed(() =>
  displayedSessionRow.value === "toolbar" && !timelineFloatPanelVisible.value,
);

// 上排右侧竖列（回到底部 + 时间线）整体位移，使时间线按钮正好落在操作条右侧那个占位上：
// 按钮两排共用一个，位置对齐后切换状态时它一动不动。偏移量只能运行时测——操作条内容换行、
// 工作区名变长都会移动那个占位，固定像素对不上。
const sessionTopColumnRef = ref<HTMLElement | null>(null);
const sessionTopTimelineBoxRef = ref<HTMLElement | null>(null);
let sessionTopColumnResizeObserver: ResizeObserver | null = null;
const sessionTopColumnOffset = ref({ x: 0, y: 0 });
const sessionTopColumnOffsetStyle = computed(() => {
  const { x, y } = sessionTopColumnOffset.value;
  return x === 0 && y === 0 ? undefined : { transform: `translate(${x}px, ${y}px)` };
});

function updateSessionTopColumnOffset() {
  const toolbarEl = toolbarContainer.value;
  const timelineBox = sessionTopTimelineBoxRef.value;
  if (!toolbarEl || !timelineBox) return;
  const slot = toolbarEl.querySelector<HTMLElement>("[data-toolbar-timeline-slot]");
  if (!slot) return;
  const targetRect = slot.getBoundingClientRect();
  const boxRect = timelineBox.getBoundingClientRect();
  if (targetRect.width <= 0 || boxRect.width <= 0) return;
  const applied = sessionTopColumnOffset.value;
  const next = computeSessionTopColumnOffset(targetRect, boxRect, applied);
  if (isSessionTopColumnOffsetSettled(next, applied)) return;
  sessionTopColumnOffset.value = next;
}

function observeSessionTopColumnOffset() {
  const toolbarEl = toolbarContainer.value;
  if (!toolbarEl || typeof ResizeObserver !== "function") return;
  if (sessionTopColumnResizeObserver) sessionTopColumnResizeObserver.disconnect();
  sessionTopColumnResizeObserver = new ResizeObserver(() => updateSessionTopColumnOffset());
  sessionTopColumnResizeObserver.observe(toolbarEl);
}

watch(displayedSessionRow, () => {
  // 两排切换时按钮整体位移，而蛇板锚在按钮上不会跟着重测，直接收起重开
  closeTimelineFloat();
  void nextTick().then(updateSessionTopColumnOffset);
});
// 操作条右侧那个占位随 canShowTimeline 出现/消失，右侧组会跟着挪位，靠它把偏移重算回来
watch(canShowTimeline, () => {
  void nextTick().then(updateSessionTopColumnOffset);
});
watch(toolbarContainer, () => {
  observeSessionTopColumnOffset();
  void nextTick().then(updateSessionTopColumnOffset);
});

onMounted(() => {
  observeSessionTopColumnOffset();
  void nextTick().then(updateSessionTopColumnOffset);
  window.addEventListener("resize", updateSessionTopColumnOffset);
  void document.fonts?.ready?.then(() => updateSessionTopColumnOffset());
});

onBeforeUnmount(() => {
  sessionTopColumnResizeObserver?.disconnect();
  sessionTopColumnResizeObserver = null;
  window.removeEventListener("resize", updateSessionTopColumnOffset);
});

// 会话悬浮操作区上排（预览条 + 时间线按钮）直接贴容器底边：
// 上下两排不会同时出现（贴底出下排、离底出上排），不需要为上排预留下排的高度；
// 上排时间线按钮靠 sessionTopColumnOffset 与下排操作条里的那个对齐。

const showConversationTodoBar = computed(() => {
  const hasActiveOrPending = normalizedConversationTodos.value.some((item) => item.status === "pending" || item.status === "in_progress");
  if (!hasActiveOrPending) return false;
  return atConversationBottom.value;
});

// ==================== previous user message jump ====================

const scrollNavigationTick = ref(0);
const pendingPreviousUserMessageJumpId = ref("");

function isPreviousUserMessageJumpItem(item: ChatRenderItem | undefined): boolean {
  if (!item || item.kind !== "message") return false;
  const block = item.block;
  if (!block || block.isExtraTextBlock || block.remoteImOrigin) return false;
  const speakerAgentId = String(block.speakerAgentId || "").trim();
  return block.role === "user" || speakerAgentId === "user-persona";
}

function collectPreviousUserMessageJumpTargets() {
  const scrollEl = scrollContainer.value;
  if (!scrollEl || virtualRenderItems.value.length <= 0) return [];
  const viewportTop = scrollEl.scrollTop;
  let firstVisibleIndex = 0;
  if (virtuaRef.value && typeof (virtuaRef.value as any).findItemIndex === "function") {
    try {
      const idx = (virtuaRef.value as any).findItemIndex(viewportTop + 2);
      if (Number.isFinite(idx) && idx >= 0) firstVisibleIndex = idx;
    } catch {}
  } else {
    firstVisibleIndex = Math.max(0, Math.floor(viewportTop / 300));
  }
  const targets: Array<{ index: number; item: ChatRenderItem }> = [];
  for (let index = Math.min(firstVisibleIndex - 1, virtualRenderItems.value.length - 1); index >= 0; index -= 1) {
    const item = virtualRenderItems.value[index];
    if (isPreviousUserMessageJumpItem(item)) targets.push({ index, item });
  }
  return targets;
}

function countPreviousUserMessageJumpItemsBeforeIndex(index: number): number {
  const targetIndex = Math.max(0, Math.min(index, virtualRenderItems.value.length));
  let count = 0;
  for (let currentIndex = targetIndex - 1; currentIndex >= 0; currentIndex -= 1) {
    if (isPreviousUserMessageJumpItem(virtualRenderItems.value[currentIndex])) count += 1;
  }
  return count;
}

const previousUserMessageJumpTargets = computed(() => {
  scrollNavigationTick.value;
  if (!userScrollingUp.value) return [];
  return collectPreviousUserMessageJumpTargets();
});

const previousUserMessageJumpTarget = computed(() => {
  return previousUserMessageJumpTargets.value[0] ?? null;
});

function collectNextUserMessageJumpTargets() {
  const scrollEl = scrollContainer.value;
  if (!scrollEl || virtualRenderItems.value.length <= 0) return [];
  const viewportBottom = scrollEl.scrollTop + scrollEl.clientHeight;
  let lastVisibleIndex = virtualRenderItems.value.length - 1;
  if (virtuaRef.value && typeof (virtuaRef.value as any).findItemIndex === "function") {
    try {
      const idx = (virtuaRef.value as any).findItemIndex(Math.max(0, viewportBottom - 2));
      if (Number.isFinite(idx) && idx >= 0) lastVisibleIndex = idx;
    } catch {}
  }
  if (lastVisibleIndex < 0) return [];
  const targets: Array<{ index: number; item: ChatRenderItem }> = [];
  for (let index = Math.min(lastVisibleIndex + 1, virtualRenderItems.value.length - 1); index < virtualRenderItems.value.length; index += 1) {
    const item = virtualRenderItems.value[index];
    if (isPreviousUserMessageJumpItem(item)) targets.push({ index, item });
  }
  return targets;
}

const nextUserMessageJumpTargets = computed(() => {
  scrollNavigationTick.value;
  if (!userScrollingUp.value) return [];
  return collectNextUserMessageJumpTargets();
});

const nextUserMessageJumpTarget = computed(() => {
  return nextUserMessageJumpTargets.value[0] ?? null;
});

const showJumpToPreviousUserMessage = computed(() =>
  userScrollingUp.value && !!previousUserMessageJumpTarget.value,
);

const showJumpToNextUserMessage = computed(() =>
  userScrollingUp.value && !!nextUserMessageJumpTarget.value,
);

// ==================== tool review ====================

const {
  toolReviewPanelOpen, toolReviewBatches, toolReviewCurrentBatchKey,
  toolReviewDetailMap, toolReviewSegmentMap, toolReviewDetailLoadingCallId, toolReviewReviewingCallId,
  toolReviewBatchReviewingKey, toolReviewSubmittingBatchKey, toolReviewErrorText,
  setToolReviewCurrentBatchKey,
  loadToolReviewItemDetail, runToolReviewForCall, runToolReviewForBatch,
  submitToolReviewCode, listToolReviewCommitOptions,
} = useChatToolReviewHandlers({
  activeConversationId: toRef(props, "activeConversationId"),
  toolReviewRefreshTick: toRef(props, "toolReviewRefreshTick"),
  initialPanelOpen: toRef(props, "initialToolReviewPanelOpen"),
  activeTab: toolReviewSidebarActiveTab,
  homePreviewActive: computed(() => props.chatRightPanelMode === "home"),
  t, syncViewportMetrics,
  onRefreshMessage: (payload) => emit("refreshToolReviewMessage", payload),
  onToolReviewPanelOpenChange: (open) => emit("toolReviewPanelOpenChange", open),
});
const effectiveToolReviewPanelOpen = computed(() => toolReviewPanelOpen.value);

watch(
  () => [effectiveToolReviewPanelOpen.value, props.chatRightPanelMode] as const,
  ([panelOpen, mode]) => {
    if (!panelOpen || mode !== "reader") clearFileReaderContextReferences();
  },
);

async function openChatReaderDirectoryIfEmpty() {
  await nextTick();
  await nextTick();
  if (!effectiveToolReviewPanelOpen.value || props.chatRightPanelMode !== "reader") return;
  const panel = chatReaderPanelRef.value;
  const workspaceRootPath = String(props.currentWorkspaceRootPath || "").trim();
  if (!panel || !workspaceRootPath) return;
  if (String(panel.activePath || "").trim()) return;
  if (String(panel.directoryRootPath || "").trim()) return;
  await panel.openDirectoryTree(workspaceRootPath);
}

async function refreshChatReaderDirectoryOnWorkspaceChange() {
  await nextTick();
  await nextTick();
  if (!effectiveToolReviewPanelOpen.value || props.chatRightPanelMode !== "reader") return;
  const panel = chatReaderPanelRef.value as unknown as { directoryRootPath: string; asideMode: string; openDirectoryTree: (path: string, opts?: { switchToFiles?: boolean }) => Promise<boolean> } | null;
  const workspaceRootPath = String(props.currentWorkspaceRootPath || "").trim();
  if (!panel || !workspaceRootPath) return;
  // 目录树已展开时才跟随；未展开保持关闭，已在目标路径则不重复加载
  const currentRoot = String(panel.directoryRootPath || "").trim();
  if (!currentRoot) return;
  const norm = (p: string) => String(p || "").trim().replace(/\\/g, "/").replace(/\/$/, "").toLowerCase();
  if (norm(currentRoot) === norm(workspaceRootPath)) return;
  // switchToFiles: false 保证若当前处于 Git 面板时不会强制跳回文件目录
  await panel.openDirectoryTree(workspaceRootPath, { switchToFiles: false });
}

function selectChatRightPanelMode(mode: ChatRightPanelMode) {
  emit("update:chatRightPanelMode", mode);
  emit("toolReviewPanelOpenChange", true);
  // 覆盖模式下不主动展开目录，避免遮挡对话主体
  if (mode === "reader" && !rightPaneOverlay.value) {
    void openChatReaderDirectoryIfEmpty();
  }
}

// ==================== 议题看板 ====================
// 磁贴与看板页读同一份数据：磁贴只截取需要用户参与的四类，
// 不单独维护聚合值，也不为磁贴另开接口。
const kanbanConversationId = computed(() => String(props.activeConversationId || "").trim());
const {
  issues: kanbanIssues,
  loading: kanbanLoading,
  loaded: kanbanLoaded,
  errorText: kanbanErrorText,
  approvingNumber: kanbanApprovingNumber,
  reload: reloadKanbanBoard,
  approve: approveKanbanIssue,
} = useKanbanBoard(kanbanConversationId);

/** 拉不到看板说明当前会话不是队长主会话：磁贴直接不出现，而不是显示一个空看板。 */
const kanbanVisible = computed(() => Boolean(kanbanLoaded.value && kanbanConversationId.value));

function openKanbanBoard() {
  selectChatRightPanelMode("kanban");
}

// 议题引用：磁贴点编号与输入框 `#` 选中都落到这一份列表，只保留一个实现。
const {
  references: kanbanIssueReferences,
  attachKanbanIssueReference,
  removeKanbanIssueReference,
  clearKanbanIssueReferences,
  buildKanbanIssueReferenceBlocks,
} = useKanbanIssueReferences();

watch(
  () => [String(props.chatRightPanelMode || ""), kanbanConversationId.value] as const,
  ([mode, conversationId]) => {
    if (!conversationId) return;
    // 主页要磁贴、看板页要全量，两处都是同一个读取入口
    if (mode === "home" || mode === "kanban") {
      void reloadKanbanBoard();
    }
  },
  { immediate: true },
);

watch(
  () => [String(props.activeConversationId || "").trim(), String(props.currentWorkspaceRootPath || "").trim()] as const,
  ([conversationId, nextRoot], [prevConversationId, prevRoot]) => {
    // 会话切换会重拉会话级工作区路径，这是正常变化，不应触发目录强刷
    if (conversationId !== prevConversationId) return;
    if (!nextRoot || nextRoot === prevRoot) return;
    void refreshChatReaderDirectoryOnWorkspaceChange();
  },
);

function selectMonitorPanelTab(key: string) {
  if (key !== "delegate" && key !== "tasks" && key !== "tools" && key !== "fastRequests") return;
  emit("update:chatMonitorPanelMode", key);
}

/** 首页队员状态表：字段由宿主容器从队员会话派生，这里只做空值与类型收敛 */
const teamMembers = computed(() =>
  (Array.isArray(props.teamMembers) ? props.teamMembers : [])
    .map((member) => ({
      conversationId: String(member?.conversationId || "").trim(),
      title: String(member?.title || "").trim(),
      summaryTitle: String(member?.summaryTitle || "").trim(),
      updatedAt: String(member?.updatedAt || "").trim(),
      lastMessageAt: String(member?.lastMessageAt || "").trim(),
      teamName: String(member?.teamName || "").trim(),
      agentId: String(member?.agentId || "").trim(),
      agentName: String(member?.agentName || "").trim(),
      runtimeState: String(member?.runtimeState || "").trim(),
      activeIssue: member?.activeIssue,
      memberState: String(member?.memberState || "").trim(),
    }))
    .filter((member) => member.conversationId),
);

/** 主页「工作目录」入口卡：切到阅读器面板并展开工作区目录树 */
async function openHomeWorkspaceDirectory() {
  const workspaceRootPath = String(props.currentWorkspaceRootPath || "").trim();
  if (!workspaceRootPath) return;
  selectChatRightPanelMode("reader");
  await nextTick();
  await nextTick();
  await openDirectoryInReader(workspaceRootPath);
}

/** 主页「已打开文件」大卡里的文件：切到阅读器面板并打开该文件 */
async function openHomeFile(path: string) {
  const target = String(path || "").trim();
  if (!target) return;
  selectChatRightPanelMode("reader");
  await nextTick();
  const panel = chatReaderPanelRef.value;
  if (!panel) return;
  // 面板首次挂载会异步恢复上次会话，等它完成再打开目标文件，避免被旧会话覆盖
  await panel.whenSessionRestored?.();
  await panel.openPath(target);
  panel.closeDirectoryTree();
}

/** 图片全屏弹层里的「在右栏打开」：把同一份内联数据交给文件阅读器，不重新下载一遍 */
async function openPreviewImageInRightPanel() {
  const dataUrl = String(imagePreviewDataUrl.value || "").trim();
  if (!dataUrl) return;
  const localPath = String(imagePreviewLocalPath.value || "").trim();
  const title = localPath ? localPath.split(/[\\/]/).filter(Boolean).pop() || "" : "";
  closeImagePreview();
  selectChatRightPanelMode("reader");
  await nextTick();
  await nextTick();
  const panel = chatReaderPanelRef.value;
  if (!panel) return;
  await panel.whenSessionRestored?.();
  await panel.openImageInReader({ dataUrl, path: localPath, title });
}

/**
 * 聊天里交互组件卡片的「在右栏打开」：把卡片渲染的同一份 html 源码交给阅读器。
 *
 * 走 chat-html: 伪路径建内存标签，不落盘、不读盘；右栏用同一份 InteractiveHtmlSandbox 渲染，
 * 与 .html 文件在右栏打开是同一套预览。源码/预览切换沿用 html kind 的既有逻辑。
 */
async function openHtmlInRightPanel(request: ChatHtmlReaderRequest) {
  const code = String(request?.code ?? "");
  if (!code.trim()) return;
  selectChatRightPanelMode("reader");
  await nextTick();
  await nextTick();
  const panel = chatReaderPanelRef.value;
  if (!panel) return;
  await panel.whenSessionRestored?.();
  await panel.openHtmlInReader({ code, title: request?.title });
}

// 顶层下发「把这段 html 送到右栏」：四层链路（ChatMessageItem → AppMarkdownRenderer → CodeBlock
// → InteractiveHtmlSandbox）逐层传 props 会让后续每加一个入口都要再穿一遍，只走 provide/inject。
// label 用 getter 读 i18n，切换语言时卡片上的按钮文案跟着变。
provide(CHAT_HTML_READER_KEY, {
  get label() { return t("chat.openInRightPanel"); },
  open: (request) => { void openHtmlInRightPanel(request); },
});

/** 主页 Git 卡的「查看剩余 N 个文件」：切到阅读器面板并打开 Git 更改列表 */
async function openHomeGitChanges() {
  selectChatRightPanelMode("reader");
  await nextTick();
  await nextTick();
  const panel = chatReaderPanelRef.value;
  if (!panel) return;
  await panel.whenSessionRestored?.();
  await panel.openGitPanel();
}

/** 首页环境卡的「最新提交」：切到阅读器面板并直接落在提交标签页 */
async function openHomeGitCommits() {
  selectChatRightPanelMode("reader");
  await nextTick();
  await nextTick();
  const panel = chatReaderPanelRef.value;
  if (!panel) return;
  await panel.whenSessionRestored?.();
  await panel.openGitPanel("commits");
}

/** 首页环境卡的分支切换失败提示：与 Git 面板一致的阻断式提示 */
function handleHomeGitError(message: string) {
  const text = String(message || "").trim();
  if (!text) return;
  window.alert(text);
}

/** 主页监控卡片里的概览条目点击后：切到监控面板的对应 tab。 */
function openMonitorTabFromHome(tab: ChatMonitorPanelMode) {
  emit("update:chatMonitorPanelMode", tab);
  selectChatRightPanelMode("monitor");
}

// ==================== 右侧主页预览 ====================

type HomeFileItem = { path: string; label: string };

const EMPTY_HOME_FILE_PREVIEW = {
  openFiles: [] as HomeFileItem[],
};

/** 文件项：path 保留完整路径用于打开与关闭，label 只用于展示 */
function toHomeFileItem(path: string): HomeFileItem {
  const normalized = normalizePath(path);
  const parts = normalized.split("/").filter(Boolean);
  return { path: normalized, label: parts.pop() || normalized };
}

/** 已打开文件与会话级持久化同步，主页不常驻阅读器面板，直接读它的会话状态 */
function readHomeOpenFiles(): { openFiles: HomeFileItem[] } {
  const sessionKey = String(chatFileReaderSessionKey.value || "").trim();
  if (!sessionKey) return { ...EMPTY_HOME_FILE_PREVIEW };
  const state = readFileReaderSessionState(sessionKey, legacyChatFileReaderSessionKey.value);
  return {
    openFiles: listSessionFilePaths(state).map(toHomeFileItem),
  };
}

/** 主页文件磁贴的关闭按钮：此时阅读器面板未挂载，直接改会话状态再刷新卡片墙 */
function closeHomeFile(path: string) {
  const sessionKey = String(chatFileReaderSessionKey.value || "").trim();
  if (!sessionKey) return;
  const state = readFileReaderSessionState(sessionKey, legacyChatFileReaderSessionKey.value);
  writeFileReaderSessionState(sessionKey, removeFilePathFromSessionState(state, path));
  homeFilePreview.value = { ...homeFilePreview.value, ...readHomeOpenFiles() };
}

const homeFilePreview = ref({
  openFiles: [] as HomeFileItem[],
});

/** 主页预览计划卡：列出当前会话最后呈现过的计划 */
const latestHomePlan = computed<LatestPlanSummary | null>(() => {
  const blocks = props.messageBlocks || [];
  for (let i = blocks.length - 1; i >= 0; i--) {
    const block = blocks[i];
    if (block.isExtraTextBlock) continue;
    const path = String(block.planCard?.path || "").trim();
    if (!path) continue;
    return { path };
  }
  return null;
});

// 卡片墙的 Git 卡与文件阅读器 Git 面板共用同一份状态：仓库由面板选定，卡片墙跟着变
const {
  currentBranch: homeGitBranch,
  statusEntries: homeGitStatusEntries,
  stagedTotal: homeGitStagedTotal,
  unstagedTotal: homeGitUnstagedTotal,
  setRepoRoot: setHomeGitRepoRoot,
  loadStatus: loadHomeGitStatus,
  loadRecentCommits: loadHomeGitRecentCommits,
  recentCommits: homeGitRecentCommitsRef,
  repoRoot: homeGitRepoRoot,
  discoverRepoRoot: discoverHomeGitRepoRoot,
  readRememberedRepoRoot: readHomeGitRepoMemory,
  acquire: acquireHomeGitStatus,
  release: releaseHomeGitStatus,
  isPanelActive: isHomeGitPanelActive,
} = useWorkspaceGitStatus();

const homeGitChangeCount = computed(() => {
  const visible = homeGitStatusEntries.value.length;
  return visible || homeGitStagedTotal.value + homeGitUnstagedTotal.value;
});
/** 卡片墙提交卡只消费窄类型，避免把面板用的完整条目透传下去 */
const homeGitRecentCommits = computed(() =>
  homeGitRecentCommitsRef.value.map((entry) => ({
    hash: String(entry?.hash || ""),
    message: String(entry?.message || ""),
  })),
);

let homeGitConsuming = false;

/** 只在右栏卡片墙模式占用共享状态源：离开时归还，避免空挂仓库监听 */
function syncHomeGitConsume(mode: string) {
  const consuming = mode === "home";
  if (consuming === homeGitConsuming) return;
  homeGitConsuming = consuming;
  if (!consuming) {
    releaseHomeGitStatus();
    return;
  }
  acquireHomeGitStatus();
  // 离开卡片墙期间仓库监听会停掉，切回来时仓库没换的话 syncHomeGitRepo 不会重载，
  // 所以在这里补一次：覆盖离开期间发生的提交与外部改动
  if (homeGitRepoRoot.value) {
    void loadHomeGitStatus();
    void loadHomeGitRecentCommits();
  }
}

let homeGitWorkspaceKey = "";

/**
 * 卡片墙的仓库来源：Git 面板在场时完全跟随面板（它负责选定仓库与刷新）；
 * 面板不在场时按当前会话工作区解析默认仓库——切会话或换工作区要重解析，
 * 仅切换右栏模式则保留面板最后选中的仓库，避免切回来又跳回默认仓库。
 * 解析结果不直接用：先查这个会话是否记住过别的仓库（同一个 sessionKey 由面板写入），
 * 记住的仓库仍在本次探测到的列表里就用它，否则才回落默认仓库。
 */
async function syncHomeGitRepo() {
  if (isHomeGitPanelActive()) return;
  const workspace = String(effectiveSessionRootPath.value || props.currentWorkspaceRootPath || "").trim();
  const workspaceChanged = workspace !== homeGitWorkspaceKey;
  homeGitWorkspaceKey = workspace;
  if (!workspaceChanged) return;
  const discovered = await discoverHomeGitRepoRoot(workspace);
  if (!homeGitConsuming) return;
  // 解析期间会话/工作区可能已经切走，过期结果直接丢弃，
  // 否则慢返回的那次会把另一个会话的仓库写进共享状态（卡片墙与 Git 面板共用同一份）
  if (homeGitWorkspaceKey !== workspace) return;
  const rememberedRoot = readHomeGitRepoMemory(
    String(chatFileReaderSessionKey.value || "").trim(),
    discovered.repoPaths,
  );
  const root = rememberedRoot || discovered.defaultRepoRoot;
  setHomeGitRepoRoot(root);
  if (root) void loadHomeGitStatus();
}

watch(
  () => [
    String(props.activeConversationId || "").trim(),
    props.chatRightPanelMode,
    String(effectiveSessionRootPath.value || props.currentWorkspaceRootPath || "").trim(),
  ] as const,
  ([, mode]) => {
    syncHomeGitConsume(mode);
    if (mode !== "home") return;
    homeFilePreview.value = { ...homeFilePreview.value, ...readHomeOpenFiles() };
    void syncHomeGitRepo();
  },
  { immediate: true },
);

// ==================== delegate status ====================

const {
  delegateStatuses, delegateStatusesErrorText,
  openDelegateArchiveDetail, abortDelegate,
} = useDelegateStatus({
  activeConversationId: toRef(props, "activeConversationId"),
  // 委托状态：打开会话即拉取（工作区 bar 常驻展示活跃委托，不依赖监控面板 delegate tab）
  panelOpen: computed(() => !!String(props.activeConversationId || "").trim()),
  enabled: computed(() => true),
});

// ==================== background shell status ====================

const {
  backgroundShells,
  interruptShell,
} = useBackgroundShell({
  activeConversationId: toRef(props, "activeConversationId"),
  // 后台任务：工作区监控 bar 常驻展示运行数量，靠事件广播驱动刷新
  active: computed(() => true),
});

const runningShellCount = computed(() =>
  backgroundShells.value.filter((task) => String(task.status || "").trim() === "running").length,
);

// ==================== running task count for monitor bar ====================

const runningTaskCount = ref(0);
/** 主页监控卡片预览用的运行中任务条目，与计数复用同一次 task.list 请求 */
const homePanelRunningTasks = ref<TaskEntry[]>([]);
let runningTaskRequestSeq = 0;
let runningTaskRequestConversationId = "";

function isTaskActive(task: TaskEntry | null | undefined): boolean {
  const state = String(task?.completionState || "").trim().toLowerCase();
  return Boolean(state) && state !== "completed" && state !== "failed_completed";
}

async function refreshRunningTaskCount() {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) {
    runningTaskRequestSeq += 1;
    runningTaskRequestConversationId = "";
    runningTaskCount.value = 0;
    homePanelRunningTasks.value = [];
    return;
  }
  const seq = ++runningTaskRequestSeq;
  runningTaskRequestConversationId = conversationId;
  try {
    const tasks = await invokeTauri<TaskEntry[]>("task.list", {});
    if (seq !== runningTaskRequestSeq || runningTaskRequestConversationId !== String(props.activeConversationId || "").trim()) return;
    const activeTasks = (Array.isArray(tasks) ? tasks : []).filter((task) =>
      isTaskActive(task)
      && String(task?.conversationId || "").trim() === conversationId,
    );
    runningTaskCount.value = activeTasks.length;
    homePanelRunningTasks.value = activeTasks;
  } catch {
    if (seq !== runningTaskRequestSeq || runningTaskRequestConversationId !== String(props.activeConversationId || "").trim()) return;
    runningTaskCount.value = 0;
    homePanelRunningTasks.value = [];
  }
}

type TaskChangedMonitorPayload = { domain?: string };

function handleRunningTaskRefreshEvent(payload: TaskChangedMonitorPayload | null | undefined) {
  if (String(payload?.domain || "").trim() !== "task") return;
  void refreshRunningTaskCount();
}

watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    void refreshRunningTaskCount();
  },
  { immediate: true },
);

watch(
  () => props.chatRightPanelMode,
  (mode) => {
    if (mode === "home") {
      void refreshRunningTaskCount();
    }
  },
);

let unlistenTaskChanged: (() => void) | null = null;
let unlistenTaskRecovered: (() => void) | null = null;
let unlistenRoundFinished: (() => void) | null = null;
let taskPollTimer: ReturnType<typeof window.setInterval> | null = null;

function syncTaskPollTimer() {
  const hasActive = runningTaskCount.value > 0 || homePanelRunningTasks.value.length > 0;
  if (hasActive && taskPollTimer == null && typeof window !== "undefined") {
    taskPollTimer = window.setInterval(() => {
      void refreshRunningTaskCount();
    }, 15000);
  } else if (!hasActive && taskPollTimer != null) {
    window.clearInterval(taskPollTimer);
    taskPollTimer = null;
  }
}

watch(runningTaskCount, () => syncTaskPollTimer());

onMounted(() => {
  unlistenTaskChanged = onTransportNotification("monitor.changed", handleRunningTaskRefreshEvent);
  unlistenRoundFinished = onTransportNotification("chat.roundFinished", () => {
    void refreshRunningTaskCount();
  });
  unlistenTaskRecovered = onTransportRecovered(() => {
    void refreshRunningTaskCount();
  });
});

onBeforeUnmount(() => {
  unlistenTaskChanged?.();
  unlistenTaskChanged = null;
  unlistenRoundFinished?.();
  unlistenRoundFinished = null;
  unlistenTaskRecovered?.();
  unlistenTaskRecovered = null;
  if (taskPollTimer != null) {
    window.clearInterval(taskPollTimer);
    taskPollTimer = null;
  }
});

// ==================== panes ====================

const panesCleanupFns: Array<() => void> = [];
const {
  leftPaneInLayout, rightPaneInLayout,
  leftPaneOverlay, rightPaneOverlay, leftPaneVisibleWidth, rightPaneVisibleWidth, activePaneResizeSide,
  collapsePreviewSide, collapsePreviewWidth,
  isPaneTransitioning, notifyPaneTransitionStart, notifyPaneTransitionEnd,
  startPaneResize, adjustPaneWidthByKeyboard,
} = useChatPanes({
  chatLayoutRoot, toolReviewPanelOpen: effectiveToolReviewPanelOpen,
  showSideConversationList,
  syncViewportMetrics,
  onPaneWidthsChange: (left, right) => emit("sidePanelWidthsChange", { leftWidth: left, rightWidth: right }),
  onPaneWidthsCommit: (left, right) => emit("sidePanelWidthsCommit", { leftWidth: left, rightWidth: right }),
  onPaneCloseRequest: (side) => {
    if (side === "left") {
      emit("sideConversationListVisibleChange", false);
      return;
    }
    emit("toolReviewPanelOpenChange", false);
  },
  onBeforeUnmountCleanup: (fn) => panesCleanupFns.push(fn),
});

// --- push 动画：仅 inLayout 时启用，overlay 瞬切；Transition css 由快照驱动，避免 leave 时已置 false 导致误判 ---
const leftPaneLayoutSnapshot = ref(showSideConversationList.value ? leftPaneInLayout.value : false);
const rightPaneLayoutSnapshot = ref(effectiveToolReviewPanelOpen.value ? rightPaneInLayout.value : false);

watch([showSideConversationList, leftPaneInLayout], ([open, inLayout]) => {
  if (open) leftPaneLayoutSnapshot.value = inLayout;
}, { flush: "sync" });

watch([effectiveToolReviewPanelOpen, rightPaneInLayout], ([open, inLayout]) => {
  if (open) rightPaneLayoutSnapshot.value = inLayout;
}, { flush: "sync" });

// 初始化时若已打开，同步一次（appear false 不动画，但需同步快照以便首次关闭能正确判定）
leftPaneLayoutSnapshot.value = showSideConversationList.value ? leftPaneInLayout.value : false;
rightPaneLayoutSnapshot.value = effectiveToolReviewPanelOpen.value ? rightPaneInLayout.value : false;

const leftPaneTransitionCssEnabled = computed(() => leftPaneLayoutSnapshot.value);
const rightPaneTransitionCssEnabled = computed(() => rightPaneLayoutSnapshot.value);

// css 由快照决定：push 动画，overlay 瞬切；拖拽时 Transition 未激活不受影响；isPaneTransitioning 仅用于锁测量

function handleLeftPaneBeforeTransition() {
  if (!leftPaneLayoutSnapshot.value) return;
  notifyPaneTransitionStart();
}

function handleLeftPaneAfterTransition() {
  if (isPaneTransitioning.value) notifyPaneTransitionEnd();
}

function handleRightPaneBeforeTransition() {
  if (!rightPaneLayoutSnapshot.value) return;
  notifyPaneTransitionStart();
}

function handleRightPaneAfterTransition() {
  if (isPaneTransitioning.value) notifyPaneTransitionEnd();
}

function closeOverlayPanes() {
  if (leftPaneOverlay.value) emit("sideConversationListVisibleChange", false);
  if (rightPaneOverlay.value) emit("toolReviewPanelOpenChange", false);
}

// 打开右侧面板 / 切到 reader / 工作区变化时：无打开文件则自动展开目录；工作区变化时刷新已展开的目录树。
// 覆盖模式（面板浮在内容上）下不主动展开目录，避免遮挡对话主体，由用户自行决定。
watch(
  () => [
    effectiveToolReviewPanelOpen.value,
    props.chatRightPanelMode,
    rightPaneOverlay.value,
    String(props.activeConversationId || "").trim(),
  ] as const,
  ([panelOpen, mode, overlay]) => {
    if (!panelOpen || mode !== "reader" || overlay) return;
    void openChatReaderDirectoryIfEmpty();
  },
);

// ==================== scroll orchestration ====================

const {
  onConversationScroll: handleConversationScrollBase,
  onConversationWheel,
  handleJumpToBottom,
  armProgrammaticScrollPaginationSuppression,
} = useChatScrollOrchestration({
  scrollContainer, chatScrollbarRef: chatScrollbarRef as Ref<{ updateThumb: () => void; hide?: () => void } | null>,
  prepareBottomAlignmentLayout,
  onScroll, scheduleVirtualMeasure,
  alignLatestOwnMessageToTop,
  startFollowBottom,
  resetConversationToBottom: resetVirtualizerAtConversationBottom,
  resolveManualScrollToBottomBehavior,
  olderHistoryCorrectionAllowed,
  props: {
    hasMoreHistory: toRef(props, "hasMoreHistory"), loadingOlderHistory: toRef(props, "loadingOlderHistory"),
    chatting: toRef(props, "chatting"), conversationBusy: conversationInteractionBusy, frozen: toRef(props, "frozen"),
    activeConversationId: toRef(props, "activeConversationId"),
    conversationScrollToBottomRequest: toRef(props, "conversationScrollToBottomRequest"),
    scrollToBottomBehavior: toRef(props, "scrollToBottomBehavior"),
    renderItems: virtualRenderItems,
  },
  emit: { loadOlderHistory: () => emit("loadOlderHistory"), jumpToConversationBottom: () => emit("jumpToConversationBottom") },
});

function handleConversationScroll() {
  handleConversationScrollBase();
  scrollNavigationTick.value += 1;
}

function handleConversationWheelInput(event: WheelEvent) {
  if (event.shiftKey) {
    handleShiftWheel(event);
    return;
  }
  noteWheelScrollIntent();
  onConversationWheel(event);
}

// harness 同语义：滚动键先打断进行中的 smooth，再按正常意图归因；
// 非滚动键不碰意图窗口，避免输入框打字误翻跟随。
const SCROLL_INTENT_KEYS = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "]);

function handleConversationKeydownInput(event: KeyboardEvent) {
  if (event.defaultPrevented) return;
  if (!SCROLL_INTENT_KEYS.has(event.key)) return;
  noteKeyboardScrollIntent();
}

// ==================== bottom follow (intent-driven) ====================

const chatContentRoot = ref<HTMLElement | null>(null);
let contentResizeObserver: ResizeObserver | null = null;
let lastChatContentHeight = 0;

// 跟随模式下内容尺寸变化（流式增长、气泡变高）时同步贴底。
// virtua 不会在内容增长时自动维持贴底，这里补上；未进入跟随则保持视口不动。
// 另外要求视口确实仍贴着底：补载历史等程序化滚动会把 followBottom 误置真，
// 此时视口停在上方，若照样下拉会把用户从历史位置直接拽到最底。
// 容差取自实测：跟随状态下距底距离基本为 0，偶发瞬态最大 54px。
const PIN_TO_BOTTOM_TOLERANCE_PX = 64;
// 离散插入（引导/队列新气泡）与流式逐字增长的区分线：
// 实测引导气泡单次增高约 67px，自己发言平滑路径单帧最大 11px，流式每 tick 增量更小。
// 超过该线走平滑滚到底（与 own_top/手动回到底部同路），以下走瞬时跟随避免流式滞后。
const PIN_SMOOTH_DISCRETE_DELTA_PX = 40;

function pinChatToBottomWhileFollowing(contentDelta = 0) {
  if (!followBottom.value) return;
  const el = scrollContainer.value;
  if (!el) return;
  const distanceToBottom = el.scrollHeight - (el.scrollTop + el.clientHeight);
  if (distanceToBottom > PIN_TO_BOTTOM_TOLERANCE_PX) return;
  const before = el.scrollTop;
  const targetTop = el.scrollHeight;
  if (contentDelta > PIN_SMOOTH_DISCRETE_DELTA_PX) {
    // 离散插入：平滑滚到底；同时强制刷新滚动条 extent，否则 300ms 缓存过期时 thumb 会跳一下。
    // harness 同语义：起飞前登记 animating target，到岸由 onScroll 按贴底阈值清掉；
    // reduced-motion 下走瞬时定位，不登记动画。
    const reduceMotion = prefersReducedMotionTimeline.value
      || (typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches);
    if (reduceMotion) {
      el.scrollTop = el.scrollHeight;
      chatScrollbarRef.value?.updateThumb(true);
    } else {
      setFollowAnimating(targetTop);
      el.scrollTo({ top: targetTop, behavior: "smooth" });
      chatScrollbarRef.value?.updateThumb(true);
    }
  } else {
    el.scrollTop = el.scrollHeight;
    chatScrollbarRef.value?.updateThumb();
  }
  if (Math.abs(el.scrollTop - before) > 1 || contentDelta > PIN_SMOOTH_DISCRETE_DELTA_PX) {
    probeChatScroll("跟随贴底", {
      before: Math.round(before),
      after: Math.round(el.scrollTop),
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
    });
  }
}

watch(chatContentRoot, (el, _prev, onCleanup) => {
  contentResizeObserver?.disconnect();
  contentResizeObserver = null;
  lastChatContentHeight = 0;
  if (!el || typeof ResizeObserver === "undefined") return;
  contentResizeObserver = new ResizeObserver(() => {
    const height = el.offsetHeight;
    const contentDelta = lastChatContentHeight > 0 ? height - lastChatContentHeight : 0;
    lastChatContentHeight = height;
    if (contentDelta > 0) pinChatToBottomWhileFollowing(contentDelta);
    scrollNavigationTick.value += 1;
  });
  contentResizeObserver.observe(el);
  onCleanup(() => {
    contentResizeObserver?.disconnect();
    contentResizeObserver = null;
    lastChatContentHeight = 0;
  });
});

// 手动「回到底部」的滚动行为：距底部在当前可视轮次 VIRTUAL_SMOOTH_DISTANCE 个以内走平滑，否则瞬移
// 这里必须用轮次容器数，和 currentFirstVisibleVirtualIndex() 同一单位；
// 拿 virtualRenderItems.length（消息项数）去比会永远超出阈值，退化成一律瞬移。
function resolveManualScrollToBottomBehavior(): "auto" | "smooth" {
  const len = turnContainers.value.length;
  return len > 0 && resolveVirtualSmooth(len - 1, "smooth") ? "smooth" : "auto";
}

// 「回到底部」是显式贴底意图：进入跟随后再执行定位滚动
function handleJumpToBottomWithFollow() {
  startFollowBottom();
  handleJumpToBottom(resolveManualScrollToBottomBehavior());
}

// 思维链预览：数据源取当前回合最后一条助理块的内容源（contentBlocks），按节点顺序交给预览条。
// activityItems 在流式期 text 为空，不能作为流式预览来源。
// 必须停在最新一条助理消息上，即使它此刻 contentBlocks 还是空的：新回合首个旁白出现前
// contentBlocks 为空，若继续往前找非空块，会回退取到「上一条助理消息」的内容，正文行跟着串台。
const thinkingPreviewBlocks = computed<AssistantStreamBlock[]>(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (String(block.role || "") !== "assistant") return [];
    return block.contentBlocks || [];
  }
  return [];
});


// 非流式预览：取最新一条助理消息的正文。
// 不能只看 contentBlocks——历史消息常常没有这个字段，会取成空。
const idlePreviewText = computed(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (String(block.role || "") !== "assistant") continue;
    const text = String(block.text || "").trim();
    if (text) return text;
  }
  return "";
});

// 预览条：离底、滚动已静止三秒才显示；思维链已展开时用户正在看思维链，不再预览。
// 不设「已读/未读」判据：预览条只表达「现在正在产出什么」，不追问用户看没看过。
// 也不等「上排排位」：排位切换有 150ms 空窗，那一瞬间两排都不显示，看起来就像它卡住不出来。
// 与操作条的先后由 sessionTopRowClaimed 表达——预览条该显示时操作条让位，两者不同时出现。
const previewBarVisible = computed(() =>
  !atConversationBottom.value
  && scrollIdle.value
  && !thinkingPreviewCollapsed.value
  && !chatStatusBanner.value
  && !timelinePanelOpen.value
  && !timelineFloatPanelVisible.value
);

// 预览条一显示就占住那一排，操作条据此让位；预览条退回（贴底、或内容不满足）时操作条立刻接班。
watchEffect(() => {
  sessionTopRowClaimed.value = previewBarVisible.value;
});

// 回到底部圆钮：与预览条同一排，离底就出现
const jumpToBottomButtonVisible = computed(() =>
  !atConversationBottom.value
  && !timelinePanelOpen.value
  && !timelineFloatPanelVisible.value
  && displayedSessionRow.value === "top",
);

// 预览条正文行前的头像：与聊天气泡同源，取最新一条助理消息的人格头像
const previewAvatarUrl = computed(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (String(block.role || "") !== "assistant") continue;
    const speakerId = String(block.speakerAgentId || "").trim();
    return speakerId ? String(props.personaAvatarUrlMap?.[speakerId] || "").trim() : "";
  }
  return "";
});

// 正在流式的那条助理消息：预览条按它的思维链展开状态决定形态
const latestAssistantBlockId = computed(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (String(block.role || "") !== "assistant") return "";
    return String(block.id || "");
  }
  return "";
});

// 活动面板的开合是消息组件内部的状态，父级读不到，只能由它上报后在这里记账。
// 多条消息可以同时开着，这里只记录哪些块开着，供流式预览条判断。
const expandedActivityBlockIds = ref<Record<string, boolean>>({});

function handleActivityToggle(payload: { blockId: string; open: boolean }) {
  const blockId = String(payload.blockId || "").trim();
  if (!blockId) return;
  const next = { ...expandedActivityBlockIds.value };
  if (payload.open) next[blockId] = true;
  else delete next[blockId];
  expandedActivityBlockIds.value = next;
}

// 最新一条助理消息的活动面板开着：用户正在看思考与工具，预览条不再预览，退化成回到底部。
const thinkingPreviewCollapsed = computed(
  () =>
    latestAssistantBlockId.value !== "" &&
    expandedActivityBlockIds.value[latestAssistantBlockId.value] === true,
);

function scrollToUserMessageTarget(target: { index: number; item: ChatRenderItem }) {
  if (!target) return;
  armProgrammaticScrollPaginationSuppression();
  scheduleVirtualMeasure();
  scrollVirtualizerToIndex(target.index, { align: "start", behavior: "smooth" });
  void nextTick(() => {
    chatScrollbarRef.value?.updateThumb();
    scrollNavigationTick.value += 1;
  });
}

function requestOlderHistoryBeforePreviousUserJump(target: { index: number; item: ChatRenderItem }): boolean {
  if (!props.hasMoreHistory) return false;
  pendingPreviousUserMessageJumpId.value = String(target.item.id || "").trim();
  if (!props.loadingOlderHistory) {
    emit("loadOlderHistory");
  }
  return true;
}

async function continuePendingPreviousUserMessageJump() {
  const pendingId = String(pendingPreviousUserMessageJumpId.value || "").trim();
  if (!pendingId || props.loadingOlderHistory) return;
  await nextTick();
  const index = virtualRenderItems.value.findIndex((item) => String(item.id || "").trim() === pendingId);
  if (index < 0) {
    pendingPreviousUserMessageJumpId.value = "";
    return;
  }
  const item = virtualRenderItems.value[index];
  if (!isPreviousUserMessageJumpItem(item)) {
    pendingPreviousUserMessageJumpId.value = "";
    return;
  }
  const target = { index, item };
  if (props.hasMoreHistory && countPreviousUserMessageJumpItemsBeforeIndex(index) < 1) {
    requestOlderHistoryBeforePreviousUserJump(target);
    return;
  }
  pendingPreviousUserMessageJumpId.value = "";
  scrollToUserMessageTarget(target);
}

function handleJumpToPreviousUserMessage() {
  const target = previousUserMessageJumpTarget.value;
  if (!target) return;
  if (props.hasMoreHistory && previousUserMessageJumpTargets.value.length < 2) {
    requestOlderHistoryBeforePreviousUserJump(target);
    return;
  }
  scrollToUserMessageTarget(target);
}

function handleJumpToNextUserMessage() {
  const target = nextUserMessageJumpTarget.value;
  if (!target) return;
  scrollToUserMessageTarget(target);
}

watch(
  () => props.loadingOlderHistory,
  (loading, wasLoading) => {
    if (loading || !wasLoading) return;
    void continuePendingPreviousUserMessageJump();
  },
);

watch(
  () => props.activeConversationId,
  () => {
    pendingPreviousUserMessageJumpId.value = "";
  },
);

// ==================== image preview ====================

const {
  imagePreviewOpen, imagePreviewDataUrl, imagePreviewLocalPath, imagePreviewZoom, imagePreviewRotation,
  IMAGE_PREVIEW_MIN_ZOOM, IMAGE_PREVIEW_MAX_ZOOM,
  previewOffsetX, previewOffsetY, previewDragging,
  zoomInPreview, zoomOutPreview, resetPreviewZoom, rotatePreviewClockwise,
  onPreviewWheel, openImagePreview, closeImagePreview,
  onPreviewPointerDown, onPreviewPointerMove, onPreviewPointerUp,
} = useChatImagePreview();

const imagePreviewCopyStatus = ref<string>('idle');
const imagePreviewSaveStatus = ref<string>('idle');

async function handleCopyLocalImage(path: string) {
  imagePreviewCopyStatus.value = 'doing';
  try {
    await copyTransportChatImageToClipboard(path);
  } catch (error) {
    console.warn('[预览] 复制图片失败', error);
  } finally {
    imagePreviewCopyStatus.value = 'idle';
  }
}

async function handleSaveLocalImage(path: string) {
  imagePreviewSaveStatus.value = 'doing';
  try {
    await saveTransportChatImageAs(path);
  } catch (error) {
    console.warn('[预览] 保存图片失败', error);
  } finally {
    imagePreviewSaveStatus.value = 'idle';
  }
}

// ==================== conversation actions ====================

function openCodeReviewDialog() {
  clearNativeTextSelection();
  codeReviewErrorText.value = "";
  launchPanel.value = readLaunchPanel();
  codeReviewDialogOpen.value = true;
}
function closeCodeReviewDialog() {
  if (toolReviewSubmittingBatchKey.value) return;
  codeReviewDialogOpen.value = false;
  codeReviewErrorText.value = "";
}
const codeReviewCommitSourceKey = computed(() =>
  `${String(props.activeConversationId || "").trim()}\n${String(effectiveSessionRootPath.value || "").trim()}`,
);
let codeReviewCommitRequestSeq = 0;

watch(codeReviewCommitSourceKey, () => {
  codeReviewCommitRequestSeq += 1;
  commitOptions.value = [];
  commitTotal.value = 0;
  commitPage.value = 1;
  commitOptionsLoading.value = false;
});

async function loadCodeReviewCommitOptions(page = 1) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return;
  const requestSeq = codeReviewCommitRequestSeq + 1;
  codeReviewCommitRequestSeq = requestSeq;
  const requestKey = codeReviewCommitSourceKey.value;
  commitOptionsLoading.value = true;
  try {
    const result = await listToolReviewCommitOptions(conversationId, page, commitPageSize.value);
    if (requestSeq !== codeReviewCommitRequestSeq || requestKey !== codeReviewCommitSourceKey.value) return;
    commitOptions.value = Array.isArray(result.commits) ? result.commits : [];
    commitTotal.value = Number(result.total || 0);
    commitPage.value = Number(result.page || page);
    commitPageSize.value = Number(result.pageSize || commitPageSize.value);
    codeReviewErrorText.value = "";
  } catch (error) {
    if (requestSeq !== codeReviewCommitRequestSeq || requestKey !== codeReviewCommitSourceKey.value) return;
    commitOptions.value = [];
    codeReviewErrorText.value = t("chat.readCommitFailed");
    console.error("[代码审查] 读取 commit 失败", error);
  } finally {
    if (requestSeq === codeReviewCommitRequestSeq) {
      commitOptionsLoading.value = false;
    }
  }
}
async function handleSubmitCodeReview(input: { scope: ToolReviewCodeReviewScope; target?: string; agentId: string; apiConfigId?: string }) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || toolReviewSubmittingBatchKey.value) return;
  codeReviewErrorText.value = "";
  const report = await submitToolReviewCode({
    conversationId,
    scope: input.scope,
    target: String(input.target || "").trim() || undefined,
    agentId: String(input.agentId || "").trim() || undefined,
    apiConfigId: String(input.apiConfigId || "").trim() || undefined,
  });
  if (!report) {
    codeReviewErrorText.value = t("chat.startCodeReviewFailed");
    return;
  }
  codeReviewDialogOpen.value = false;
}
/** 合卡对话框的「自定义委托」分支：只把请求发出去，结果由 finishLaunchDelegate 收口 */
function handleLaunchDelegate(input: { agentId: string; goal: string; apiConfigId?: string }) {
  if (launchDelegating.value) return;
  codeReviewErrorText.value = "";
  launchDelegating.value = true;
  emit("selectionActionDelegate", {
    count: 0,
    messageIds: [],
    blocks: [],
    conversationId: String(props.activeConversationId || "").trim() || undefined,
    agentId: String(input.agentId || "").trim(),
    presetId: "custom",
    why: "",
    goal: String(input.goal || "").trim(),
    todo: "",
    apiConfigId: String(input.apiConfigId || "").trim() || undefined,
  });
}
/** 委托提交结果收口：成功关闭卡片并记入最近目标，失败保留已填内容并在卡片内报错 */
function finishLaunchDelegate(ok: boolean) {
  if (!launchDelegating.value) return;
  launchDelegating.value = false;
  if (!ok) {
    codeReviewErrorText.value = t("chat.startDelegateFailed");
    return;
  }
  launchDialogRef.value?.rememberCurrentDelegateGoal();
  codeReviewDialogOpen.value = false;
}
/** 运行监控胶囊点击：按当前在跑的类型分流。只跑委托/任务时打开对应页面；
 *  只跑后台终端、或多种混合时打开主页卡片墙（即预览）。 */
function openRunSummaryPanel() {
  const delegateRunningCount = delegateStatuses.value.filter((delegate) => {
    const status = String(delegate.status || "").trim();
    return delegate.active && (status === "running" || status === "delivered");
  }).length;
  const taskRunning = Number(runningTaskCount.value) > 0;
  const shellRunning = Number(runningShellCount.value) > 0;
  const kindCount = [delegateRunningCount > 0, taskRunning, shellRunning].filter(Boolean).length;
  if (kindCount === 1 && delegateRunningCount > 0) {
    openMonitorTabFromHome("delegate");
    return;
  }
  if (kindCount === 1 && taskRunning) {
    openMonitorTabFromHome("tasks");
    return;
  }
  selectChatRightPanelMode("home");
}

async function openActiveConversationInBrowser() {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || props.systemNotificationMode) return;
  try {
    const info = await invokeTauri<WebAccessInfo>("transport.accessInfo", {
      input: { forceRefresh: false },
    });
    const localUrl = String(info?.localUrl || "").trim();
    if (!info?.enabled) {
      throw new Error(t("config.networkAccess.disabled"));
    }
    if (!info?.running || !localUrl) {
      throw new Error(t("config.networkAccess.statusUnavailable"));
    }
    const url = new URL(localUrl);
    url.searchParams.set("conversationId", conversationId);
    await openTransportExternalUrl(url.toString());
    linkOpenErrorText.value = "";
  } catch (error) {
    linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
  }
}

/** 分支确认弹窗：非空表示这一次发送被拦住，等用户选「继续对话」或「不发送」 */
const branchGuardPrompt = ref<{ repoBranch: string; recordedBranch: string } | null>(null);

async function handleSendChat() {
  // 发送前主动回读工作目录分支：不一致就拦住这一次发送，先让用户确认
  const check: BranchGuardCheck = props.checkBranchBeforeSend
    ? await props.checkBranchBeforeSend()
    : { kind: "ok" };
  if (check.kind === "prompt") {
    branchGuardPrompt.value = { repoBranch: check.repoBranch, recordedBranch: check.recordedBranch };
    return;
  }
  performSendChat();
}

/** 「继续对话」：先复写会话记录，再照常发送 */
function handleBranchGuardConfirm() {
  branchGuardPrompt.value = null;
  props.acceptBranchGuardPrompt?.();
  performSendChat();
}

/** 「不发送」：什么都不做，草稿留在输入框里，下次再点发送还会再问 */
function handleBranchGuardCancel() {
  branchGuardPrompt.value = null;
}

function performSendChat() {
  const conversationId = String(props.activeConversationId || "").trim();
  // 用户按下回车/点发送即视为转正：前端立刻隐藏草稿选择卡，不等后端水位线。
  if (conversationId && activeConversationIsDraft.value) {
    locallyPromotedConversationId.value = conversationId;
  }
  const extraTextBlocks = [
    ...attachedIdeContextReferences.value.map((item) => String(item.textBlock || "").trim()),
    ...buildKanbanIssueReferenceBlocks(),
  ].filter(Boolean);
  // 用户直接发言等于放弃护航建议：走到这一步才算真的发送，分支守卫拦下的那次不清。
  emit("dismissWheelchairSuggestion");
  emit("sendChat", extraTextBlocks.length > 0 ? { extraTextBlocks } : undefined);
  clearAttachedIdeContextReferences();
  clearKanbanIssueReferences();
}
/** 会话搜索页：由侧栏底部搜索按钮打开，覆盖在中间栏之上，不卸载聊天区。 */
const conversationSearchOpen = ref(false);
const conversationSearchNotice = ref("");

const conversationSearchAgentOptions = computed(() => {
  const items = props.conversationItems || props.unarchivedConversationItems || [];
  const seen = new Map<string, string>();
  for (const item of items) {
    const id = String(item.agentId || "").trim();
    if (!id || seen.has(id)) continue;
    seen.set(id, String(props.personaNameMap?.[id] || "").trim() || id);
  }
  return Array.from(seen.entries()).map(([id, name]) => ({ id, name }));
});

const conversationSearchWorkspaceOptions = computed(() => {
  const items = props.conversationItems || props.unarchivedConversationItems || [];
  const seen = new Map<string, string>();
  for (const item of items) {
    const path = String(item.workspaceRootPath || "").trim();
    if (!path || seen.has(path)) continue;
    const label = String(item.workspaceLabel || "").trim()
      || path.split(/[\\/]/).filter(Boolean).pop()
      || path;
    seen.set(path, label);
  }
  return Array.from(seen.entries()).map(([path, label]) => ({ path, label }));
});


function openConversationSearch() {
  conversationSearchNotice.value = "";
  conversationSearchOpen.value = true;
}

function closeConversationSearch() {
  conversationSearchNotice.value = "";
  conversationSearchOpen.value = false;
}

// 任何路径切会话都要收起搜索覆盖页：侧栏直选、批量归档、外部切换都走 activeConversationId 兜底。
watch(() => props.activeConversationId, () => {
  if (conversationSearchOpen.value) closeConversationSearch();
});

/** 搜索结果与近期会话点击：按会话 id 回到既有切换链路，联系人会话会自动切到对应侧栏分组。 */
function handleConversationSearchSelect(conversationId: string) {
  const id = String(conversationId || "").trim();
  if (!id) return;
  const items = props.conversationItems || props.unarchivedConversationItems || [];
  const target = items.find((item) => String(item.conversationId || "").trim() === id);
  if (!target) {
    conversationSearchNotice.value = t("chat.conversationSearchPanel.conversationMissing");
    return;
  }
  conversationSearchNotice.value = "";
  conversationSearchOpen.value = false;
  emit("switchConversation", {
    conversationId: id,
    kind: target.kind || "local_unarchived",
    remoteContactId: String(target.remoteContactId || "").trim() || undefined,
  });
}

function handleConversationListSelect(payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact" | "acp_session"; remoteContactId?: string }) {
  const id = String(payload?.conversationId || "").trim();
  if (!id || id === String(props.activeConversationId || "").trim()) return;
  const target = (props.conversationItems || props.unarchivedConversationItems).find((item) => String(item.conversationId || "").trim() === id);
  closeConversationSearch();
  emit("switchConversation", { conversationId: id, kind: payload?.kind || target?.kind, remoteContactId: String(payload?.remoteContactId || target?.remoteContactId || "").trim() || undefined });
  if (leftPaneOverlay.value) {
    emit("sideConversationListVisibleChange", false);
  }
}
function handleConversationRename(payload: { conversationId: string; title: string }) {
  const id = String(payload?.conversationId || "").trim();
  if (id) emit("renameConversation", { conversationId: id, title: String(payload?.title || "").trim() });
}
function handleConversationPinToggle(id: string) { emit("togglePinConversation", String(id || "").trim()); }
function handleConversationArchive(id: string) { emit("archiveConversation", String(id || "").trim()); }
function handleConversationExport(id: string) { emit("exportConversation", String(id || "").trim()); }
function handleConversationDelete(id: string) { emit("deleteConversation", String(id || "").trim()); }
function handleBatchArchiveCompleted(payload: { archivedConversationIds: string[]; activeConversationId?: string }) {
  const currentId = String(props.activeConversationId || "").trim();
  if (!currentId || !payload.archivedConversationIds.includes(currentId)) return;
  const nextId = String(payload.activeConversationId || "").trim();
  if (!nextId || nextId === currentId) return;
  closeConversationSearch();
  emit("switchConversation", { conversationId: nextId, kind: "local_unarchived" });
}
function handleRebindConversationRecipient() {
  const option = repairRecipientSelectedOption.value;
  const conversationId = String(props.activeConversationId || "").trim();
  const agentId = String(option?.agentId || repairRecipientAgentId.value || "").trim();
  if (!conversationId || !agentId) return;
  emit("rebindConversationRecipient", { conversationId, agentId });
}

// 「当前项目」分组只允许在 VS Code 侧边栏显示：
// 宿主为 VS Code 时跟随扩展注入的 workspaceRoots（当前打开的项目），
// 与会话工作区（currentWorkspaceRootPath）无关。
// 工作树感知：host 若打开的是 .pai/.worktree/{id}，回溯到项目根再做分组。
const currentProjectWorkspaceRoot = computed<string>(() => {
  if (!isVscodeHost()) return "";
  try {
    const hostRoot = getTransportHostContext().workspaceRoots[0];
    const raw = String(hostRoot?.path || "").trim();
    if (!raw) return "";
    return canonicalWorkspaceRootForComparison(raw);
  } catch {
    return "";
  }
});

const conversationDisplaySections = computed<ConversationSection[]>(() => {
  const sections = buildConversationSections(props.conversationItems || props.unarchivedConversationItems || [], {
    tab: props.chatLeftPanelMode,
    titles: {
      recent: t("chat.recentConversations"),
      pinned: t("chat.systemNotifications"),
      other: t("chat.otherConversations"),
      defaultWorkspace: t("chat.defaultWorkspace"),
      currentProject: t("chat.currentProject"),
      unknownPersona: t("chat.unknownPersona"),
    },
    locale: locale.value,
    currentWorkspaceRootPath: currentProjectWorkspaceRoot.value,
    activeConversationId: props.activeConversationId,
  });
  // Shift+滚轮跳过「最近会话」区：其中的会话在工作区/频道区会重复出现，
  // 滚动时同一会话滚两遍，顺序对不上列表直觉。
  return sections.filter((section) => section.key !== "recent");
});

// 草稿下拉唯一真相源：持久化 recent + 侧边栏全量分组，去重合并纯函数，永久只增不减
const draftWorkspaceOptions = computed<Array<{ id: string; name: string; path: string; access: ShellWorkspace["access"] }>>(() => {
  const allItems = (props.conversationItems || props.unarchivedConversationItems || []) as ChatConversationOverviewItem[];
  const sidebarPaths = buildWorkspaceConversationSections(allItems, { defaultWorkspaceTitle: t("chat.defaultWorkspace"), locale: locale.value }).map((s) => String(s.workspaceRootPath || "").trim()).filter(Boolean);
  const merged = [...recentWorkspacePaths.value, ...sidebarPaths];
  const deduped = new Map<string, { id: string; name: string; path: string; access: ShellWorkspace["access"] }>();
  for (const rawPath of merged) {
    const path = stripExtendedPathPrefix(String(rawPath || "").trim()).replace(/\/+$/, "");
    if (!path) continue;
    const key = normalizeWorkspacePathKey(path);
    if (!key || deduped.has(key)) continue;
    deduped.set(key, { id: `conversation-workspace-${key}`, name: defaultWorkspaceNameFromPath(path) || path, path, access: "approval" });
  }
  return Array.from(deduped.values());
});

async function handleDraftWorkspaceSave(workspaces: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, branch?: string, worktreePath?: string) {
  if (!props.saveDraftWorkspaces) return;
  await props.saveDraftWorkspaces(workspaces, autonomousMode, workMode, branch, worktreePath);
}

async function handleDraftWorkspaceSaveLegacy(payload: { path: string; name: string; access: ShellWorkspace["access"]; workMode: ShellWorkMode }) {
  if (!props.saveDraftWorkspaces) return;
  await props.saveDraftWorkspaces([{ id: `conversation-workspace-${Date.now().toString(36)}`, name: payload.name, path: payload.path, level: "main", access: payload.access, builtIn: false }], Boolean(props.currentWorkspaceAutonomousMode), payload.workMode);
}

function handleShiftWheel(event: WheelEvent) {
  if (!event.shiftKey) return;
  event.preventDefault();
  const sections = conversationDisplaySections.value;
  const orderedItems = sections.flatMap((section) => section.items);
  if (orderedItems.length === 0) return;
  const currentId = String(props.activeConversationId || "").trim();
  const currentIndex = orderedItems.findIndex((item) => String(item.conversationId || "").trim() === currentId);
  if (currentIndex < 0) return;
  const direction = event.deltaY > 0 ? 1 : -1;
  const target = orderedItems[currentIndex + direction];
  if (!target) return;
  emit("switchConversation", {
    conversationId: String(target.conversationId || "").trim(),
    kind: target.kind,
    remoteContactId: String(target.remoteContactId || "").trim() || undefined,
  });
}

// ==================== link / copy ====================

async function openChatMessageImagePreview(payload: {
  mime?: string;
  bytesBase64?: string;
  dataUrl?: string;
  localPath?: string;
  src?: string;
  alt?: string;
}) {
  const mime = String(payload?.mime || "").trim() || "image/png";
  const dataUrl = String(payload?.dataUrl || payload?.src || "").trim();
  const bytesBase64 = String(payload?.bytesBase64 || "").trim();
  const localPath = String(payload?.localPath || "").trim();
  if (dataUrl) {
    openImagePreview({ mime, dataUrl, localPath });
    return;
  }
  if (bytesBase64) {
    openImagePreview({ mime, bytesBase64, localPath });
    return;
  }
  if (localPath) {
    if (isAssistantSpacePath(localPath)) {
      try {
        const result = await readTransportChatImage({ path: localPath, mime, original: true });
        const originalDataUrl = String(result?.dataUrl || "").trim();
        if (originalDataUrl) {
          openImagePreview({ mime: result?.mime || mime, dataUrl: originalDataUrl, localPath });
        }
      } catch (error) {
        console.warn("[预览] Assistant Space 图片原图加载失败", { path: localPath, error });
      }
      return;
    }
    openImagePreview({ mime, dataUrl: resolveLocalFileUrl(localPath), localPath });
  }
}

async function handleAssistantLinkClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const localImage = target?.closest("[data-local-image-path]") as HTMLElement | null;
  if (localImage) {
    const rawPath = localImage.getAttribute("data-local-image-path") || "";
    let path = normalizeLocalLinkHref(rawPath);
    if (!path) return;
    if (!isAssistantSpacePath(path) && !isAbsoluteLocalPath(path)) {
      const root = String(props.currentWorkspaceRootPath || "").trim().replace(/\\/g, "/").replace(/\/$/, "");
      if (root) path = `${root}/${path.replace(/^\.\//, "")}`;
    }
    event.preventDefault(); event.stopPropagation();
    if (!isAssistantSpacePath(path) && await openTransportLocalFileReference(path)) {
      return;
    }
    try {
      const result = await readTransportChatImage({ path, mime: "image/png", original: true });
      const dataUrl = String(result?.dataUrl || "").trim();
      if (!dataUrl) return;
      openImagePreview({ mime: result?.mime || "image/png", dataUrl, localPath: path });
      linkOpenErrorText.value = "";
    } catch (error) {
      linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
    }
    return;
  }
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const rawHref = anchor.getAttribute("data-href") || anchor.getAttribute("href")?.trim() || "";
  let href = normalizeLocalLinkHref(rawHref);
  if (!href || href === "#") return;
  // 相对路径：基于当前工作目录解析为绝对路径
  if (!isAbsoluteLocalPath(href) && !href.startsWith("http://") && !href.startsWith("https://")) {
    const root = String(props.currentWorkspaceRootPath || "").trim().replace(/\\/g, "/").replace(/\/$/, "");
    if (root) {
      href = `${root}/${href}`;
    }
  }
  const localReference = parseLocalFileReference(href);
  const localPath = localReference?.path || href;
  if (isAbsoluteLocalPath(localPath)) {
    event.preventDefault(); event.stopPropagation();
    if (await openTransportLocalFileReference(href)) {
      return;
    }
    try {
      if (canOpenInFileReader(localPath) || !fileExtensionFromPath(localPath)) {
        if (!fileExtensionFromPath(localPath)) {
          // 无扩展名：先按目录展开，失败由上层回退按文件读（如 Makefile）
          openLocalDirectoryInChatReader(localPath, localReference?.line);
        } else {
          await openLocalFileInChatReader(localPath, localReference?.line);
        }
      }
      else { await openTransportLocalDirectory(localPath); }
      linkOpenErrorText.value = "";
    } catch (error) { linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) }); }
    return;
  }
  if (href.startsWith("http://") || href.startsWith("https://")) {
    event.preventDefault(); event.stopPropagation();
    try { await openTransportExternalUrl(href); linkOpenErrorText.value = ""; }
    catch (error) { linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) }); }
  }
}

function openLocalFileInChatReader(path: string, line?: number) {
  emit("openChatReaderFile", path, line);
}

function openLocalDirectoryInChatReader(path: string, line?: number) {
  emit("openChatReaderDirectory", path, line);
}

// Markdown 文件链接的右键菜单：命中判定与菜单状态由共享组合式函数管理。
const { state: fileLinkMenuState, close: closeFileLinkMenu } = useFileLinkContextMenu({
  workspaceRoot: () => props.currentWorkspaceRootPath,
});

/** 菜单「在侧边打开」：与左键点击同一套打开口径。 */
function handleFileLinkOpenInSidebar(payload: { path: string; line?: number }) {
  const path = String(payload?.path || "").trim();
  if (!path) return;
  void openFileLinkReferenceInReader(path, payload?.line);
}

async function openFileLinkReferenceInReader(path: string, line?: number) {
  try {
    if (canOpenInFileReader(path) || !fileExtensionFromPath(path)) {
      if (!fileExtensionFromPath(path)) {
        // 无扩展名：先按目录展开，失败由上层回退按文件读（如 Makefile）
        openLocalDirectoryInChatReader(path, line);
      } else {
        await openLocalFileInChatReader(path, line);
      }
      linkOpenErrorText.value = "";
    } else {
      await openTransportLocalDirectory(path);
    }
  } catch (error) {
    linkOpenErrorText.value = t("status.openLinkFailed", { err: String(error) });
  }
}

async function openFileInReader(path: string, line?: number) {
  const panel = chatReaderPanelRef.value;
  if (!panel) {
    throw new Error("文件阅读面板尚未就绪");
  }
  await panel.openPath(path, { targetLine: line });
  // 从聊天跳转打开文件后收起左侧文件夹/Git 栏，让内容区占满（目录内双击打开不受影响）
  panel.closeDirectoryTree();
}

/** 目录展开成功返回 true；失败时关闭目录树并返回 false，由调用方回退按文件打开。 */
async function openDirectoryInReader(path: string): Promise<boolean> {
  const panel = chatReaderPanelRef.value;
  if (!panel) {
    return false;
  }
  // 面板首次挂载时会异步恢复会话（可能把目录根切回旧会话目录），等它完成再展开目标目录
  await panel.whenSessionRestored?.();
  const opened = await panel.openDirectoryTree(path);
  if (!opened) {
    panel.closeDirectoryTree();
  }
  return opened;
}

// ==================== lifecycle ====================

let unlistenFileReaderAddToChat: (() => void) | null = null;

onMounted(() => {
  void nextTick(() => chatScrollbarRef.value?.updateThumb());
  document.addEventListener("pointerdown", closeCompactionSummaryContextMenu);
  document.addEventListener("keydown", handleCompactionSummaryContextMenuKeydown);
  // 文件阅读器只向主 chat 窗口广播引用；侧面板（追问/队员）是同一份组件的第二实例，
  // 若也订阅就会各挂一份引用并各自抢焦点，导致引用同时进入两个会话。
  if (props.composerScope !== "side") {
    unlistenFileReaderAddToChat = onTransportNotification<IdeContextReferenceItem>("fileReader.addToChat", (payload) => {
      handleAddFileReaderContextReference(payload);
    });
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeCompactionSummaryContextMenu);
  document.removeEventListener("keydown", handleCompactionSummaryContextMenuKeydown);
  unlistenFileReaderAddToChat?.();
  unlistenFileReaderAddToChat = null;
  if (transientNoticeTimer) window.clearTimeout(transientNoticeTimer);
  panesCleanupFns.forEach((fn) => fn());
  stopAudioPlayback();
});
</script>

<style scoped>
.ecall-chat-scroll-container {
  overflow-anchor: none;
}

/* 消息项作为宽度容器：气泡与正文据此决定是贴头像左缘（窄屏）还是让出头像列、对齐名字（宽屏） */
.ecall-elastic-item-shell {
  container-type: inline-size;
}

/* 打开右侧面板：像手机应用那样从略小放大进入；时长与曲线复用侧栏 push 动画的 220ms 同参 */
.ecall-panel-enter {
  animation: ecall-panel-enter 220ms cubic-bezier(0.2, 0, 0, 1);
}

@keyframes ecall-panel-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ecall-panel-enter {
    animation: none;
  }
}

.todo-bar-slide-enter-active,
.todo-bar-slide-leave-active {
  transition: opacity 200ms ease, transform 200ms ease;
}

.todo-bar-slide-enter-from,
.todo-bar-slide-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

@media (prefers-reduced-motion: reduce) {
  .todo-bar-slide-enter-active,
  .todo-bar-slide-leave-active {
    transition: none;
  }

  .todo-bar-slide-enter-from,
  .todo-bar-slide-leave-to {
    opacity: 1;
    transform: none;
  }
}

.chat-status-banner-enter-active,
.chat-status-banner-leave-active {
  transition: opacity 160ms ease-out, transform 160ms ease-out;
}

.chat-status-banner-enter-from,
.chat-status-banner-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.ecall-timeline-rail-enter-active,
.ecall-timeline-rail-leave-active {
  transition: opacity 200ms ease, transform 200ms ease, width 200ms ease, min-width 200ms ease;
  overflow: hidden;
}
.ecall-timeline-rail-enter-from,
.ecall-timeline-rail-leave-to {
  opacity: 0;
  transform: translateX(-8px);
  width: 0 !important;
  min-width: 0 !important;
}

@media (prefers-reduced-motion: reduce) {
  .ecall-timeline-rail-enter-active,
  .ecall-timeline-rail-leave-active {
    transition: none;
  }
  .ecall-timeline-rail-enter-from,
  .ecall-timeline-rail-leave-to {
    opacity: 1;
    transform: none;
    width: 0 !important;
    min-width: 0 !important;
  }
}

.ecall-timeline-preview-enter-active,
.ecall-timeline-preview-leave-active {
  transition: opacity 180ms ease, transform 180ms ease;
}
.ecall-timeline-preview-enter-from,
.ecall-timeline-preview-leave-to {
  opacity: 0;
  transform: translateX(6px) scale(0.98);
}
@media (prefers-reduced-motion: reduce) {
  .ecall-timeline-preview-enter-active,
  .ecall-timeline-preview-leave-active {
    transition: none;
  }
}
</style>
