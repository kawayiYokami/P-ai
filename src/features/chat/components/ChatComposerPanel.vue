<template>
  <div>
    <div
      v-if="linkOpenErrorText"
      class="alert alert-warning mb-2 py-2 px-3 text-sm whitespace-pre-wrap break-all max-h-24 overflow-auto"
    >
      <span>{{ linkOpenErrorText }}</span>
    </div>
    <ChatSelectionActionPanel
      v-if="selectionModeEnabled"
      :show-conversation-actions="showConversationActions"
      :selected-message-count="selectedMessageCount"
      :active-conversation-id="activeConversationId"
      :unarchived-conversation-items="unarchivedConversationItems"
      :remote-im-contact-conversations="remoteImContactConversations"
      @exit-selection-mode="emit('exitSelectionMode')"
      @selection-action-branch="emit('selectionActionBranch')"
      @selection-action-forward="emit('selectionActionForward', $event)"
      @selection-action-copy="emit('selectionActionCopy')"
      @selection-action-share="emit('selectionActionShare', $event)"
    />
    <template v-else>
    <div v-if="systemNotificationMode" class="flex flex-wrap items-center justify-center gap-2">
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openDelegateSelection')"
      >
        <AppIcon name="clipboard-list" class="h-3.5 w-3.5" />
        {{ t("chat.conversationMenu.groupDelegate") }}
      </button>
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openTimerCreate')"
      >
        <AppIcon name="clock" class="h-3.5 w-3.5" />
        {{ t("chat.newTimer") }}
      </button>
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="openCreateConversationDialog"
      >
        <AppIcon name="plus" class="h-3.5 w-3.5" />
        {{ t("chat.newConversation") }}
      </button>
    </div>
    <div v-else-if="remoteContactMode" class="flex flex-wrap items-center justify-center gap-2">
      <ChatModelPicker
        variant="chip"
        class="min-w-0 max-w-full"
        :model-value="activeModelDisplayId"
        :api-configs="chatModelOptions"
        :theme="teleportTheme"
        @update:model-value="selectConversationPreferredModel"
      />
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openDelegateSelection')"
      >
        <AppIcon name="clipboard-list" class="h-3.5 w-3.5" />
        {{ t("chat.conversationMenu.groupDelegate") }}
      </button>
      <button
        type="button"
        class="btn btn-sm gap-2"
        :disabled="frozen || busy"
        @click="emit('openTimerCreate')"
      >
        <AppIcon name="clock" class="h-3.5 w-3.5" />
        {{ t("chat.newTimer") }}
      </button>
    </div>
    <template v-else>
    <InputPanelDock
      :deck-extra-visible="!!wheelchairSuggestion"
      :queue-events="queueEnabled && !systemNotificationMode && !remoteContactMode ? visibleQueueEvents : []"
      :user-persona-name="queueUserPersonaName"
      :queue-visible="props.queueVisible ?? true"
      :is-rounded="!!isRounded"
      dock-bg="base-200"
      @recall-to-input="handleRecallToInput"
      @mark-guided="handleQueueMarkGuided"
    >
        <template #deck-extra>
          <!-- 护航建议：贴在底盘露头层，与队列同层，不进输入卡内部 -->
          <div
            v-if="wheelchairSuggestion"
            class="flex items-start gap-2 px-2 py-1 text-xs"
          >
            <template v-if="wheelchairSuggestion.type === 'suggest'">
              <!-- 单条建议是给用户读的点评，不是可以直接发出去的话，所以只展示不填入 -->
              <span class="min-w-0 flex-1 text-sm">{{ wheelchairSuggestion.text }}</span>
            </template>
            <template v-else-if="wheelchairSuggestion.type === 'options'">
              <div class="flex min-w-0 flex-1 flex-wrap gap-1">
                <button
                  v-for="(opt, idx) in wheelchairSuggestion.options"
                  :key="idx"
                  type="button"
                  class="inline-flex max-w-full items-start gap-1.5 rounded-lg border border-base-300 bg-base-100 px-3 py-2 text-left text-sm transition-colors hover:border-primary hover:text-primary"
                  @click="fillWheelchairSuggestion(opt)"
                >
                  <span class="shrink-0 tabular-nums text-primary">{{ idx + 1 }}.</span>
                  <span class="min-w-0">{{ opt }}</span>
                </button>
              </div>
            </template>
            <button
              type="button"
              class="mt-0.5 inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-base-content/50 transition hover:bg-base-300 hover:text-base-content"
              :title="t('common.close')"
              :aria-label="t('common.close')"
              @click="emit('dismissWheelchairSuggestion')"
            >
              <AppIcon name="close" class="h-3.5 w-3.5" />
            </button>
          </div>
        </template>
        <template #deck-top>
          <div v-if="showLegacyActionRow" class="flex items-center gap-1 pb-1 pl-1">
            <button type="button" class="btn btn-sm btn-ghost h-7 min-h-7 w-7 rounded-md px-0" title="@ 提及">
              <AppIcon name="at-sign" class="h-4 w-4" />
            </button>
            <button type="button" class="btn btn-sm btn-ghost h-7 min-h-7 w-7 rounded-md px-0" title="# 引用">
              <AppIcon name="hash" class="h-4 w-4" />
            </button>
            <button type="button" class="btn btn-sm btn-ghost h-7 min-h-7 w-7 rounded-md px-0" title="图片">
              <AppIcon name="image" class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="btn btn-sm btn-ghost h-7 min-h-7 w-7 rounded-md px-0"
              :title="t('chat.attach')"
              @pointerdown.prevent
              @mousedown.prevent
              @click="emit('pickAttachments')"
            >
              <AppIcon name="folder" class="h-4 w-4" />
            </button>
          </div>
        </template>
        <template #attachments>
          <div v-if="attachedIssueReferences.length > 0" class="mb-2 flex flex-wrap gap-1">
            <span
              v-for="item in attachedIssueReferences"
              :key="`issue-ref-${item.number}`"
              class="badge gap-1 bg-base-300 px-3 py-3 text-sm text-base-content border-transparent"
            >
              <span class="max-w-56 truncate leading-none">{{ buildKanbanIssueReferenceLabel(item) }}</span>
              <button
                type="button"
                class="ml-0.5 inline-flex h-5 w-5 items-center justify-center rounded-full text-base-content transition hover:bg-error hover:text-error-content"
                :title="t('chat.kanban.removeIssueReference')"
                @click.stop="removeIssueReference(item.number)"
              >
                <AppIcon name="close" class="h-3 w-3" />
              </button>
            </span>
          </div>
          <div v-if="selectedMentions.length > 0" class="mb-2 flex flex-wrap gap-1">
            <span
              v-for="item in selectedMentions"
              :key="item.agentId"
              class="badge gap-1 bg-base-300 px-3 py-3 text-sm text-base-content border-transparent"
            >
              <span class="max-w-40 truncate leading-none">@{{ mentionDisplayLabel(item) }}</span>
              <button
                type="button"
                class="ml-0.5 inline-flex h-5 w-5 items-center justify-center rounded-full text-base-content transition hover:bg-error hover:text-error-content"
                @click.stop="removeSelectedMention(item)"
              >
                <AppIcon name="close" class="h-3 w-3" />
              </button>
            </span>
          </div>
          <InputPanelAttachments
            :images="panelAttachmentImages"
            :files="panelAttachmentFiles"
            :bridges="panelBridgeItems"
            @remove-image="removeClipboardImageAt($event)"
            @remove-file="removePanelAttachmentFile($event)"
            @toggle-bridge="togglePanelBridge($event)"
          />
        </template>
        <template #body>
          <!-- 提问内容与普通输入是同一种容器内的两种内容形态：
               有此行时整块内容换成提问卡，外框（底座、露头、圆角、底色）完全不动。 -->
          <slot v-if="questionExpanded" name="question" />
          <div v-else ref="composerRootRef" class="relative">
            <Transition name="ecall-dropdown-up">
              <div
                v-if="instructionPanelOpen"
                class="absolute bottom-full left-0 right-0 z-30 mb-1.5 overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
              >
                <div class="flex flex-wrap content-start gap-2 max-h-48 overflow-y-auto p-2">
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
              </div>
            </Transition>
            <Transition name="ecall-dropdown-up">
              <div
                v-if="mentionPanelOpen"
                class="absolute bottom-full left-0 z-30 mb-1.5 w-max max-w-[min(80vw,20rem)] overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
              >
                <div
                  ref="mentionPanelScrollRef"
                  class="max-h-[min(56vh,24rem)] overflow-y-auto overscroll-contain p-1"
                >
                  <ul class="flex flex-col gap-1">
                    <li
                      v-for="(item, index) in filteredMentionOptions"
                      :key="item.agentId"
                    >
                      <button
                        type="button"
                        :data-mention-option-index="index"
                        class="flex min-h-0 w-full items-start gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors"
                        :class="[
                          mentionFocusIndex === index ? 'bg-base-200' : '',
                          item.mentionable ? 'hover:bg-base-200/80' : 'opacity-65',
                        ]"
                        :disabled="!item.mentionable"
                        @click="applyMention(item)"
                      >
                        <div class="indicator shrink-0">
                          <span
                            v-if="isMentionSelected(item)"
                            class="indicator-item inline-flex h-4 w-4 items-center justify-center rounded-full bg-primary text-micro font-bold text-primary-content"
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
                              <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-caption">
                                {{ avatarInitial(item.agentName) }}
                              </div>
                            </div>
                          </div>
                        </div>
                        <div class="min-w-0 flex-1 pr-0.5">
                          <div class="truncate text-sm leading-5">@{{ mentionDisplayLabel(item) }}</div>
                          <div
                            v-if="!item.mentionable && item.unavailableReason"
                            class="truncate text-xs leading-4 text-base-content/60"
                          >
                            {{ item.unavailableReason }}
                          </div>
                        </div>
                      </button>
                    </li>
                  </ul>
                  <div v-if="filteredMentionOptions.length === 0" class="px-2.5 py-2 text-sm opacity-60">
                    {{ t("chat.noMentionCandidates") }}
                  </div>
                </div>
              </div>
            </Transition>
            <Transition name="ecall-dropdown-up">
              <div
                v-if="issuePanelOpen"
                class="absolute bottom-full left-0 z-30 mb-1.5 w-max max-w-[min(80vw,20rem)] overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
              >
                <div
                  ref="issuePanelScrollRef"
                  class="max-h-[min(56vh,24rem)] overflow-y-auto overscroll-contain p-1"
                >
                  <div class="flex flex-col gap-1">
                    <div v-for="group in issueOptionGroups" :key="group.status">
                      <div class="px-2 pb-0.5 pt-1 text-caption text-base-content/45">{{ group.label }}</div>
                      <ul class="flex flex-col gap-1">
                        <li
                          v-for="{ issue: item, index } in group.items"
                          :key="item.number"
                        >
                          <button
                            type="button"
                            :data-issue-option-index="index"
                            class="flex min-h-0 w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left text-base-content transition-colors"
                            :class="issueFocusIndex === index ? 'bg-base-200' : 'hover:bg-base-200/80'"
                            @click="applyIssueOption(item)"
                          >
                            <span class="shrink-0 font-mono text-xs tabular-nums text-base-content/55">#{{ item.number }}</span>
                            <span class="min-w-0 flex-1 truncate text-sm leading-5">{{ item.title }}</span>
                          </button>
                        </li>
                      </ul>
                    </div>
                  </div>
                </div>
              </div>
            </Transition>
            <textarea
              ref="chatInputRef"
              v-model="localChatInput"
              class="ecall-chat-composer-input block max-h-40 min-h-8 w-full resize-none overflow-y-auto bg-transparent px-2 py-1 text-sm leading-6 outline-none chat-input-no-focus"
            rows="1"
            :placeholder="effectiveChatInputPlaceholder"
            @input="handleChatInputInput"
            @compositionstart="handleChatInputCompositionStart"
            @compositionend="handleChatInputCompositionEnd"
            @keydown="handleChatInputKeydown"
            @focus="handleChatInputFocus"
            @blur="handleChatInputBlur"
          ></textarea>
            <FloatingScrollbar v-if="chatInputRef" :target="chatInputRef" />
          </div>
        </template>
        <template #footer>
          <InputPanelToolbar>
            <template #left>
              <div v-if="!questionExpanded" class="flex min-w-0 items-center gap-1">
                <EcallDropdown
                  v-model="plusPanelOpen"
                  teleport
                  placement="top"
                  :match-trigger-width="false"
                  root-class="w-auto min-w-0"
                  panel-class="w-48"
                >
                  <template #trigger="{ toggle }">
                    <button
                      type="button"
                      class="btn btn-sm btn-ghost h-7 min-h-7 w-7 rounded-md px-0"
                      :title="t('chat.moreActions')"
                      @click="toggle"
                    >
                      <AppIcon name="plus" class="h-4 w-4" />
                    </button>
                  </template>
                  <template #default="{ close }">
                    <ul class="menu w-full p-1">
                      <li>
                        <button type="button" @click="close(); emit('pickAttachments', 'image')">
                          <AppIcon name="image" class="h-4 w-4 shrink-0" />
                          <span class="min-w-0 flex-1 truncate">{{ t("chat.openImage") }}</span>
                        </button>
                      </li>
                      <li>
                        <button type="button" @click="close(); emit('pickAttachments')">
                          <AppIcon name="folder" class="h-4 w-4 shrink-0" />
                          <span class="min-w-0 flex-1 truncate">{{ t("chat.openFile") }}</span>
                        </button>
                      </li>
                      <li>
                        <button type="button" @click="close(); insertChatInputTrigger('#')">
                          <AppIcon name="hash" class="h-4 w-4 shrink-0" />
                          <span class="min-w-0 flex-1 truncate">{{ t("chat.referenceIssue") }}</span>
                        </button>
                      </li>
                    </ul>
                  </template>
                </EcallDropdown>
                <EcallDropdown
                  v-model="modePanelOpen"
                  teleport
                  placement="top"
                  :match-trigger-width="false"
                  root-class="w-auto min-w-0"
                  panel-class="w-56 overflow-visible!"
                >
                  <template #trigger="{ toggle }">
                    <button
                      type="button"
                      class="btn btn-sm h-7 min-h-7 w-7 rounded-md px-0"
                      :class="modeButtonClass"
                      :title="t('chat.mode.buttonTitle')"
                      @click="toggle"
                    >
                      <AppIcon :name="modeButtonIcon" class="h-4 w-4" />
                    </button>
                  </template>
                  <template #default="{ close }">
                  <ul class="menu w-full p-1">
                    <li>
                      <button
                        type="button"
                        class="tooltip tooltip-right"
                        :data-tip="t('chat.plan.modeHint', { personaName })"
                        @click="close(); togglePlanMode()"
                      >
                        <AppIcon
                          name="quill-write-02"
                          class="h-4 w-4 shrink-0"
                          :class="planModeEnabled ? 'text-info' : ''"
                        />
                        <span class="min-w-0 flex-1 truncate">{{ t("chat.plan.mode") }}</span>
                        <span v-if="planModeEnabled" class="badge badge-sm shrink-0 badge-info">
                          {{ t("chat.plan.exitMode") }}
                        </span>
                      </button>
                    </li>
                    <li>
                      <button
                        type="button"
                        class="tooltip tooltip-right"
                        :data-tip="t('chat.wheelchair.modeHint')"
                        @click="close(); toggleWheelchairMode()"
                      >
                        <AppIcon
                          name="wheelchair"
                          class="h-4 w-4 shrink-0"
                          :class="wheelchairModeEnabled ? 'text-warning' : ''"
                        />
                        <span class="min-w-0 flex-1 truncate">{{ t("chat.wheelchair.mode") }}</span>
                        <span v-if="wheelchairModeEnabled" class="badge badge-sm shrink-0 badge-warning">
                          {{ t("chat.wheelchair.exitMode") }}
                        </span>
                      </button>
                    </li>
                  </ul>
                  <div class="my-1 border-t border-base-300" />
                  <ul class="menu w-full p-1">
                    <li>
                      <button
                        type="button"
                        class="tooltip tooltip-right"
                        :data-tip="t('chat.goal.modeHint', { personaName })"
                        @click="toggleGoalSection()"
                      >
                        <AppIcon
                          name="target"
                          class="h-4 w-4 shrink-0"
                          :class="goalActive ? 'text-primary' : ''"
                        />
                        <span class="min-w-0 flex-1 truncate">{{ t("chat.goal.goalLabel") }}</span>
                        <span v-if="goalActive" class="badge badge-sm shrink-0 badge-primary">
                          {{ t("chat.goal.activeBadge") }}
                        </span>
                        <AppIcon
                          name="chevron-down"
                          class="h-3.5 w-3.5 shrink-0 transition-transform"
                          :class="goalSectionOpen ? 'rotate-180' : ''"
                        />
                      </button>
                    </li>
                  </ul>
                  <div
                    class="grid transition-[grid-template-rows] duration-200 ease-out"
                    :class="goalSectionOpen || goalActive ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'"
                  >
                    <div class="min-h-0 overflow-hidden">
                      <div class="px-2 pb-2">
                        <div v-if="goalActive" class="flex items-center gap-2">
                          <span class="min-w-0 flex-1 truncate text-caption text-base-content/70">{{ goalTitle }}</span>
                          <button
                            type="button"
                            class="btn btn-xs btn-error shrink-0"
                            :disabled="frozen"
                            @click="emit('stopGoalTask')"
                          >{{ t("chat.goal.stopAction") }}</button>
                        </div>
                        <template v-else>
                          <textarea
                            v-model="goalObjectiveDraft"
                            class="textarea textarea-bordered w-full text-sm"
                            rows="3"
                            :placeholder="t('chat.goal.goalPlaceholder')"
                            @keydown.enter.exact.prevent="submitGoalObjective"
                          ></textarea>
                          <button
                            type="button"
                            class="btn btn-sm btn-primary mt-2 w-full"
                            :disabled="!goalObjectiveDraft.trim() || frozen || goalDisabled"
                            @click="submitGoalObjective"
                          >{{ t("chat.goal.createAction") }}</button>
                        </template>
                      </div>
                    </div>
                  </div>
                  </template>
                </EcallDropdown>
                <ChatModelPicker
                  variant="chip"
                  class="min-w-0 max-w-full"
                  :model-value="activeModelDisplayId"
                  :api-configs="chatModelOptions"
                  :theme="teleportTheme"
                  @update:model-value="selectConversationPreferredModel"
                />
              </div>
            </template>
            <template #right>
              <!-- 问题图标：有待答问题时高亮，点一下在提问卡与普通输入之间切换。
                   它留在同一张输入卡右下角，所以收起后仍然看得见还有题没答。 -->
              <button
                v-if="questionActive"
                type="button"
                class="btn btn-sm btn-circle shrink-0 transition-transform duration-150 ease-out active:scale-90"
                :class="questionExpanded ? 'btn-warning' : 'text-warning btn-ghost'"
                :title="questionExpanded ? t('chat.questionPanel.collapse') : t('chat.questionPanel.expand')"
                @pointerdown.prevent
                @mousedown.prevent
                @click="emit('toggleQuestion')"
              >
                <AppIcon name="help-circle" class="h-4 w-4" />
              </button>
              <template v-if="!questionExpanded">
              <EcallDropdown
                v-model="usagePanelOpen"
                teleport
                placement="top"
                :match-trigger-width="false"
                root-class="w-auto min-w-0"
                panel-class="w-80"
              >
                <template #trigger="{ toggle }">
                  <button
                    type="button"
                    class="btn btn-sm btn-ghost btn-square h-7 min-h-7 w-7"
                    :title="t('chat.contextUsageTitle', { percent: normalizedChatUsagePercent })"
                    @click="toggle"
                  >
                    <svg class="h-5 w-5 -rotate-90" viewBox="0 0 36 36">
                      <circle
                        cx="18"
                        cy="18"
                        r="14"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="4"
                        class="opacity-20"
                      />
                      <circle
                        cx="18"
                        cy="18"
                        r="14"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="4"
                        stroke-linecap="round"
                        :stroke-dasharray="ringCircumference"
                        :stroke-dashoffset="ringDashoffset"
                      />
                    </svg>
                  </button>
                </template>
                <template #default="{ close }">
                  <div class="flex w-80 flex-col gap-2 p-3">
                    <div class="flex h-2 overflow-hidden rounded-full bg-base-300">
                      <span
                        v-for="row in usageRows"
                        :key="row.key"
                        class="h-full"
                        :style="{ width: `${usagePercentOf(row.tokens)}%`, backgroundColor: row.color }"
                      />
                    </div>
                    <ul class="flex flex-col gap-1">
                      <li v-for="row in usageRows" :key="row.key" class="flex items-center gap-2 text-xs">
                        <span class="h-2 w-2 shrink-0 rounded-full" :style="{ backgroundColor: row.color }" />
                        <span class="min-w-0 flex-1 truncate">{{ row.label }}</span>
                        <span class="shrink-0 tabular-nums text-base-content/70">{{ formatUsageTokens(row.tokens) }}</span>
                        <span class="w-9 shrink-0 text-right tabular-nums text-base-content/50">{{ usagePercentOf(row.tokens) }}%</span>
                      </li>
                    </ul>
                    <div class="flex items-center justify-between text-xs text-base-content/70">
                      <span>{{ t("dialogs.trim.breakdownContextWindow") }}</span>
                      <span class="tabular-nums">{{ formatUsageTokens(usageWindowTokens) }}</span>
                    </div>
                    <div class="flex justify-end">
                      <button
                        type="button"
                        class="btn btn-sm gap-1"
                        @click="close(); emit('trim-conversation')"
                      >
                        <AppIcon name="arrow-shrink" class="h-3.5 w-3.5" />
                        {{ t("chat.contextUsageCompact") }}
                      </button>
                    </div>
                  </div>
                </template>
              </EcallDropdown>
              <button
                v-if="showStopAction"
                type="button"
                class="btn btn-sm btn-circle shrink-0 btn-error transition-transform duration-150 ease-out active:scale-90"
                :disabled="frozen"
                :title="`${t('chat.stop')} / ${t('chat.stopReplying')}`"
                @pointerdown.prevent
                @mousedown.prevent
                @click="emit('stopChat')"
              >
                <AppIcon name="square" class="h-3.5 w-3.5 fill-current" />
              </button>
              <div v-if="!composerInputBlank || !showStopAction" ref="sendModeMenuRef" class="relative flex shrink-0">
                <button
                  type="button"
                  class="btn btn-sm btn-circle shrink-0 transition-transform duration-150 ease-out active:scale-90"
                  :class="composerInputBlank ? 'bg-base-300' : 'btn-success'"
                  :disabled="!composerInputBlank && (frozen || busy)"
                  :title="composerInputBlank ? t('chat.sendModeMenu') : t('chat.send')"
                  @pointerdown.prevent
                  @mousedown.prevent
                  @click="composerInputBlank ? (sendModeMenuOpen = !sendModeMenuOpen) : handleSendChat()"
                  @contextmenu.prevent="sendModeMenuOpen = !sendModeMenuOpen"
                >
                  <AppIcon name="arrow-up" class="h-3.5 w-3.5" />
                </button>
                <div
                  v-if="sendModeMenuOpen"
                  class="absolute bottom-full right-0 z-50 mb-1.5 min-w-52 overflow-hidden rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
                >
                  <div class="flex flex-col p-1">
                    <button
                      type="button"
                      class="flex min-h-8 w-full items-center justify-between gap-3 rounded-lg px-2.5 text-left text-sm transition-colors hover:bg-base-200"
                      @pointerdown.prevent
                      @mousedown.prevent
                      @click="setSendMode('enter')"
                    >
                      <span>{{ t("chat.sendModeEnter") }}</span>
                      <AppIcon name="check" v-if="sendMode === 'enter'" class="h-4 w-4 shrink-0 text-primary" />
                    </button>
                    <button
                      type="button"
                      class="flex min-h-8 w-full items-center justify-between gap-3 rounded-lg px-2.5 text-left text-sm transition-colors hover:bg-base-200"
                      @pointerdown.prevent
                      @mousedown.prevent
                      @click="setSendMode('ctrl_enter')"
                    >
                      <span>{{ t("chat.sendModeCtrlEnter") }}</span>
                      <AppIcon name="check" v-if="sendMode === 'ctrl_enter'" class="h-4 w-4 shrink-0 text-primary" />
                    </button>
                    <div class="px-2.5 pt-1 pb-0.5 text-xs opacity-50">{{ t("chat.sendModeAltS") }}</div>
                  </div>
                </div>
              </div>
              </template>
            </template>
          </InputPanelToolbar>
        </template>
    </InputPanelDock>
    </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ApiConfigItem, ChatConversationOverviewItem, ChatMentionEntry, ChatMentionTarget, ConversationForwardTarget, IdeContextReferenceItem, IdeContextWorkspaceGroup, PromptCommandPreset, RemoteImContactConversationOption } from "../../../types/app";
