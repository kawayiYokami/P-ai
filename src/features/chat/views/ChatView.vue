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
        :class="leftPaneLayoutSnapshot ? 'relative flex h-full min-h-0 shrink-0 overflow-hidden' : 'absolute bottom-0 left-0 top-0 z-50 flex h-full min-h-0 border-r border-base-300 bg-base-100 shadow-2xl overflow-hidden'"
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
      <div data-chat-center-pane="true" class="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <div
          v-if="mediaDragActive && !chatting && !frozen && !conversationInteractionBusy"
          class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
        >
          <div class="rounded-box border border-primary/40 bg-base-100 px-4 py-2 text-sm font-medium text-primary">
            {{ t("chat.dropImageOrPdf") }}
          </div>
        </div>

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
            class="ecall-chat-scroll-container relative flex flex-1 min-h-0 flex-col overflow-x-hidden overflow-y-auto px-0 py-3"
            :class="chatting || frozen || conversationInteractionBusy ? 'pointer-events-auto' : ''"
            :data-chat-interaction-locked="chatting || frozen || conversationInteractionBusy ? 'true' : undefined"
            @scroll="handleConversationScroll"
            @wheel="handleConversationWheelInput"
            @pointerdown="beginPointerScrollIntent"
          >
          <div ref="chatContentRoot" class="flex min-w-0 shrink-0 flex-col">
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
            :workspaces="props.workspaces"
            :workspace-autonomous-mode="Boolean(props.currentWorkspaceAutonomousMode)"
            :save-workspace="props.saveDraftWorkspaces ? handleDraftWorkspaceSaveLegacy : undefined"
            :save-workspaces="props.saveDraftWorkspaces ? handleDraftWorkspaceSave : undefined"
            :git-root-check="props.draftWorkspaceGitRootCheck"
            @change="handleDraftPersonaChange($event)"
            @update:title="handleDraftTitleChange($event)"
          />
          <div class="ecall-chat-history-flow flex min-w-0 shrink-0 flex-col">
            <div
              v-if="showNoMoreHistoryDivider"
              class="mx-auto flex w-full max-w-225 items-center gap-3 px-4 pb-2 pt-1 text-xs text-base-content/45"
            >
              <div class="h-px flex-1 bg-base-300/70"></div>
              <span class="shrink-0 font-semibold text-base-content/55">{{ t("chat.noMoreHistory") }}</span>
              <div class="h-px flex-1 bg-base-300/70"></div>
            </div>
            <Virtualizer
              v-if="scrollContainer"
              :data="virtualRenderItems"
              :shift="virtuaShift"
              ref="virtuaRef"
              :key="virtuaKey"
              :scroll-ref="(scrollContainer as unknown as HTMLElement)"
              class="min-w-0 w-full shrink-0"
            >
              <template #default="{ item }">
                <div :key="item.id" class="w-full ecall-chat-virtual-item">
                  <div
                    v-if="item.kind === 'compaction'"
                    class="mt-4 flex items-center gap-3 text-xs text-base-content/45"
                  >
                    <div class="h-px flex-1 bg-base-300/80"></div>
                    <button type="button" class="btn btn-ghost btn-xs shrink-0 gap-1.5 px-2 text-base-content/60 hover:text-base-content"
                      :title="t('chat.viewSummary')" @click="openConversationSummary(item.block, $event)"
                      @contextmenu.prevent.stop="openCompactionSummaryContextMenu(item.block, $event)">
                      <History class="h-3.5 w-3.5" />
                      <span>{{ t("chat.viewSummary") }}</span>
                    </button>
                    <div class="h-px flex-1 bg-base-300/80"></div>
                  </div>
                  <div v-else-if="item.kind === 'plan_started'" class="mt-4 flex items-center gap-3 text-xs text-base-content/45">
                    <div class="h-px flex-1 bg-base-300/80"></div>
                    <span class="shrink-0 rounded-full border border-base-300 bg-base-100 px-3 py-1 text-base-content/55">{{ t("chat.planStartedDivider") }}</span>
                    <div class="h-px flex-1 bg-base-300/80"></div>
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
              </template>
            </Virtualizer>

            <div
              class="pointer-events-none overflow-hidden"
              :style="{ height: `${latestOwnTailSpacerMinHeight}px` }"
            ></div>
            <div
              v-if="supportsFloatingSessionToolbar"
              class="pointer-events-none mx-auto w-full max-w-225 shrink-0 px-4"
              :style="{ height: `${toolbarReservedHeight + 10}px` }"
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
          :class="displayedSessionRow === 'toolbar'
            ? 'pointer-events-auto opacity-100'
            : 'pointer-events-none opacity-0'"
          :aria-hidden="displayedSessionRow === 'toolbar' ? undefined : 'true'"
        >
          <div class="ecall-chat-toolbar-shell w-full px-2">
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
              :show-code-review-menu-item="true"
              :side-chat-enabled="sideChatPanelEnabled"
              :delegate-statuses="delegateStatuses"
              :running-task-count="runningTaskCount"
              :running-shell-count="runningShellCount"
              @lock-workspace="$emit('lockWorkspace')" @open-branch-selection="openBranchSelectionMenu"
              @open-task-create="openTaskCreateDialog"
              @open-delegate-selection="openDelegateSelectionMenu" @open-forward-selection="openForwardSelectionMenu"
              @open-auto-push="openAutoPushCard"
              @open-share-selection="openShareSelectionMenu"
              @open-conversation-in-browser="openActiveConversationInBrowser"
              @open-run-summary="openRunSummaryPanel"
              @open-code-review="openCodeReviewDialog"
              @open-branch-from-current="openBranchFromCurrentMessage"
              @open-side-chat="selectChatRightPanelMode('sideChat')"
            />
          </div>
        </div>
        <!-- 上排：离底时出现（预览条 + 时间线按钮），始终位于工作区 bar 上方 -->
        <div
          class="pointer-events-none absolute inset-x-0 bottom-0"
        >
        <div class="flex w-full items-end justify-between gap-2 px-2">
          <div class="pointer-events-none min-w-0 flex-1">
            <ChatThinkingPreviewBar
              :blocks="previewBlocksForBar"
              :idle-text="previewTextForBar"
              :avatar-url="previewAvatarUrl"
              :collapse-preview="thinkingPreviewCollapsed"
              :visible="!atConversationBottom && !chatStatusBanner && !timelinePanelOpen && !timelineFloatPanelVisible && displayedSessionRow === 'top'"
              :streaming="chatting"
              @jump-to-bottom="handleJumpToBottomWithFollow"
            />
          </div>
          <!-- 时间线按钮：悬停即在原位向上展开蛇形时间线；蛇形起点后面挂一个预览点，点它打开垂直面板 -->
          <div class="relative flex h-10 shrink-0 items-center">
            <TimelineSnakeBoard
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
              v-if="timelineButtonVisible"
              ref="timelineFloatButtonRef"
              type="button"
              class="flex items-center rounded-full p-2"
              :class="[FROST_SURFACE, timelineFloatPanelVisible ? 'invisible' : 'pointer-events-auto']"
              :aria-label="t('chat.timelineButtonAria')"
              :aria-expanded="timelineFloatPanelVisible ? 'true' : 'false'"
              @mouseenter="handleTimelineFloatEnter"
              @mouseleave="handleTimelineButtonLeave"
              @click="handleTimelineButtonClick"
            >
              <Route class="h-4 w-4 shrink-0" />
              <span class="ml-1.5 whitespace-nowrap text-xs leading-none">{{ t("chat.timelineButtonLabel") }}</span>
            </button>
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
            <!-- 背景压黑：全覆盖层 -->
            <div class="absolute inset-0 bg-black/50"></div>
            <!-- 卡片：统一毛玻璃底座（FROST_GLASS），宽度限死上限，高度占 92%，居中 -->
            <div
              class="relative flex h-[92%] w-[92%] max-w-3xl flex-col overflow-hidden rounded-box shadow-xl"
              :class="FROST_GLASS"
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
                      :class="entry.index === activeTimelineIndex ? 'ring-2 ring-primary' : 'opacity-80'"
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
                    <div class="text-sm font-black">{{ entry.speaker }}</div>
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
                  <CircleAlert class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  {{ t("chat.timelineJumpHint") }}
                </span>
                <button type="button" class="btn btn-sm gap-1" @click="closeTimelinePanel">
                  <ArrowLeft class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
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
              <Undo2 class="h-4 w-4" />
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
          :class="isWebRoundedMode ? 'border-transparent bg-transparent px-4 pt-0 pb-3' : ''"
        >
          <div class="w-full" :class="isWebRoundedMode ? '-mt-6' : ''">
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
                        type="button"
                        class="btn btn-ghost btn-sm gap-1 text-error hover:bg-error/15"
                        @click="void copyStatusText(chatStatusBanner.text)"
                      >
                        <Copy class="h-3.5 w-3.5" />
                        <span>{{ t("common.copy") }}</span>
                      </button>
                      <button
                        type="button"
                        class="btn btn-ghost btn-sm gap-1 text-error hover:bg-error/15"
                        @click="$emit('clearChatError')"
                      >
                        <X class="h-3.5 w-3.5" />
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
            v-if="activeConversationTerminalApprovals.length > 0"
            :key="activeConversationTerminalApprovals.map((a) => a.requestId).join(',')"
            :items="approvalQuestionItems"
            v-model="approvalQuestionAnswers"
            :submitting="!!terminalApprovalResolving"
            :class="isWebRoundedMode ? 'pb-1' : 'px-4 pb-4 pt-1'"
            @submit="handleApprovalQuestionSubmit"
            @approve-for-workspace="handleApprovalQuestionWorkspaceRemember"
          />
          <div
            v-else-if="activeConversationRecipientMissing"
            class="rounded-box border border-warning/30 bg-warning/10 p-4 text-sm"
            :class="isWebRoundedMode ? '' : 'm-4'"
          >
            <div class="flex flex-col gap-4">
              <!-- 信息区：图标锚点 + 标题 + 说明，独立成块不被操作区挤压 -->
              <div class="flex items-start gap-3">
                <div
                  class="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-warning/15 text-warning"
                >
                  <CircleAlert class="h-5 w-5" />
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
                    <Check class="h-3.5 w-3.5" />
                    <span>{{ t("chat.recipientMissingApply") }}</span>
                  </button>
                  <button
                    type="button"
                    class="btn btn-sm btn-error btn-outline gap-2"
                    :disabled="conversationInteractionBusy"
                    @click="handleConversationDelete(activeConversationId)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
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
            :workspace-access="workspaceAccess"
            :frontend-round-phase="frontendRoundPhase" :chat-usage-percent="chatUsagePercent"
            :chatting="chatting" :busy="conversationInteractionBusy"
            :stop-chat-disabled="isOrganizingContextBusy || submitPending" :frozen="frozen"
            :goal-active="goalActive"
            :goal-title="goalButtonTitle"
            :goal-disabled="activeConversationSummary?.kind === 'remote_im_contact'"
            :system-notification-mode="activeConversationIsSystemNotification"
            :remote-contact-mode="activeConversationIsRemoteContact"
            :selection-delegate-only="messageSelectionDelegateOnly"
            :show-side-conversation-list="showSideConversationList"
            :active-conversation-id="activeConversationId" :unarchived-conversation-items="unarchivedConversationItems"
            :remote-im-contact-conversations="remoteImContactConversations"
            :user-alias="userAlias" :user-avatar-url="userAvatarUrl"
            :persona-name="personaName" :persona-name-map="personaNameMap" :persona-avatar-url-map="personaAvatarUrlMap"
            :create-conversation-agent-options="createConversationAgentOptions"
            :default-create-conversation-agent-id="defaultCreateConversationAgentId"
            :ide-context-groups="mergedVisibleIdeContextGroups" :attached-ide-context-references="attachedIdeContextReferences"
            :current-theme="currentTheme"
            :show-conversation-actions="showConversationActions"
            :active-agent-id="activeAgentId"
            :is-rounded="isWebRoundedMode"
            @update:chat-input="$emit('update:chatInput', $event)" @add-mention="$emit('addMention', $event)"
            @remove-mention="$emit('removeMention', $event)" @remove-clipboard-image="$emit('removeClipboardImage', $event)"
            @remove-queued-attachment-notice="$emit('removeQueuedAttachmentNotice', $event)"
            @pick-attachments="$emit('pickAttachments')"
            @update:conversation-preferred-api-config-id="$emit('update:conversationPreferredApiConfigId', $event)"
            @update:workspace-access="$emit('updateWorkspaceAccess', $event)"
            @update:plan-mode-enabled="$emit('update:planModeEnabled', $event)"
            @attach-ide-context-reference="handleAttachIdeContextReference"
            @remove-ide-context-reference="handleRemoveIdeContextReference"
            @send-chat="handleSendChat" @stop-chat="$emit('stopChat')"
            @open-delegate-selection="openDelegateSelectionMenu"
            @open-task-create="openTaskCreateDialog"
            @open-goal-task="$emit('openGoalTask')"
            @exit-selection-mode="handleExitMessageSelectionMode"
            @selection-action-copy="copySelectedMessages"
            @selection-action-branch="emitSelectionAction('branch')"
            @selection-action-forward="emitSelectionAction('forward', $event)"
            @selection-action-delegate="emitSelectionAction('delegate', $event)"
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
        />

        <ChatGoalTaskDialog
          :open="goalDialogOpen" :saving="goalSaving" :error-text="goalError"
          :active-task="activeGoalTask" :recent-history="recentGoalTaskHistory"
          @close="$emit('closeGoalTask')" @save="$emit('saveGoalTask', $event)"
          @stop="$emit('stopGoalTask')"
        />
        <ToolReviewTargetDialog
          v-if="showConversationActions"
          :open="codeReviewDialogOpen"
          :submitting="!!toolReviewSubmittingBatchKey"
          :error-text="codeReviewErrorText"
          :current-agent-id="props.activeAgentId"
          :agent-options="props.createConversationAgentOptions"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :commit-options="commitOptions"
          :commit-options-loading="commitOptionsLoading"
          :commit-total="commitTotal"
          :commit-page="commitPage"
          :commit-page-size="commitPageSize"
          @close="closeCodeReviewDialog"
          @pick-commit-review="loadCodeReviewCommitOptions"
          @review-code="handleSubmitCodeReview"
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
          :branch="homeGitBranch"
          :git-changes="homeGitChanges"
          :change-count="homeGitChangeCount"
          :recent-commits="homeGitRecentCommits"
          :open-files="homeFilePreview.openFiles"
          :active-path="homeFilePreview.activePath"
          :open-file-count="homeFilePreview.openFileCount"
          :side-chats="sideChatItems"
          :side-chat-enabled="Boolean(sideChatPanelEnabled)"
          :delegates="delegateStatuses"
          :running-tasks="homePanelRunningTasks"
          :shells="backgroundShells"
          :tool-batches="toolReviewBatches"
          @select-panel="selectChatRightPanelMode"
          @open-file="openHomeFile"
          @open-side-chat="openHomeSideChat"
          @create-side-chat="openHomeSideChatNewPage"
          @open-workspace="openHomeWorkspaceDirectory"
          @open-git-changes="openHomeGitChanges"
          @open-git-commits="openHomeGitChanges"
          @open-monitor-tab="openMonitorTabFromHome"
        />
        <FileReaderPanel
          v-else-if="chatRightPanelMode === 'reader'"
          ref="chatReaderPanelRef"
          class="ecall-panel-enter h-full w-full"
          :narrow-overlay="rightPaneOverlay"
          :initial-root-path="effectiveFileReaderRootPath"
          :session-key="chatFileReaderSessionKey"
          :legacy-session-key="legacyChatFileReaderSessionKey"
          :enable-global-drop="false"
          :show-pick-file-button="false"
          :show-tab-local-file-actions="true"
          :markdown-is-dark="markdownIsDark"
          custom-markstream-id="chat-file-reader-markstream"
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
        <div v-else-if="chatRightPanelMode === 'monitor'" class="ecall-panel-enter flex h-full min-h-0 w-full flex-col bg-base-200">
          <PanelTabStrip
            :tabs="monitorPanelTabs"
            :active-key="chatMonitorPanelMode"
            :show-tab-borders="false"
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, toRef, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { isDarkAppTheme, isVscodeHost } from "../../shell/composables/use-app-theme";
import {
  useChatComposerAppearance,
  visibleChatComposerContextGroups,
} from "../../shell/composables/use-chat-composer-appearance";
import { ArrowLeft, Check, CircleAlert, Copy, History, Inbox, ListTodo, Network, Route, Trash2, Undo2, Wrench, X } from "@lucide/vue";
import {
  copyTransportChatImageToClipboard,
  getTransportHostContext,
  invokeTauri,
  isDesktopTauriHost,
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
import ChatComposerPanel from "../components/ChatComposerPanel.vue";
import ChatThinkingPreviewBar from "../components/ChatThinkingPreviewBar.vue";
import TimelineSnakeBoard from "../components/TimelineSnakeBoard.vue";
import { FROST_GLASS, FROST_SURFACE } from "../components/session-float-styles";
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
import { computeTimelineSegmentStarts, resolveTimelineVisibleStartIndex } from "../utils/timeline-segments";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import { clearFileReaderContextCandidates } from "../utils/file-reader-context-tags";
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
import { clearNativeTextSelection } from "../../../utils/native-selection";

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
  mediaDragActive: boolean; chatting: boolean; trimming: boolean; trimmingConversationId?: string;
  compactingConversation: boolean; compactingConversationId?: string;
  conversationBusy: boolean; frozen: boolean; messageBlocks: ChatMessageBlock[];
  hasMoreHistory: boolean; loadingOlderHistory: boolean;
  latestOwnMessageAlignRequest: number; conversationScrollToBottomRequest: number; scrollToBottomBehavior: "auto" | "smooth" | "own_top" | "manual" | "follow";
  currentWorkspaceName: string; currentWorkspaceDisplayName?: string; currentWorkspaceRootPath: string; workspaces: ShellWorkspace[];
  currentWorkspaceAutonomousMode?: boolean;
  currentWorkspaceWorkMode?: ShellWorkMode;
  currentWorkspaceBranch?: string;
  configShellWorkspaces?: ShellWorkspace[];
  saveDraftWorkspaces?: (items: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, shellWorkBranch?: string) => Promise<void>;
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
  /** 右侧主页「追问」卡片数据；追问会话由宿主容器持有 */
  sideChatItems?: Array<{ id: string; title: string }>;
  createConversationAgentOptions: AgentPersonaOption[];
  recipientOptionsReady?: boolean;
  defaultCreateConversationAgentId: string;
  ideContextGroups: IdeContextWorkspaceGroup[];
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
  hideConversationControlPanel?: boolean;
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
  (e: "openSideChatConversation", conversationId: string): void;
  (e: "openSideChatNewPage"): void;
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
  (e: "sendChat", payload?: { extraTextBlocks?: string[] }): void;
  (e: "stopChat"): void; (e: "trimConversation"): void; (e: "openConversationList"): void; (e: "openSettings"): void;
  (e: "clearChatError"): void;
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
  (e: "switchConversation", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
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
  (e: "selectionActionDelegate", payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; agentId: string; presetId: string; why: string; goal: string; todo: string }): void;
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
const codeReviewErrorText = ref("");
const commitOptions = ref<ToolReviewCommitOption[]>([]);
const commitOptionsLoading = ref(false);
const commitTotal = ref(0);
const commitPage = ref(1);
const commitPageSize = ref(5);

type ToolReviewSidebarTab = "tools" | "delegates" | "tasks" | "fastRequests";

const monitorPanelTabs = computed<Array<{ key: ChatMonitorPanelMode; label: string; icon: typeof Network; closeable: false }>>(() => [
  { key: "delegate", label: t("chat.toolReview.delegatesTab"), icon: Network, closeable: false },
  { key: "tasks", label: t("chat.toolReview.tasksTab"), icon: ListTodo, closeable: false },
  { key: "tools", label: t("chat.toolReview.toolsTab"), icon: Wrench, closeable: false },
  { key: "fastRequests", label: t("chat.toolReview.overviewOthers"), icon: Inbox, closeable: false },
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
const chatStatusBanner = computed(() => {
  if (transientNotice.value) return transientNotice.value;
  return baseChatStatusBanner.value;
});
const requestErrorTitle = computed(() => {
  const title = t("chat.errorTitleRequest");
  return title === "chat.errorTitleRequest" ? "请求发生错误" : title;
});
const conversationInteractionBusy = computed(() =>
  props.conversationBusy || isOrganizingContextBusy.value,
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
// 避免 currentWorkspaceRootPath（workspace.list 异步）滞后导致的 Home 闪现
const effectiveFileReaderRootPath = computed(() => {
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

const messageSelectionDelegateOnly = ref(false);

function openSelectionMenu(options: { delegateOnly?: boolean; allowWhenBusy?: boolean } = {}) {
  // 多选入口忙碌时禁用；分支/委托/转发/分享属于子代理或纯读取操作，
  // 不影响主轮次，忙碌时允许进入选择模式（allowWhenBusy）。
  if (!options.allowWhenBusy && (props.chatting || props.frozen || conversationInteractionBusy.value)) return;
  clearNativeTextSelection();
  messageSelectionDelegateOnly.value = !!options.delegateOnly;
  messageSelectionModeEnabled.value = true;
  selectedMessageRenderIds.value = [];
  void nextTick(() => composerPanelRef.value?.focusInput?.({ preventScroll: true }));
}
const openBranchSelectionMenu = () => openSelectionMenu({ allowWhenBusy: true });
const openDelegateSelectionMenu = () => openSelectionMenu({ delegateOnly: true, allowWhenBusy: true });
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
  emit("taskCreated", task);
}

function handleTaskUpdated(task: TaskEntry) {
  taskDialogOpen.value = false;
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

// virtua 最简：shift 在顶部 prepend 时保持视口锚定，无需隐藏预量 staged 容器
const virtuaRef = ref<InstanceType<typeof Virtualizer> | null>(null);
const virtuaShift = computed(() => {
  if (virtualRenderItems.value.length === 0) return false;
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

const showNoMoreHistoryDivider = computed(() =>
  !!String(props.activeConversationId || "").trim()
  && props.messageBlocks.length > 0
  && !props.loadingOlderHistory
  && !props.hasMoreHistory,
);

// ==================== block tracking ====================

const { isOwnMessage, latestOwnMessageId, latestOwnElasticItemId } =
  useChatBlockTracking(toRef(props, "messageBlocks"), chatRenderItems);

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
    selectionActionDelegate: (payload) => emit("selectionActionDelegate", payload),
    selectionActionShare: (payload) => emit("selectionActionShare", payload),
  },
});

function handleEnterMessageSelectionMode(selectionKey: string) {
  messageSelectionDelegateOnly.value = false;
  enterMessageSelectionMode(selectionKey);
}

function handleExitMessageSelectionMode() {
  messageSelectionDelegateOnly.value = false;
  resetMessageSelectionMode();
}

defineExpose({
  exitMessageSelectionMode: handleExitMessageSelectionMode,
  showTransientNotice,
  openFileInReader,
  openDirectoryInReader,
});

// Web双栏圆角悬浮判：桌面常直角，Web左右栏同时在位才圆角
const isWebRoundedMode = ref(false);

const {
  scrollContainer, composerContainer, toolbarContainer, chatLayoutRoot,
  latestOwnElasticMinHeight, atConversationBottom, userScrollingUp,
  followBottom, startFollowBottom, stopFollowBottom,
  sessionControlPanelVisible, sessionFloatDockStyle, toolbarReservedHeight, onScroll,
  noteWheelScrollIntent, beginPointerScrollIntent, prepareBottomAlignmentLayout,
} = useChatScrollLayout({
  activeConversationId: toRef(props, "activeConversationId"),
  chatting: toRef(props, "chatting"), busy: conversationInteractionBusy,
  frozen: toRef(props, "frozen"),
  timelineItemCount: computed(() => virtualRenderItems.value.length),
  isWebRoundedMode: isWebRoundedMode as unknown as Ref<boolean>,
  onReachedBottom: () => emit("reachedBottom"),
  focusComposerInput: (options) => composerPanelRef.value?.focusInput(options),
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
  const len = virtualRenderItems.value.length;
  const clampedTarget = len > 0 ? Math.max(0, Math.min(index, len - 1)) : index;
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
// 对齐落点抽样：确认平滑滚动到位后没有别的机制再把它挪走
function traceAlignLanding(index: number) {
  let step = 0;
  const tick = () => {
    step += 1;
    probeChatScroll("对齐后", {
      index,
      step,
      scrollTop: Math.round(scrollContainer.value?.scrollTop ?? -1),
      scrollHeight: scrollContainer.value?.scrollHeight ?? -1,
      followBottom: followBottom.value,
    });
    if (step < 6) window.setTimeout(tick, 150);
  };
  window.setTimeout(tick, 150);
}
// 最新用户消息对齐到视口顶部：与时间线跳转同一套 align: "start"。
// 新消息刚插入时尾段尚未测量，落点会被夹到底部，这里按帧重试到找到锚点且尾段测量完成。
function alignLatestOwnMessageToTop() {
  let lastIndex = -1;
  let lastMeasured = false;
  // 对齐到顶部就是「不贴底」：先退出跟随，否则流式内容一长就会被贴底顶走
  stopFollowBottom();
  probeChatScroll("对齐请求", { itemId: String(latestOwnElasticItemId.value || "").trim() });
  const attempt = (retries = 24) => {
    const itemId = String(latestOwnElasticItemId.value || "").trim();
    const index = itemId ? virtualRenderItems.value.findIndex((item) => item.id === itemId) : -1;
    const measured = latestOwnTailContentMeasured.value;
    const canScroll = index >= 0 && !!virtuaRef.value;
    // 找到锚点先落一次；尾段测量完成、留白到位后再落一次终稿
    if (canScroll && (index !== lastIndex || (measured && !lastMeasured))) {
      lastIndex = index;
      lastMeasured = measured;
      try { virtuaRef.value!.scrollToIndex(index, { align: "start", smooth: true } as any); } catch {}
      probeChatScroll("对齐落点", {
        index,
        measured,
        scrollTop: Math.round(scrollContainer.value?.scrollTop ?? -1),
        followBottom: followBottom.value,
        chatting: props.chatting,
      });
      if (measured) traceAlignLanding(index);
    }
    if (canScroll && measured) return;
    if (retries > 0) requestAnimationFrame(() => attempt(retries - 1));
  };
  void nextTick(() => requestAnimationFrame(() => attempt()));
}
function resetVirtualizerAtConversationBottom(behavior: "auto" | "smooth" = "auto") {
  const el = scrollContainer.value;
  if (el) {
    const attempt = (retries = 6) => {
      const targetTop = Math.max(0, el.scrollHeight - el.clientHeight);
      el.scrollTo({ top: targetTop, behavior });
      chatScrollbarRef.value?.updateThumb();
      if (retries > 0) {
        const needTail = !!String(latestOwnElasticItemId.value || "").trim() && !latestOwnTailContentMeasured.value;
        const v = virtuaRef.value as unknown as { cache?: unknown } | null;
        const sizes = (v as any)?.cache?.[0] as number[] | undefined;
        const hasUnmeasured = Array.isArray(sizes) && sizes.slice(-4).some((h: number) => h === -1);
        if (needTail || hasUnmeasured) {
          requestAnimationFrame(() => attempt(retries - 1));
        } else if (Math.abs(el.scrollTop - targetTop) > 2) {
          requestAnimationFrame(() => attempt(retries - 1));
        }
      }
    };
    void nextTick(() => requestAnimationFrame(() => attempt()));
    return;
  }
  const len = virtualRenderItems.value.length;
  if (len <= 0 || !virtuaRef.value) return;
  try { virtuaRef.value.scrollToIndex(len - 1, { align: "end", smooth: resolveVirtualSmooth(len - 1, behavior) } as any); } catch {}
}
function scheduleVirtualMeasure() {
  // virtua 内部 ResizeObserver 自动测量，无需手动触发
}
function syncViewportMetrics() {
  void nextTick(() => chatScrollbarRef.value?.updateThumb());
}

// 尾部弹性高度：直接用 virtua 测量值求和，需轮询 cache（非响应式）
const latestOwnTailContentRange = computed(() => {
  const itemId = String(latestOwnElasticItemId.value || "").trim();
  if (!itemId) return [] as ChatRenderItem[];
  const startIndex = virtualRenderItems.value.findIndex((item) => item.id === itemId);
  return startIndex < 0 ? [] : virtualRenderItems.value.slice(startIndex);
});
const tailMetricsTick = ref(0);
let tailMetricsRaf = 0;
let tailMetricsRetry = 0;
function scheduleTailMetricsRefresh() {
  if (tailMetricsRaf) return;
  tailMetricsRaf = requestAnimationFrame(() => {
    tailMetricsRaf = 0;
    tailMetricsTick.value += 1;
    if (!latestOwnTailContentMeasured.value && latestOwnTailContentRange.value.length > 0 && tailMetricsRetry < 24) {
      tailMetricsRetry += 1;
      scheduleTailMetricsRefresh();
    } else {
      tailMetricsRetry = 0;
    }
  });
}
watch([() => virtualRenderItems.value.length, () => String(latestOwnElasticItemId.value || "").trim(), () => !!virtuaRef.value], () => {
  tailMetricsRetry = 0;
  void nextTick(() => scheduleTailMetricsRefresh());
}, { immediate: true });
onBeforeUnmount(() => {
  if (tailMetricsRaf) {
    cancelAnimationFrame(tailMetricsRaf);
    tailMetricsRaf = 0;
  }
});

const latestOwnTailContentHeight = computed(() => {
  void tailMetricsTick.value;
  const v = virtuaRef.value as unknown as { cache?: unknown } | null;
  if (!v || !Array.isArray((v as any).cache) || !Array.isArray((v as any).cache[0])) return 0;
  const sizes = (v as any).cache[0] as number[];
  let total = 0;
  for (const item of latestOwnTailContentRange.value) {
    const idx = virtualRenderItems.value.findIndex((it) => it.id === item.id);
    if (idx >= 0 && idx < sizes.length) {
      const h = sizes[idx];
      if (Number.isFinite(h) && h > 0 && h !== -1) total += h;
    }
  }
  return total;
});
const latestOwnTailContentMeasured = computed(() => {
  void tailMetricsTick.value;
  const v = virtuaRef.value as unknown as { cache?: unknown } | null;
  if (!v || !Array.isArray((v as any).cache) || !Array.isArray((v as any).cache[0])) return false;
  const sizes = (v as any).cache[0] as number[];
  const range = latestOwnTailContentRange.value;
  if (range.length === 0) return false;
  for (const item of range) {
    const idx = virtualRenderItems.value.findIndex((it) => it.id === item.id);
    if (idx < 0 || idx >= sizes.length) return false;
    const h = sizes[idx];
    if (h === -1 || !Number.isFinite(h) || h <= 0) return false;
  }
  return true;
});

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

const TIMELINE_TOOL_BREAK = "\uE000TOOLBREAK\uE000";
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

function isOwnTimelineBlock(block: ChatMessageBlock): boolean {
  const speakerId = String(block.speakerAgentId || "").trim();
  const role = String(block.role || "").trim();
  return !speakerId || speakerId === "user-persona" || role === "user";
}

function timelineSpeakerName(block: ChatMessageBlock): string {
  if (isOwnTimelineBlock(block)) return String(props.userAlias || "").trim() || "我";
  const speakerId = String(block.speakerAgentId || "").trim();
  if (speakerId && props.personaNameMap?.[speakerId]) return props.personaNameMap[speakerId];
  return String(props.personaName || "").trim() || "助理";
}

function timelineSpeakerAvatar(block: ChatMessageBlock): string {
  if (isOwnTimelineBlock(block)) return String(props.userAvatarUrl || "").trim();
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

// 蛇形时间线锚点：每条完成态用户消息一个点，附上紧随其后的助理回复尾段，供悬停预览卡显示。
// 与上面按「每条消息一个节点」的垂直时间线各管一摊：蛇板负责定位与挪动，垂直面板负责逐条浏览。
function timelineAssistantTail(raw: string): string {
  const parts = String(raw || "")
    .split(TIMELINE_TOOL_BREAK)
    .map((part) => part.replace(/\[toolcall:[^\]\n]+\]/g, "").replace(/\s+/g, " ").trim())
    .filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1]! : "";
}

const timelineAnchors = computed<Array<{ id: string; userText: string; assistantTail: string; index: number }>>(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  const idToVirtual = new Map<string, number>();
  virtualRenderItems.value.forEach((item, idx) => {
    if (item.kind === "message" && item.block) {
      const bid = String(item.block.id || "");
      if (bid) idToVirtual.set(bid, idx);
    }
  });
  const anchors: Array<{ id: string; userText: string; assistantTail: string; index: number }> = [];
  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i]!;
    if (block.isStreaming || block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (!isOwnTimelineBlock(block)) continue;
    const text = String(block.text || "");
    if (!text.trim() && block.images.length === 0 && block.audios.length === 0) continue;
    const index = idToVirtual.get(String(block.id || ""));
    if (index === undefined) continue;
    let assistantTail = "";
    for (let j = i + 1; j < blocks.length; j++) {
      const next = blocks[j]!;
      if (next.isStreaming) continue;
      if (isOwnTimelineBlock(next)) break;
      const tail = timelineAssistantTail(String(next.text || ""));
      if (tail) {
        assistantTail = tail;
        break;
      }
    }
    anchors.push({ id: String(block.id || `timeline-${i}`), userText: text, assistantTail, index });
  }
  return anchors;
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
      isOwn: isOwnTimelineBlock(block),
      isCompaction: compaction || pendingCompactionBoundary,
    });
    pendingCompactionBoundary = false;
  }
  return entries;
});

const canShowTimeline = computed(() => timelineEntries.value.length >= 2);

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
const timelineFloatOpen = ref(false);
// 悬停展开的微防抖：避免光标扫过按钮误触弹开
const TIMELINE_FLOAT_OPEN_DELAY_MS = 120;
// 收起前的逗留时间：光标在按钮与卡片之间移动时靠它兜住
const TIMELINE_FLOAT_CLOSE_DELAY_MS = 320;
let timelineFloatOpenTimer: ReturnType<typeof setTimeout> | null = null;
let timelineFloatCloseTimer: ReturnType<typeof setTimeout> | null = null;

// 时间线面板：点击「预览」按钮弹出，几乎盖满聊天区；点节点跳转到该条消息并收起面板。
const timelinePanelOpen = ref(false);

const timelineButtonVisible = computed(() =>
  canShowTimeline.value && !timelinePanelOpen.value && displayedSessionRow.value === "top",
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
function handleTimelineFloatDocumentPointerDown(event: MouseEvent | TouchEvent) {
  if (!timelineFloatPanelVisible.value) return;
  const target = event.target as Node | null;
  if (target instanceof Element && target.closest(".ecall-snake-board-card")) return;
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

const latestOwnTailSpacerMinHeight = ref(0);

watch(
  [
    latestOwnElasticItemId,
    latestOwnElasticMinHeight,
    latestOwnTailContentHeight,
    latestOwnTailContentMeasured,
    tailMetricsTick,
  ],
  ([itemId, targetHeight, tailContentHeight, tailContentMeasured]) => {
    if (!itemId) {
      latestOwnTailSpacerMinHeight.value = targetHeight;
      return;
    }
    if (!tailContentMeasured) return;

    const next = Math.max(0, targetHeight - tailContentHeight);
    if (latestOwnTailSpacerMinHeight.value !== next) {
      latestOwnTailSpacerMinHeight.value = next;
      // 留白重算只改高度，不再补滚到底：
      // 对齐语义是「最新用户消息停在视口顶部」，这里再滚一次会把它顶出可视区（跟随贴底另有 pin 负责）。
      void nextTick(() => {
        requestAnimationFrame(() => {
          const el = scrollContainer.value;
          if (!el) return;
          const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
          probeChatScroll("留白重算", {
            spacer: next,
            tailContentHeight,
            targetHeight,
            distanceToBottom: Math.round(distanceToBottom),
            followBottom: followBottom.value,
          });
        });
      });
    }
  },
  { immediate: true },
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

// 会话悬浮操作区：下排工作区 bar 贴底时出现，上排（预览条 + 时间线按钮）离底时出现
const showFloatingSessionToolbar = computed(() => {
  if (!supportsFloatingSessionToolbar.value) return false;
  return sessionControlPanelVisible.value;
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

// 会话悬浮操作区上排（预览条 + 时间线按钮）直接贴容器底边：
// 上下两排不会同时出现（贴底出下排、离底出上排），不需要为上排预留下排的高度

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
  // Git 模式下不跟随工作区强刷，避免把 Git 面板盖回文件目录
  if (String(panel.asideMode || "").trim() === "git") return;
  // 目录树已展开时才跟随；未展开保持关闭，已在目标路径则不重复加载
  const currentRoot = String(panel.directoryRootPath || "").trim();
  if (!currentRoot) return;
  const norm = (p: string) => String(p || "").trim().replace(/\\/g, "/").replace(/\/$/, "").toLowerCase();
  if (norm(currentRoot) === norm(workspaceRootPath)) return;
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

const sideChatItems = computed(() =>
  (Array.isArray(props.sideChatItems) ? props.sideChatItems : [])
    .map((item) => ({ id: String(item?.id || "").trim(), title: String(item?.title || "").trim() }))
    .filter((item) => item.id),
);

/** 主页「追问」小卡：切到该追问会话（追问面板与会话激活都由宿主持有） */
function openHomeSideChat(conversationId: string) {
  const id = String(conversationId || "").trim();
  if (!id) return;
  emit("update:chatRightPanelMode", "sideChat");
  emit("openSideChatConversation", id);
}

/** 主页「新建追问」入口卡：与追问面板右上角 ＋ 同一行为，进新建选择页（追问会话由宿主创建） */
function openHomeSideChatNewPage() {
  emit("update:chatRightPanelMode", "sideChat");
  emit("openSideChatNewPage");
}

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

/** 主页监控卡片里的概览条目点击后：切到监控面板的对应 tab。 */
function openMonitorTabFromHome(tab: ChatMonitorPanelMode) {
  emit("update:chatMonitorPanelMode", tab);
  selectChatRightPanelMode("monitor");
}

// ==================== 右侧主页预览 ====================

/** 大卡一屏最多展示的条目数，超出交给卡片显示「还有 N 项」 */
const HOME_CARD_ITEM_LIMIT = 6;

type HomeFileItem = { path: string; label: string };

const EMPTY_HOME_FILE_PREVIEW = {
  openFiles: [] as HomeFileItem[],
  activePath: "",
  openFileCount: 0,
};

/** 文件项：path 保留完整路径用于打开，label 只用于展示 */
function toHomeFileItem(path: string): HomeFileItem {
  const normalized = String(path || "").replace(/\\/g, "/").trim();
  const parts = normalized.split("/").filter(Boolean);
  return { path: normalized, label: parts.pop() || normalized };
}

/** 已打开文件与会话级持久化同步，主页不常驻阅读器面板，直接读它的会话状态 */
function readHomeOpenFiles(): { openFiles: HomeFileItem[]; activePath: string; openFileCount: number } {
  const sessionKey = String(chatFileReaderSessionKey.value || "").trim();
  if (!sessionKey || typeof window === "undefined") return { ...EMPTY_HOME_FILE_PREVIEW };
  try {
    const legacyKey = String(legacyChatFileReaderSessionKey.value || "").trim();
    const raw = window.localStorage.getItem(sessionKey)
      || (legacyKey ? window.localStorage.getItem(legacyKey) : "")
      || "{}";
    const state = JSON.parse(raw) as { tabs?: unknown; activePath?: unknown };
    const tabs = (Array.isArray(state.tabs) ? state.tabs : [])
      .map((item) => String(item || "").replace(/\\/g, "/").trim())
      .filter((path) => path && !path.startsWith("git-diff:"));
    const activeRaw = String(state.activePath || "").replace(/\\/g, "/").trim();
    const activePath = activeRaw && !activeRaw.startsWith("git-diff:") ? activeRaw : "";
    const ordered = activePath ? [activePath, ...tabs.filter((path) => path !== activePath)] : tabs;
    return {
      openFiles: ordered.slice(0, HOME_CARD_ITEM_LIMIT).map(toHomeFileItem),
      activePath,
      openFileCount: ordered.length,
    };
  } catch {
    return { ...EMPTY_HOME_FILE_PREVIEW };
  }
}

const homeFilePreview = ref({
  openFiles: [] as HomeFileItem[],
  activePath: "",
  openFileCount: 0,
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

const homeGitChanges = computed(() =>
  homeGitStatusEntries.value.map((entry) => ({
    path: String(entry?.path || ""),
    status: String(entry?.unstagedStatus || entry?.stagedStatus || ""),
  })),
);
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
  const workspace = String(props.currentWorkspaceRootPath || "").trim();
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
    String(props.currentWorkspaceRootPath || "").trim(),
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
      String(task?.completionState || "").trim() === "active"
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

let unlistenTaskChanged: (() => void) | null = null;
let unlistenTaskRecovered: (() => void) | null = null;

onMounted(() => {
  unlistenTaskChanged = onTransportNotification("monitor.changed", handleRunningTaskRefreshEvent);
  unlistenTaskRecovered = onTransportRecovered(() => {
    void refreshRunningTaskCount();
  });
});

onBeforeUnmount(() => {
  unlistenTaskChanged?.();
  unlistenTaskChanged = null;
  unlistenTaskRecovered?.();
  unlistenTaskRecovered = null;
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

watch(
  () => !isDesktopTauriHost() && leftPaneInLayout.value && rightPaneInLayout.value,
  (val) => { isWebRoundedMode.value = val; },
  { immediate: true },
);
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

// ==================== bottom follow (intent-driven) ====================

const chatContentRoot = ref<HTMLElement | null>(null);
let contentResizeObserver: ResizeObserver | null = null;

// 跟随模式下内容尺寸变化（流式增长、气泡变高）时同步贴底。
// virtua 不会在内容增长时自动维持贴底，这里补上；未进入跟随则保持视口不动。
// 另外要求视口确实仍贴着底：补载历史等程序化滚动会把 followBottom 误置真，
// 此时视口停在上方，若照样下拉会把用户从历史位置直接拽到最底。
// 容差取自实测：跟随状态下距底距离基本为 0，偶发瞬态最大 54px。
const PIN_TO_BOTTOM_TOLERANCE_PX = 64;
function pinChatToBottomWhileFollowing() {
  if (!followBottom.value) return;
  const el = scrollContainer.value;
  if (!el) return;
  const distanceToBottom = el.scrollHeight - (el.scrollTop + el.clientHeight);
  if (distanceToBottom > PIN_TO_BOTTOM_TOLERANCE_PX) return;
  const before = el.scrollTop;
  el.scrollTop = el.scrollHeight;
  if (Math.abs(el.scrollTop - before) > 1) {
    probeChatScroll("跟随贴底", {
      before: Math.round(before),
      after: Math.round(el.scrollTop),
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
    });
  }
  chatScrollbarRef.value?.updateThumb();
}

watch(chatContentRoot, (el, _prev, onCleanup) => {
  contentResizeObserver?.disconnect();
  contentResizeObserver = null;
  if (!el || typeof ResizeObserver === "undefined") return;
  contentResizeObserver = new ResizeObserver(() => {
    pinChatToBottomWhileFollowing();
  });
  contentResizeObserver.observe(el);
  onCleanup(() => {
    contentResizeObserver?.disconnect();
    contentResizeObserver = null;
  });
});

// 手动「回到底部」的滚动行为：与底部相隔在当前可视项 10 项以内走平滑，否则瞬移
function resolveManualScrollToBottomBehavior(): "auto" | "smooth" {
  const len = virtualRenderItems.value.length;
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

// 预览只表达「最新的未读内容」：把预览看成最新未读内容的预览，读过了自然就没有。
// 未读版本 = 「最新助理块的 id + 思考字数 + 执行次数」——只有新增一段思考或执行一次工具才算有新内容产出；
// 读到最底部时记下当时的版本，之后只要版本变了就还有未读，交给预览条显示内容；
// 版本一致即已读，预览内容清空，预览条自然回落到它本来就有的「回到底部」形态，不另造按钮。
const latestAssistantContentVersion = computed(() => {
  const blocks = (props.messageBlocks || []) as ChatMessageBlock[];
  for (let i = blocks.length - 1; i >= 0; i -= 1) {
    const block = blocks[i];
    if (block.isExtraTextBlock || block.remoteImOrigin) continue;
    if (String(block.role || "") !== "assistant") continue;
    const reasoningChars = Number(block.activityReasoningCharCount || 0);
    const toolCount = Number(block.toolCallCount || 0);
    return `${block.id}:${reasoningChars}:${toolCount}`;
  }
  return "";
});
const readContentVersion = ref("");
watch(atConversationBottom, (atBottom) => {
  if (!atBottom) return;
  readContentVersion.value = latestAssistantContentVersion.value;
});
const previewHasUnread = computed(() => latestAssistantContentVersion.value !== readContentVersion.value);

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

// 预览条实际收到的内容：没有未读内容时清空，让预览条自己回落到「回到底部」
const previewBlocksForBar = computed(() => (previewHasUnread.value ? thinkingPreviewBlocks.value : []));
const previewTextForBar = computed(() => (previewHasUnread.value ? idlePreviewText.value : ""));

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

// 思维链的展开状态是消息组件内部的 UI 状态，父级读不到，只能由它上报后在这里记账
const expandedActivityBlockId = ref("");

function handleActivityToggle(payload: { blockId: string; open: boolean }) {
  if (payload.open) {
    expandedActivityBlockId.value = payload.blockId;
    return;
  }
  if (expandedActivityBlockId.value === payload.blockId) expandedActivityBlockId.value = "";
}

// 正在流式的那条消息思维链已展开：用户已经在看思维链，预览条不再预览，退化成回到底部
const thinkingPreviewCollapsed = computed(
  () =>
    !!props.chatting &&
    expandedActivityBlockId.value !== "" &&
    expandedActivityBlockId.value === latestAssistantBlockId.value,
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
  codeReviewDialogOpen.value = true;
}
function closeCodeReviewDialog() {
  if (toolReviewSubmittingBatchKey.value) return;
  codeReviewDialogOpen.value = false;
  codeReviewErrorText.value = "";
}
async function loadCodeReviewCommitOptions(page = 1) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return;
  commitOptionsLoading.value = true;
  try {
    const result = await listToolReviewCommitOptions(conversationId, page, commitPageSize.value);
    commitOptions.value = Array.isArray(result.commits) ? result.commits : [];
    commitTotal.value = Number(result.total || 0);
    commitPage.value = Number(result.page || page);
    commitPageSize.value = Number(result.pageSize || commitPageSize.value);
    codeReviewErrorText.value = "";
  } catch (error) {
    commitOptions.value = [];
    codeReviewErrorText.value = t("chat.readCommitFailed");
    console.error("[代码审查] 读取 commit 失败", error);
  } finally {
    commitOptionsLoading.value = false;
  }
}
async function handleSubmitCodeReview(input: { scope: ToolReviewCodeReviewScope; target?: string; agentId: string }) {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId || toolReviewSubmittingBatchKey.value) return;
  codeReviewErrorText.value = "";
  const report = await submitToolReviewCode({
    conversationId,
    scope: input.scope,
    target: String(input.target || "").trim() || undefined,
    agentId: String(input.agentId || "").trim() || undefined,
  });
  if (!report) {
    codeReviewErrorText.value = t("chat.startCodeReviewFailed");
    return;
  }
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

function handleSendChat() {
  const conversationId = String(props.activeConversationId || "").trim();
  // 用户按下回车/点发送即视为转正：前端立刻隐藏草稿选择卡，不等后端水位线。
  if (conversationId && activeConversationIsDraft.value) {
    locallyPromotedConversationId.value = conversationId;
  }
  const extraTextBlocks = attachedIdeContextReferences.value.map((item) => String(item.textBlock || "").trim()).filter(Boolean);
  emit("sendChat", extraTextBlocks.length > 0 ? { extraTextBlocks } : undefined);
  clearAttachedIdeContextReferences();
}
function handleConversationListSelect(payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }) {
  const id = String(payload?.conversationId || "").trim();
  if (!id || id === String(props.activeConversationId || "").trim()) return;
  const target = (props.conversationItems || props.unarchivedConversationItems).find((item) => String(item.conversationId || "").trim() === id);
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
      pinned: t("chat.pinnedConversations"),
      other: t("chat.otherConversations"),
      defaultWorkspace: t("chat.defaultWorkspace"),
      currentProject: t("chat.currentProject"),
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

async function handleDraftWorkspaceSave(workspaces: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, branch?: string) {
  if (!props.saveDraftWorkspaces) return;
  await props.saveDraftWorkspaces(workspaces, autonomousMode, workMode, branch);
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
  unlistenFileReaderAddToChat = onTransportNotification<IdeContextReferenceItem>("fileReader.addToChat", (payload) => {
    handleAddFileReaderContextReference(payload);
  });
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