import ChatSelectionActionPanel from "./ChatSelectionActionPanel.vue";
import ChatModelPicker from "../../config/components/ApiConfigPicker.vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import InputPanelAttachments from "./input-panel/InputPanelAttachments.vue";
import InputPanelDock from "./input-panel/InputPanelDock.vue";
import InputPanelToolbar from "./input-panel/InputPanelToolbar.vue";
import { useChatQueue, type ChatQueueEvent } from "../composables/use-chat-queue";
import { isMobileTouchViewport } from "../../shared/utils/mobile-viewport";
import { chatInputEnterConfirmsComposition } from "../composables/chat-composer-ime";
import { clearChatComposerFocus, registerChatComposerFocus } from "../composables/chat-composer-focus";
import EcallDropdown from "../../shared/components/EcallDropdown.vue";
import type { AppIconName } from "../../shared/icons/icon-map";
import { ideContextReferenceDisplayParts } from "../utils/ide-context-reference-display";
import { mergeComposerIdeContextGroups } from "../utils/ide-context-reference-groups";
import {
  buildKanbanIssueReferenceLabel,
  filterKanbanIssueOptions,
  findKanbanIssueTrigger,
  groupKanbanIssueOptions,
  sortKanbanIssueOptions,
  type KanbanIssueReference,
} from "../composables/use-kanban-issue-references";
import type { KanbanIssue } from "../composables/use-kanban-board";
import { useConversationMaintenanceDialog } from "../composables/use-conversation-maintenance-dialog";

type BinaryAttachment = { mime: string; bytesBase64: string; previewDataUrl?: string };
type QueuedAttachmentNotice = { id: string; fileName: string; path: string; mime: string; pending?: boolean };
type MentionOptionView = {
  agentId: string;
  agentName: string;
  avatarUrl?: string;
  mentionable: boolean;
  hidden?: boolean;
  unavailableReason?: string;
};

const props = withDefaults(defineProps<{
  composerScope?: "main" | "side";
  selectionModeEnabled: boolean;
  selectedMessageCount: number;
  chatInput: string;
  instructionPresets: PromptCommandPreset[];
  mentionEntries: ChatMentionEntry[];
  selectedMentions: ChatMentionTarget[];
  clipboardImages: BinaryAttachment[];  queuedAttachmentNotices: QueuedAttachmentNotice[];
  linkOpenErrorText: string;
  conversationCallPrimaryApiConfigId: string;
  preferredChatModelId?: string;
  chatModelOptions: ApiConfigItem[];
  workspaceAccess?: "approval" | "full_access" | "";
  planModeEnabled: boolean;
  wheelchairModeEnabled?: boolean;
  wheelchairSuggestion?: { type: "suggest" | "options"; text: string; options: string[] } | null;
  chatting: boolean;
  /** 提交期：发送请求尚未返回。此时停止入口也应可见并可用。 */
  submitPending?: boolean;
  frontendRoundPhase?: "idle" | "queued" | "waiting" | "streaming";
  busy: boolean;
  frozen: boolean;
  goalActive: boolean;
  goalTitle: string;
  goalDisabled?: boolean;
  systemNotificationMode?: boolean;
  remoteContactMode?: boolean;
  showSideConversationList: boolean;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  remoteImContactConversations: RemoteImContactConversationOption[];
  userAlias: string;
  userAvatarUrl: string;
  personaName: string;
  personaNameMap: Record<string, string>;
  defaultCreateConversationAgentId: string;
  ideContextGroups: IdeContextWorkspaceGroup[];
  attachedIdeContextReferences: IdeContextReferenceItem[];
  /** `#` 候选只列当前会话的议题，空数组时按 `#` 不弹面板。 */
  kanbanIssues?: KanbanIssue[];
  /** 已挂上的议题引用：磁贴点击与 `#` 选中都写进这一份。 */
  attachedIssueReferences?: KanbanIssueReference[];
  currentTheme?: string;
  showConversationActions?: boolean;
  /** 附件入口独立于会话操作：追问面板没有会话操作，但同样要能加附件。 */
  showAttachAction?: boolean;
  chatUsagePercent?: number;
  isRounded?: boolean;
  queueEventsOverride?: ChatQueueEvent[] | null;
  queueVisible?: boolean;
  /** 当前会话是否有待回答的问题（会话提问或终端审批）。有则右下角显示问题图标并高亮。 */
  questionActive?: boolean;
  /** 输入卡内部当前显示的是提问内容还是普通输入 */
  questionExpanded?: boolean;
}>(), {
  /** Vue 会把未传的可选布尔 prop 转成 false 而不是 undefined，必须显式默认显示，否则生产面板队列永远隐藏 */
  queueVisible: true,
});

const emit = defineEmits<{
  (e: "exitSelectionMode"): void;
  (e: "selectionActionBranch"): void;
  (e: "selectionActionForward", target: ConversationForwardTarget): void;
  (e: "selectionActionCopy"): void;
  (e: "selectionActionShare", format: "html" | "png" | "copyPng"): void;
  (e: "update:chatInput", value: string): void;
  (e: "addMention", value: ChatMentionTarget): void;
  (e: "removeMention", value: string | { agentId: string }): void;
  (e: "removeClipboardImage", index: number): void;
  (e: "removeQueuedAttachmentNotice", index: number): void;
  (e: "pickAttachments", kind?: "image" | "file"): void;
  (e: "update:conversationPreferredApiConfigId", value: string): void;
  (e: "update:workspaceAccess", value: "approval" | "full_access"): void;
  (e: "update:planModeEnabled", value: boolean): void;
  (e: "update:wheelchairModeEnabled", value: boolean): void;
  (e: "dismissWheelchairSuggestion"): void;
  (e: "attachIdeContextReference", value: IdeContextReferenceItem): void;
  (e: "removeIdeContextReference", value: string): void;
  /** 磁贴 `#N` 与 `#` 候选选中走同一个入口：只上报议题，引用由上层那份列表收口。 */
  (e: "attachKanbanIssueReference", issue: KanbanIssue): void;
  (e: "removeKanbanIssueReference", number: number): void;
  (e: "sendChat"): void;
  (e: "stopChat"): void;
  (e: "openDelegateSelection"): void;
  (e: "openTimerCreate"): void;
  /** 模式面板里直接填目标内容启动，不走完整的目标弹窗 */
  (e: "startGoalTask", objective: string): void;
  (e: "stopGoalTask"): void;
  (e: "open-conversation-list"): void;
  (e: "open-settings"): void;
  (e: "trim-conversation"): void;
  (e: "queueRecall", event: ChatQueueEvent): void;
  (e: "queueMarkGuided", eventId: string): void;
  (e: "createConversation", input?: { agentId?: string }): void;
  /** 点输入卡右下角的问题图标：在提问内容与普通输入之间切换 */
  (e: "toggleQuestion"): void;
}>();

const { t } = useI18n();
const queueEnabled = computed(() => true);
const showConversationActions = computed(() => props.showConversationActions ?? true);
const showAttachAction = computed(() => props.showAttachAction ?? (props.showConversationActions ?? true));
const systemNotificationMode = computed(() => !!props.systemNotificationMode);
const remoteContactMode = computed(() => !!props.remoteContactMode);

/** 输入区无可发送内容（无文字、无图片、无待发附件）时，发送按钮降级为菜单入口。 */
const composerInputBlank = computed(() => {
  if (String(props.chatInput || "").trim()) return false;
  return props.clipboardImages.length === 0 && props.queuedAttachmentNotices.length === 0;
});

/** 计划类请求关键词：命中时显示可点击的「计划」按钮（base-200），点击进入计划模式。 */
const PLAN_SUGGESTION_KEYWORDS = ["计划", "方案", "plan", "design"];
const planSuggestionVisible = computed(() => {
  if (props.planModeEnabled) return false;
  const text = String(props.chatInput || "").toLowerCase();
  if (!text) return false;
  return PLAN_SUGGESTION_KEYWORDS.some((keyword) => text.includes(keyword.toLowerCase()));
});

// Product rule: an in-flight assistant reply must not lock the input toolbar.
// Users can keep typing while streaming, so do not use `chatting` as the disabled
// condition for attach/record/command/task/delegate actions. Only gate on real
// hard blockers such as frozen state, explicit busy flows, permissions, or action-specific prerequisites.
const teleportTheme = computed(() => {
  const documentTheme = typeof document === "undefined" ? "" : document.documentElement.getAttribute("data-theme");
  return String(props.currentTheme || documentTheme || "light").trim() || "light";
});

function openCreateConversationDialog() {
  if (typeof window === "undefined") {
    emit("createConversation");
    return;
  }
  window.dispatchEvent(new CustomEvent("easy-call:open-draft-conversation"));
}

const menuOpen = ref(false);
const menuTriggerRef = ref<HTMLButtonElement | null>(null);
const menuWrapperRef = ref<HTMLDivElement | null>(null);

function closeMenu() {
  menuOpen.value = false;
}

function handleOpenHistory() {
  closeMenu();
  emit('open-conversation-list');
}

function handleOpenConfig() {
  closeMenu();
  emit('open-settings');
}

function onMenuOutsideClick(event: MouseEvent) {
  const target = event.target as Node | null;
  if (menuOpen.value) {
    if (menuWrapperRef.value && menuWrapperRef.value.contains(target)) return;
    closeMenu();
  }
  if (sendModeMenuOpen.value) {
    const sendModeRoot = sendModeMenuRef.value;
    if (sendModeRoot && sendModeRoot.contains(target)) return;
    sendModeMenuOpen.value = false;
  }
  if (instructionPanelOpen.value) {
    const root = composerRootRef.value;
    if (!root || !root.contains(target)) {
      closeInstructionPanel();
    }
  }
  if (mentionPanelOpen.value) {
    const mentionRoot = mentionPanelScrollRef.value;
    const inputEl = chatInputRef.value;
    const insideMention = !!mentionRoot && mentionRoot.contains(target);
    const insideInput = !!inputEl && inputEl.contains(target);
    const insideComposer = !!composerRootRef.value && composerRootRef.value.contains(target);
    if (!insideMention && !insideInput && !insideComposer) {
      closeMentionPanel();
    }
  }
  if (issuePanelOpen.value) {
    const root = composerRootRef.value;
    if (!root || !root.contains(target)) closeIssuePanel();
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onMenuOutsideClick);
});
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onMenuOutsideClick);
});

const { queueEvents, refreshQueue, recallQueueEvent, markGuided } = useChatQueue({
  enabled: queueEnabled,
});

// 切会话回来时补拉一次队列：在他会话期间本会话队列发出去了，
// 期间那次快照推送浏览器可能没接住，不补就会看到历史已刷出、队列还挂着。
watch(
  () => String(props.activeConversationId || "").trim(),
  (next, prev) => {
    if (!next || next === prev) return;
    void refreshQueue();
  },
);

const visibleQueueEvents = computed(() => {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    return props.queueEventsOverride;
  }
  const activeConversationId = String(props.activeConversationId || "").trim();
  if (!activeConversationId) return [];
  return queueEvents.value.filter(
    (event) => String(event.conversationId || "").trim() === activeConversationId,
  );
});

const queueUserPersonaName = computed(() =>
  String(props.personaNameMap["user-persona"] || props.userAlias || "").trim(),
);

/** 输入框占位文案：按忙碌/队列/引导状态切换，忙碌态嵌入人格名。 */
const effectiveChatInputPlaceholder = computed(() => {
  const personaName = String(props.personaName || "").trim();
  if (visibleQueueEvents.value.some((event) => event.queueMode === "guided")) {
    return t("chat.placeholderGuided", { personaName });
  }
  if (visibleQueueEvents.value.length > 0) {
    return t("chat.placeholderBusyQueued", { personaName });
  }
  if (props.busy || props.chatting) {
    return t("chat.placeholderBusyIdle", { personaName });
  }
  return t("chat.placeholder", { personaName });
});

const localChatInput = computed({
  get: () => props.chatInput,
  set: (value: string) => emit("update:chatInput", value),
});
const CHAT_INPUT_HISTORY_STORAGE_KEY = "easy_call.chat_input_history.v1";
const CHAT_INPUT_HISTORY_LIMIT = 100;
const SEND_MODE_STORAGE_KEY = "easy_call.send_mode.v1";
type SendMode = "enter" | "ctrl_enter";
const composerRootRef = ref<HTMLDivElement | null>(null);
const chatInputRef = ref<HTMLTextAreaElement | null>(null);
const chatInputComposing = ref(false);
const chatInputCompositionEndedAt = ref(0);

const sendMode = ref<SendMode>("enter");
const sendModeMenuOpen = ref(false);
const sendModeMenuRef = ref<HTMLDivElement | null>(null);

/** 手机窄屏（触摸）下的单行输入条：默认收起为单行，展开为完整输入模式。 */
function loadSendMode() {
  try {
    const raw = window.localStorage.getItem(SEND_MODE_STORAGE_KEY);
    if (raw === "ctrl_enter") sendMode.value = "ctrl_enter";
  } catch {
    // ignore storage failures
  }
}

function setSendMode(mode: SendMode) {
  sendMode.value = mode;
  sendModeMenuOpen.value = false;
  try {
    window.localStorage.setItem(SEND_MODE_STORAGE_KEY, mode);
  } catch {
    // ignore persistence failures
  }
}

const chatInputHistory = ref<string[]>([]);
const chatInputHistoryCursor = ref(-1);
const chatInputHistoryDraft = ref("");
const chatInputHistoryApplying = ref(false);
const resizeInputRaf = ref(0);
const instructionPanelOpen = ref(false);
const instructionFocusIndex = ref(0);
const modePanelOpen = ref(false);
const plusPanelOpen = ref(false);
const usagePanelOpen = ref(false);

/**
 * 上下文占用卡片的词元账单：直接复用维护对话框那套预览流程，
 * 数据由输入面板自己取，不再让标题栏持有。
 */
const usageMaintenance = useConversationMaintenanceDialog({
  t,
  currentConversationId: computed(() => String(props.activeConversationId || "").trim()),
  conversationSummaries: computed(() => [] as const),
  chatUsagePercent: computed(() => Number(props.chatUsagePercent || 0)),
  trimCompactNow: async () => {
    emit("trim-conversation");
  },
  trimNow: async () => {
    emit("trim-conversation");
  },
  deleteConversation: () => undefined,
  setStatus: () => undefined,
  setStatusError: () => undefined,
});

const usageBreakdown = computed(
  () => usageMaintenance.trimCompactionPreview.value?.tokenBreakdown,
);
const usageWindowTokens = computed(() =>
  Math.max(0, Number(usageBreakdown.value?.contextWindowTokens) || 0),
);

/** 压缩阈值：超过 90% 触发压缩，10% 为预留给压缩的保留区（与维护对话框一致） */
const COMPACTION_THRESHOLD_RATIO = 0.9;

type UsageRow = { key: string; label: string; tokens: number | undefined; color: string };

const usageRows = computed<UsageRow[]>(() => {
  const breakdown = usageBreakdown.value;
  const rows: UsageRow[] = [
    {
      key: "system",
      label: t("dialogs.trim.breakdownSystem"),
      tokens: breakdown?.systemTokens,
      color: "var(--color-primary)",
    },
    {
      key: "tools",
      label: t("dialogs.trim.breakdownTools"),
      tokens: breakdown?.toolsTokens,
      color: "var(--color-secondary)",
    },
    {
      key: "message",
      label: t("dialogs.trim.breakdownMessage"),
      tokens: breakdown?.messageTokens,
      color: "var(--color-accent)",
    },
  ];
  const windowTokens = usageWindowTokens.value;
  if (windowTokens <= 0) return rows;
  const usedTokens = rows.reduce((sum, row) => sum + Math.max(0, Number(row.tokens) || 0), 0);
  const reservedTokens = Math.round(windowTokens * (1 - COMPACTION_THRESHOLD_RATIO));
  rows.push({
    key: "reserved",
    label: t("dialogs.trim.breakdownReserved"),
    tokens: reservedTokens,
    color: "var(--color-warning)",
  });
  rows.push({
    key: "available",
    label: t("dialogs.trim.breakdownAvailable"),
    tokens: Math.max(0, windowTokens - usedTokens - reservedTokens),
    color: "var(--color-base-300)",
  });
  return rows;
});

function formatUsageTokens(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return "—";
  const rounded = Math.round(value);
  if (rounded >= 1000) {
    const k = rounded / 1000;
    return `~${Number.isInteger(k) ? String(k) : k.toFixed(1).replace(/\.0$/, "")}K`;
  }
  return `~${rounded.toLocaleString("en-US")}`;
}

function usagePercentOf(value: number | undefined): number {
  const windowTokens = usageWindowTokens.value;
  if (windowTokens <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((Math.max(0, Number(value) || 0) / windowTokens) * 100)));
}

watch(usagePanelOpen, async (open) => {
  if (open) await usageMaintenance.openTrimActionDialog();
});
/** 旧的 deck-top 操作行：默认隐藏，保留实现以便随时切回 */
const showLegacyActionRow = ref(false);
const goalSectionOpen = ref(false);
const goalObjectiveDraft = ref("");
const mentionPanelOpen = ref(false);
const mentionQuery = ref("");
const mentionFocusIndex = ref(0);
const mentionRange = ref<{ start: number; end: number } | null>(null);
const mentionPanelScrollRef = ref<HTMLDivElement | null>(null);
const issuePanelOpen = ref(false);
const issueQuery = ref("");
const issueFocusIndex = ref(0);
const issueRange = ref<{ start: number; end: number } | null>(null);
const issuePanelScrollRef = ref<HTMLDivElement | null>(null);

const normalizedInstructionPresets = computed(() =>
  (Array.isArray(props.instructionPresets) ? props.instructionPresets : [])
    .map((item) => ({
      id: String(item?.id || "").trim(),
      name: String(item?.prompt || item?.name || "").trim(),
      prompt: String(item?.prompt || item?.name || "").trim(),
    }))
    .filter((item) => !!item.id && !!item.prompt),
);
const localModelOptionId = ref("");

function modelOptionIdFromProps(): string {
  return String(props.preferredChatModelId || "").trim()
    || String(props.conversationCallPrimaryApiConfigId || "").trim();
}

watch(
  () => String(props.activeConversationId || "").trim(),
  () => {
    localModelOptionId.value = modelOptionIdFromProps();
  },
  { immediate: true },
);

watch(
  () => [
    String(props.preferredChatModelId || "").trim(),
    String(props.conversationCallPrimaryApiConfigId || "").trim(),
  ].join("|"),
  () => {
    localModelOptionId.value = modelOptionIdFromProps();
  },
);

// 传给 ChatModelPicker 的当前模型 id：本地选择为空时回退会话主模型
const activeModelDisplayId = computed(() =>
  localModelOptionId.value || String(props.conversationCallPrimaryApiConfigId || "").trim());
const attachedIdeContextReferenceIds = computed(() => new Set((props.attachedIdeContextReferences || []).map((item) => item.id)));
const mergedIdeContextGroups = computed<IdeContextWorkspaceGroup[]>(() => mergeComposerIdeContextGroups(
  props.ideContextGroups || [],
  props.attachedIdeContextReferences || [],
));
function normalizedComposerPathKey(value: string): string {
  return String(value || "").trim().replace(/\\/g, "/").toLowerCase();
}
const mergedIdeContextPathKeys = computed(() => new Set(
  mergedIdeContextGroups.value
    .flatMap((group) => group.references || [])
    .map((item) => normalizedComposerPathKey(item.filePath || item.relativePath || ""))
    .filter(Boolean),
));
const visibleQueuedAttachmentNotices = computed(() =>
  (Array.isArray(props.queuedAttachmentNotices) ? props.queuedAttachmentNotices : []).filter((item) => {
    const pathKey = normalizedComposerPathKey(item.path || "");
    return !pathKey || !mergedIdeContextPathKeys.value.has(pathKey);
  }),
);

function isIdeContextAttached(referenceId: string): boolean {
  return attachedIdeContextReferenceIds.value.has(referenceId);
}

function toggleIdeContextReference(item: IdeContextReferenceItem) {
  if (isIdeContextAttached(item.id)) {
    emit("removeIdeContextReference", item.id);
  } else {
    emit("attachIdeContextReference", item);
  }
  void nextTick(() => focusInput({ preventScroll: true }));
}

/** 输入卡附件适配：图片 / 文件 / IDE 桥胶囊。 */
const panelAttachmentImages = computed(() =>
  (Array.isArray(props.clipboardImages) ? props.clipboardImages : []).map((img, idx) => ({
    mime: String(img?.mime || ""),
    label: t("chat.image", { index: idx + 1 }),
    previewDataUrl: clipboardImagePreviewSrc(img),
  })),
);
const panelAttachmentFiles = computed(() =>
  visibleQueuedAttachmentNotices.value.map((file) => ({
    id: String(file.id || ""),
    fileName: String(file.fileName || ""),
    pending: !!file.pending,
  })),
);
const panelBridgeItems = computed(() =>
  mergedIdeContextGroups.value.flatMap((group) => group.references || []).map((item) => {
    const parts = ideContextReferenceDisplayParts(item);
    return {
      id: String(item.id || ""),
      fileName: String(parts.fileName || ""),
      lineSuffix: String(parts.lineSuffix || ""),
      title: ideContextReferenceTitle(item),
      attached: isIdeContextAttached(item.id),
    };
  }),
);

function removePanelAttachmentFile(index: number) {
  const target = panelAttachmentFiles.value[index];
  if (!target) return;
  const realIndex = (Array.isArray(props.queuedAttachmentNotices) ? props.queuedAttachmentNotices : [])
    .findIndex((item) => String(item.id || "") === String(target.id || ""));
  if (realIndex < 0) return;
  emit("removeQueuedAttachmentNotice", realIndex);
  void nextTick(() => focusInput({ preventScroll: true }));
}

function togglePanelBridge(id: string) {
  const targetId = String(id || "").trim();
  if (!targetId) return;
  const target = mergedIdeContextGroups.value
    .flatMap((group) => group.references || [])
    .find((item) => String(item.id || "") === targetId);
  if (!target) return;
  toggleIdeContextReference(target);
}

function ideContextReferenceTitle(item: IdeContextReferenceItem): string {
  const relativePath = String(item.relativePath || "").trim();
  const startLine = Number(item.startLine || 0);
  const endLine = Number(item.endLine || 0);
  if (!relativePath) return String(item.displayLabel || "").trim();
  if (startLine > 0 && endLine > startLine) {
    return `${relativePath}:${startLine}-${endLine}`;
  }
  if (startLine > 0) {
    return `${relativePath}:${startLine}`;
  }
  return relativePath;
}

// 停止入口的存在与否只取决于「这一轮是否还在进行」。
// 不再要求输入框为空：用户正在打字时同样有权随时打断。
const showStopAction = computed(() =>
  props.chatting
  || !!props.submitPending
  || ["queued", "waiting", "streaming"].includes(String(props.frontendRoundPhase || "idle")),
);
const selectedMentions = computed(() =>
  (Array.isArray(props.selectedMentions) ? props.selectedMentions : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.agentName),
);
const filteredMentionOptions = computed<MentionOptionView[]>(() => {
  const query = mentionQuery.value.trim().toLowerCase();
  return (Array.isArray(props.mentionEntries) ? props.mentionEntries : [])
    .map((item) => ({
      agentId: String(item?.agentId || "").trim(),
      agentName: String(item?.agentName || "").trim(),
      avatarUrl: String(item?.avatarUrl || "").trim() || undefined,
      mentionable: !!item?.mentionable,
      hidden: !!item?.hidden,
      unavailableReason: String(item?.unavailableReason || "").trim() || undefined,
    }))
    .filter((item) => !!item.agentId && !!item.agentName && !item.hidden)
    .filter((item) => {
      if (!query) return true;
      if (item.agentName.toLowerCase().includes(query)) return true;
      return false;
    });
});

const planModeToggleAllowed = computed(() => !props.frozen);

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

function toggleInstructionPanel() {
  if (instructionPanelOpen.value) {
    closeInstructionPanel();
    return;
  }
  openInstructionPanel();
}

function buildInstructionPresetInput(currentText: string, prompt: string): string {
  const current = String(currentText || "");
  const nextPrompt = String(prompt || "").trim();
  if (!nextPrompt) return current;
  if (!current) return nextPrompt;
  return `${current}\n\n${nextPrompt}`;
}

function applyInstructionPreset(item: PromptCommandPreset | undefined) {
  if (!item) return;
  const prompt = String(item.prompt || item.name || "").trim();
  if (!prompt) return;
  const nextValue = buildInstructionPresetInput(localChatInput.value, prompt);
  localChatInput.value = nextValue;
  closeInstructionPanel();
  closeMentionPanel();
  closeIssuePanel();
  nextTick(() => {
    scheduleResizeChatInput();
    const el = chatInputRef.value;
    if (!el) return;
    el.focus({ preventScroll: true });
    const cursor = nextValue.length;
    el.setSelectionRange(cursor, cursor);
  });
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

function removeSelectedMention(item: ChatMentionTarget | undefined) {
  if (!item) return;
  emit("removeMention", {
    agentId: String(item.agentId || "").trim(),
  });
  closeMentionPanel();
}

function applyMention(item: MentionOptionView | undefined) {
  if (!item || !item.mentionable || !mentionRange.value) return;
  const current = String(localChatInput.value || "");
  const before = current.slice(0, mentionRange.value.start);
  const after = current.slice(mentionRange.value.end);
  const nextValue = `${before}${after}`;
  localChatInput.value = nextValue;
  if (selectedMentions.value.some((entry) =>
    String(entry.agentId || "").trim() === String(item.agentId || "").trim()
  )) {
    emit("removeMention", {
      agentId: String(item.agentId || "").trim(),
    });
  } else {
    emit("addMention", {
      agentId: String(item.agentId || "").trim(),
      agentName: String(item.agentName || "").trim(),
      avatarUrl: String(item.avatarUrl || "").trim() || undefined,
    });
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
  const target = list[nextIndex];
  if (!target.mentionable) return;
  mentionFocusIndex.value = nextIndex;
  applyMention(target);
}

function moveMentionFocus(delta: number) {
  const list = filteredMentionOptions.value;
  if (list.length === 0) return;
  let next = mentionFocusIndex.value + delta;
  while (next >= 0 && next < list.length && !list[next].mentionable) {
    next += delta;
  }
  if (next < 0 || next >= list.length) return;
  mentionFocusIndex.value = next;
  scrollMentionFocusIntoView();
}

function scrollMentionFocusIntoView() {
  nextTick(() => {
    const container = mentionPanelScrollRef.value;
    if (!container) return;
    const active = container.querySelector<HTMLElement>(`[data-mention-option-index="${mentionFocusIndex.value}"]`);
    active?.scrollIntoView({ block: "nearest" });
  });
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
  const match = beforeCursor.match(/(?:^|\s)@([^\s@]*)$/);
  if (!match) {
    closeMentionPanel();
    return;
  }
  const query = match[1];
  mentionQuery.value = query;
  const atStart = cursor - 1 - query.length;
  mentionRange.value = { start: atStart, end: cursor };
  mentionPanelOpen.value = true;
  const firstMentionable = filteredMentionOptions.value.findIndex((item) => item.mentionable);
  mentionFocusIndex.value = firstMentionable >= 0 ? firstMentionable : 0;
}

const filteredIssueOptions = computed(() =>
  sortKanbanIssueOptions(filterKanbanIssueOptions(props.kanbanIssues || [], issueQuery.value)),
);
/** 渲染用分组：组序按用户优先级，组内最新在上；每项带扁平下标，键盘导航仍走 filteredIssueOptions。 */
const issueOptionGroups = computed(() => groupKanbanIssueOptions(filteredIssueOptions.value));
const attachedIssueReferences = computed<KanbanIssueReference[]>(() =>
  Array.isArray(props.attachedIssueReferences) ? props.attachedIssueReferences : [],
);

function closeIssuePanel() {
  issuePanelOpen.value = false;
  issueQuery.value = "";
  issueFocusIndex.value = 0;
  issueRange.value = null;
}

function removeIssueReference(number: number) {
  emit("removeKanbanIssueReference", Number(number || 0));
  closeIssuePanel();
}

/** `#` 候选选中：吃掉输入框里的 `#词元`，再把议题交给上层那份引用列表。 */
function applyIssueOption(item: KanbanIssue | undefined) {
  if (!item) return;
  const range = issueRange.value;
  const current = String(localChatInput.value || "");
  const before = range ? current.slice(0, range.start) : current;
  const after = range ? current.slice(range.end) : "";
  if (range) localChatInput.value = `${before}${after}`;
  emit("attachKanbanIssueReference", item);
  closeIssuePanel();
  nextTick(() => {
    const el = chatInputRef.value;
    if (!el) return;
    const cursor = Math.min(before.length, `${before}${after}`.length);
    el.focus({ preventScroll: true });
    el.setSelectionRange(cursor, cursor);
    scheduleResizeChatInput();
  });
}

function selectIssueOptionByIndex(index: number) {
  const list = filteredIssueOptions.value;
  if (list.length === 0) return;
  const nextIndex = Math.max(0, Math.min(list.length - 1, index));
  issueFocusIndex.value = nextIndex;
  applyIssueOption(list[nextIndex]);
}

function moveIssueFocus(delta: number) {
  const list = filteredIssueOptions.value;
  if (list.length === 0) return;
  const next = issueFocusIndex.value + delta;
  issueFocusIndex.value = Math.max(0, Math.min(list.length - 1, next));
  nextTick(() => {
    const container = issuePanelScrollRef.value;
    if (!container) return;
    container
      .querySelector<HTMLElement>(`[data-issue-option-index="${issueFocusIndex.value}"]`)
      ?.scrollIntoView({ block: "nearest" });
  });
}

/** `#` 与 `@` 各按自己的词元判定，谁不匹配谁关面板，两个面板不会同时开着。 */
function updateIssueState() {
  const el = chatInputRef.value;
  const options = props.kanbanIssues || [];
  if (!el || el.selectionStart !== el.selectionEnd || options.length === 0) {
    closeIssuePanel();
    return;
  }
  const value = String(localChatInput.value || "");
  const cursor = el.selectionStart ?? value.length;
  const trigger = findKanbanIssueTrigger(value, cursor);
  if (!trigger) {
    closeIssuePanel();
    return;
  }
  issueRange.value = { start: trigger.start, end: trigger.end };
  issueQuery.value = trigger.query;
  issueFocusIndex.value = 0;
  // 议题列表为空、或过滤后一条不剩时都不弹空面板。
  issuePanelOpen.value = filteredIssueOptions.value.length > 0;
  if (!issuePanelOpen.value) issueRange.value = null;
}

function selectConversationPreferredModel(id: string) {
  const nextId = String(id || "").trim();
  if (!nextId || nextId === activeModelDisplayId.value) return;
  localModelOptionId.value = nextId;
  emit("update:conversationPreferredApiConfigId", nextId);
}

function togglePlanMode() {
  if (!planModeToggleAllowed.value) return;
  const next = !props.planModeEnabled;
  emit("update:planModeEnabled", next);
  // 三种模式互斥：开一个就关掉另外两个
  if (next) {
    if (props.wheelchairModeEnabled) emit("update:wheelchairModeEnabled", false);
    if (props.goalActive) emit("stopGoalTask");
  }
  closeInstructionPanel();
}

/** 模式按钮底色跟随当前生效的模式，没开模式时是普通幽灵按钮。 */
/** 上下文占用环形：半径 14 与标题栏原图一致，搬进面板后不改变观感 */
const RING_RADIUS = 14;
const ringCircumference = 2 * Math.PI * RING_RADIUS;

const normalizedChatUsagePercent = computed(() =>
  Math.min(100, Math.max(0, Math.round(Number(props.chatUsagePercent || 0)))),
);

const ringDashoffset = computed(
  () => ringCircumference * (1 - normalizedChatUsagePercent.value / 100),
);

const modeButtonClass = computed(() => {
  if (props.goalActive) return "btn-primary";
  if (props.planModeEnabled) return "btn-ghost bg-info/15 text-info";
  if (props.wheelchairModeEnabled) return "btn-ghost bg-warning/15 text-warning";
  return "btn-ghost";
});

/** 模式按钮图标跟着生效的模式走：空模式是 ai-swap，开了哪个模式就用哪个图标。 */
const modeButtonIcon = computed<AppIconName>(() => {
  if (props.planModeEnabled) return "quill-write-02";
  if (props.wheelchairModeEnabled) return "wheelchair";
  if (props.goalActive) return "target";
  return "ai-art";
});

function toggleGoalSection() {
  goalSectionOpen.value = !goalSectionOpen.value;
}

function submitGoalObjective() {
  const objective = goalObjectiveDraft.value.trim();
  if (!objective || props.frozen || props.goalDisabled) return;
  emit("startGoalTask", objective);
  goalObjectiveDraft.value = "";
  goalSectionOpen.value = false;
}

function toggleWheelchairMode() {
  const next = !props.wheelchairModeEnabled;
  emit("update:wheelchairModeEnabled", next);
  // 三种模式互斥：开一个就关掉另外两个
  if (next) {
    if (props.planModeEnabled) emit("update:planModeEnabled", false);
    if (props.goalActive) emit("stopGoalTask");
  }
  closeInstructionPanel();
}

function fillWheelchairSuggestion(text: string) {
  const current = props.chatInput.trim();
  const next = current ? `${current}\n${text}` : text;
  emit("update:chatInput", next);
  emit("dismissWheelchairSuggestion");
}

function resizeChatInput() {
  const el = chatInputRef.value;
  if (!el) return;
  // 单布局：空态单行高度，有字随内容增高，上限 160。
  const minHeight = 24;
  const maxHeight = 160;
  el.style.height = "auto";
  const nextHeight = Math.max(Math.min(el.scrollHeight, maxHeight), minHeight);
  el.style.height = `${nextHeight}px`;
  el.style.overflowY = "auto";
}

function handleChatInputInput() {
  scheduleResizeChatInput();
  updateMentionState();
  updateIssueState();
}

/** 从加号菜单往输入框光标处插入触发字符（@ / #），复用已有输入处理流程 */
function insertChatInputTrigger(char: string) {
  const el = chatInputRef.value;
  const value = localChatInput.value;
  const start = el?.selectionStart ?? value.length;
  const end = el?.selectionEnd ?? start;
  localChatInput.value = value.slice(0, start) + char + value.slice(end);
  nextTick(() => {
    const target = chatInputRef.value;
    if (target) {
      const pos = start + char.length;
      target.focus();
      target.setSelectionRange(pos, pos);
    }
    handleChatInputInput();
  });
}

function handleChatInputCompositionStart() {
  chatInputComposing.value = true;
}

function handleChatInputCompositionEnd() {
  chatInputComposing.value = false;
  chatInputCompositionEndedAt.value = performance.now();
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
  closeInstructionPanel();
  closeMentionPanel();
  closeIssuePanel();
  // 手机端回车/点发送后收起软键盘：桌面端保持聚焦，方便连着打字。
  if (isMobileTouchViewport()) {
    chatInputRef.value?.blur();
  }
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

function handleChatInputFocus() {
  if (props.composerScope) {
    registerChatComposerFocus(props.composerScope);
  }
}

function handleChatInputBlur() {
  if (props.composerScope) {
    clearChatComposerFocus(props.composerScope);
  }
}

function handleChatInputKeydown(event: KeyboardEvent) {
  if (
    chatInputEnterConfirmsComposition(
      event,
      chatInputComposing.value,
      chatInputCompositionEndedAt.value,
      performance.now(),
    )
  ) {
    return;
  }
  if (event.key === "Escape") {
    const hadMention = mentionPanelOpen.value;
    const hadIssue = issuePanelOpen.value;
    const hadInstruction = instructionPanelOpen.value;
    const hadSendMode = sendModeMenuOpen.value;
    if (hadMention || hadIssue || hadInstruction || hadSendMode) {
      event.preventDefault();
      if (hadMention) closeMentionPanel();
      if (hadIssue) closeIssuePanel();
      if (hadInstruction) closeInstructionPanel();
      else instructionPanelOpen.value = false;
      if (hadSendMode) sendModeMenuOpen.value = false;
      return;
    }
  }
  if (issuePanelOpen.value) {
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveIssueFocus(-1);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveIssueFocus(1);
      return;
    }
    if (event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      selectIssueOptionByIndex(issueFocusIndex.value);
      return;
    }
  }
  if (mentionPanelOpen.value) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMentionPanel();
      instructionPanelOpen.value = false;
      sendModeMenuOpen.value = false;
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
  if (event.key === "Escape" && showStopAction.value) {
    event.preventDefault();
    emit("stopChat");
    return;
  }
  const ctrlEnterPressed = event.key === "Enter" && event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  const plainEnterPressed = event.key === "Enter" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  // Alt+S 兜底发送：无论当前发送模式，始终可用（输入法组合中除外）
  const altSPressed = event.key.toLowerCase() === "s" && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.isComposing;
  if (altSPressed) {
    if (props.frozen) return;
    event.preventDefault();
    handleSendChat();
    return;
  }
  if (sendMode.value === "ctrl_enter") {
    if (ctrlEnterPressed) {
      if (props.frozen) return;
      event.preventDefault();
      handleSendChat();
      return;
    }
    // Ctrl+Enter 模式：普通 Enter 保留为换行
    if (plainEnterPressed) return;
  } else if (plainEnterPressed) {
    if (props.frozen) return;
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

function clipboardImagePreviewSrc(image: BinaryAttachment): string {
  const previewDataUrl = String(image?.previewDataUrl || "").trim();
  if (previewDataUrl.startsWith("data:image/")) return previewDataUrl;
  const mime = String(image?.mime || "").trim().toLowerCase();
  const bytesBase64 = String(image?.bytesBase64 || "").trim();
  if (!mime.startsWith("image/") || !bytesBase64) return "";
  return `data:${mime};base64,${bytesBase64}`;
}

function removeClipboardImageAt(index: number) {
  emit("removeClipboardImage", index);
  void nextTick(() => focusInput({ preventScroll: true }));
}

function avatarInitial(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function mentionDisplayLabel(target: Pick<ChatMentionTarget, "agentName">): string {
  return String(target?.agentName || "").trim();
}

function isMentionSelected(target: Pick<ChatMentionTarget, "agentId"> | undefined): boolean {
  const agentId = String(target?.agentId || "").trim();
  if (!agentId) return false;
  return selectedMentions.value.some((item) =>
    String(item.agentId || "").trim() === agentId
  );
}

async function handleRecallToInput(event: {
  source?: string;
  messagePreview?: string;
  messageText?: string;
  id?: string;
  queueMode?: "normal" | "guided";
}) {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    emit("queueRecall", event as ChatQueueEvent);
    nextTick(() => focusInput({ preventScroll: true }));
    return;
  }
  if (event.source === "user" && event.queueMode !== "guided") {
    if (event.id) {
      const result = await recallQueueEvent(event.id);
      if (result.removed) {
        localChatInput.value = result.messageText || event.messageText || event.messagePreview || "";
        nextTick(() => focusInput({ preventScroll: true }));
      }
    }
  }
}

function handleQueueMarkGuided(eventId: string) {
  if (props.queueEventsOverride !== undefined && props.queueEventsOverride !== null) {
    emit("queueMarkGuided", eventId);
    nextTick(() => focusInput({ preventScroll: true }));
    return;
  }
  markGuided(eventId);
  nextTick(() => focusInput({ preventScroll: true }));
}

function focusInput(options?: FocusOptions) {
  chatInputRef.value?.focus(options);
}

defineExpose({
  focusInput,
});

onMounted(() => {
  loadChatInputHistory();
  loadSendMode();
  window.addEventListener("keydown", handleWindowKeydown);
  nextTick(() => {
    resizeChatInput();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
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
      updateMentionState();
    });
  },
);

watch(
  () => props.activeConversationId,
  () => {
    closeInstructionPanel();
    closeMentionPanel();
    nextTick(() => scheduleResizeChatInput());
  },
);

watch(
  () => normalizedInstructionPresets.value,
  (list) => {
    if (list.length === 0) {
      instructionFocusIndex.value = 0;
      instructionPanelOpen.value = false;
      return;
    }
    if (instructionFocusIndex.value >= list.length) {
      instructionFocusIndex.value = list.length - 1;
    }
  },
  { deep: true },
);

watch(
  () => props.selectedMentions.map((item) => item.agentId).join("|"),
  () => {
    closeMentionPanel();
  },
);
</script>

<style scoped>
.chat-input-no-focus::-webkit-scrollbar {
  display: none;
}
.chat-input-no-focus {
  scrollbar-width: none;
}
</style>
