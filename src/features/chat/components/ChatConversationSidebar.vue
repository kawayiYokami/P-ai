<template>
  <aside class="conversation-time-container flex h-full w-full shrink-0 flex-col border-r border-base-300 bg-base-200">
    <div class="flex items-center p-2 pb-0">
      <SegmentedControl
        v-model="activeConversationTab"
        :options="conversationTabOptions"
        size="sm"
        full-width
        surface-class="bg-base-300"
      />
    </div>
    <ChatConversationFloatingScroll ref="conversationFloatingScrollRef" class="flex-1 min-h-0 p-1">
      <Transition :name="conversationTabTransitionName" mode="out-in" @after-enter="handleConversationTabTransitionSettled">
        <div :key="activeConversationTab" class="conversation-tab-panel">
          <ChatTaskSidebarPanel
            v-if="activeConversationTab === 'task'"
            :conversation-items="items"
            :search-query="conversationSearchQuery"
            @edit-task="requestTaskEdit"
            @layout-change="scheduleConversationListScrollbarUpdate"
          />
          <template v-else-if="activeConversationTab === 'contact'">
            <template v-for="section in displayedContactSections" :key="section.key">
              <CollapsibleGroup
                :ref="(el) => setConversationSectionElement(section.key, el)"
                :title="section.title"
                :model-value="isConversationSectionCollapsed(section.key)"
                :icon="conversationSectionIcon(section)"
                :avatar-url="conversationSectionAvatarUrl(section)"
                :draggable="isConversationSectionDraggable(section)"
                :drop-indicator="conversationSectionDragIndicator(section)"
                @update:model-value="toggleConversationSection(section.key)"
                @collapse-all="collapseAllConversationSections"
                @after-enter="scheduleConversationListScrollbarUpdate"
                @after-leave="scheduleConversationListScrollbarUpdate"
                @dragstart="handleConversationSectionDragStart(section, $event)"
                @dragover="handleConversationSectionDragOver(section, $event)"
                @drop="handleConversationSectionDrop(section, $event)"
                @dragend="handleConversationSectionDragEnd"
              >
                <template v-for="item in section.visibleItems" :key="item.conversationId">
                  <ChatConversationItem
                    :item="item"
                    :level="isSimpleConversationRows ? simpleConversationItemLevel(item) : 'full'"
                    :active-conversation-id="props.activeConversationId"
                    :user-alias="props.userAlias"
                    :user-avatar-url="props.userAvatarUrl"
                    :persona-name-map="props.personaNameMap"
                    :persona-avatar-url-map="props.personaAvatarUrlMap"
                    :pipeline-status-by-id="conversationStatusById"
                    :show-source-badge="false"
                    :compact-indicator="isSimpleConversationRows"
                    @select="(payload) => emit('select', payload)"
                    @rename="(payload) => emit('rename', payload)"
                    @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                    @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                    @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                    @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                  />
                </template>
                <div
                  v-if="section.hiddenItemCount > 0 || conversationSectionHasExtraItems(section.key)"
                  class="mx-1 flex min-w-0 items-center gap-2 pb-1.5 pt-0.5"
                >
                  <button
                    v-if="section.hiddenItemCount > 0"
                    type="button"
                    class="group flex h-7.5 items-center gap-2 rounded-lg px-2.5 text-left text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                    :title="t('chat.loadMore')"
                    @click.stop="loadMoreConversationsInSection(section.key)"
                  >
                    <span class="shrink-0" :style="conversationSectionLeadStyle"></span>
                    <span>{{ t("chat.loadMore") }}（{{ section.hiddenItemCount }}）</span>
                  </button>
                  <button
                    v-if="conversationSectionHasExtraItems(section.key)"
                    type="button"
                    class="flex flex-1 h-7.5 items-center rounded-lg px-2 text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                    :title="t('chat.collapseSection')"
                    @click.stop="collapseConversationSection(section.key)"
                  >
                    {{ t("chat.collapseSection") }}
                  </button>
                </div>
              </CollapsibleGroup>
            </template>
            <div
              v-if="displayedContactSections.length === 0"
              class="px-3 py-4 text-center text-sm text-base-content/60"
            >
              {{ t("chat.conversationSearchEmpty") }}
            </div>
          </template>
          <template v-else>
            <button
              type="button"
              class="mx-1 mb-1 flex min-h-9 w-[calc(100%-0.5rem)] items-center gap-2 rounded-lg px-2.5 py-1 text-left text-sm transition-colors"
              :class="activeConversationIsDraft
                ? 'bg-base-300 text-base-content'
                : 'text-base-content hover:bg-base-300/70 active:bg-base-300'"
              :title="t('chat.newConversation')"
              @click="createConversationFromSidebar"
            >
              <SquarePen class="h-4 w-4" />
              <span>{{ t("chat.newConversation") }}</span>
            </button>

            <!-- 1. 最近大区 -->
            <div v-if="displayedRecentSections.length > 0" class="super-section mb-1">
              <div class="mx-1 mt-1 mb-0.5 flex select-none items-center justify-between rounded px-1.5 py-0.5 text-sm text-base-content/60">
                <!-- 左边：折叠/展开按钮（只控制内容区开合） -->
                <button
                  type="button"
                  class="group flex items-center gap-1 min-w-0 rounded px-1 py-0.5 transition-colors hover:text-base-content/90"
                  :title="t('chat.recentConversations')"
                  @click="toggleRecentSuperSection"
                >
                  <span class="truncate">{{ t("chat.recentConversations") }}</span>
                  <ChevronRight
                    class="h-3 w-3 shrink-0 ml-0.5 transition-transform duration-200 ease-out opacity-0 group-hover:opacity-60"
                    :class="recentSuperCollapsed ? '' : 'rotate-90'"
                  />
                </button>

                <!-- 右边：时间窗口切换按钮（只负责打开选项） -->
                <div class="shrink-0">
                  <EcallDropdown
                    v-model="recentTimeFilterOpen"
                    teleport
                    :match-trigger-width="false"
                    panel-class="w-36 p-1"
                    placement="bottom"
                  >
                    <template #trigger="{ toggle: toggleTimeFilter }">
                      <button
                        type="button"
                        class="flex h-5 min-h-5 items-center rounded px-1 text-sm text-base-content/60 transition-colors hover:bg-base-300/60 hover:text-base-content"
                        :title="currentRecentTimeLabel"
                        @click.stop="toggleTimeFilter"
                      >
                        <Clock class="h-3 w-3" />
                      </button>
                    </template>
                    <template #default="{ close: closeTimeFilter }">
                      <ul class="menu w-full p-0">
                        <li v-for="opt in recentTimeFilterOptions" :key="opt.value">
                          <button type="button" class="flex-nowrap" @click="selectRecentTimeFilter(opt.value, closeTimeFilter)">
                            <Clock class="h-3.5 w-3.5 shrink-0" />
                            <span class="whitespace-nowrap">{{ opt.label }}</span>
                            <Check v-if="recentTimeFilterHours === opt.value" class="ml-auto h-3.5 w-3.5 shrink-0 text-primary" />
                          </button>
                        </li>
                      </ul>
                    </template>
                  </EcallDropdown>
                </div>
              </div>

              <div class="super-section-shell" :class="{ 'is-collapsed': recentSuperCollapsed }">
                <div class="super-section-inner">
                  <template v-for="section in displayedRecentSections" :key="section.key">
                    <CollapsibleGroup
                      :ref="(el) => setConversationSectionElement(section.key, el)"
                      :title="section.title"
                      :model-value="isConversationSectionCollapsed(section.key)"
                      :icon="conversationSectionIcon(section)"
                      :avatar-url="conversationSectionAvatarUrl(section)"
                      :draggable="false"
                      @update:model-value="toggleConversationSection(section.key)"
                      @collapse-all="collapseAllConversationSections"
                      @after-enter="scheduleConversationListScrollbarUpdate"
                      @after-leave="scheduleConversationListScrollbarUpdate"
                    >
                      <template #actions>
                        <button
                          v-if="section.workspaceRootPath"
                          type="button"
                          class="btn btn-ghost btn-xs ml-auto h-6 min-h-6 w-6 min-w-6 shrink-0 p-0 text-base-content opacity-0 transition-opacity group-hover/section:opacity-100"
                          :title="t('chat.newConversation')"
                          @click.stop="createConversationInSection(section)"
                          @dblclick.stop
                        >
                          <SquarePen class="h-3.5 w-3.5" />
                        </button>
                      </template>
                      <template v-for="item in section.visibleItems" :key="item.conversationId">
                        <ChatConversationItem
                          :item="item"
                          :level="isSimpleConversationRows ? simpleConversationItemLevel(item) : 'full'"
                          :active-conversation-id="props.activeConversationId"
                          :user-alias="props.userAlias"
                          :user-avatar-url="props.userAvatarUrl"
                          :persona-name-map="props.personaNameMap"
                          :persona-avatar-url-map="props.personaAvatarUrlMap"
                          :pipeline-status-by-id="conversationStatusById"
                          :show-source-badge="false"
                          :show-jump-to-section="true"
                          :compact-indicator="isSimpleConversationRows"
                          @select="(payload) => emit('select', payload)"
                          @rename="(payload) => emit('rename', payload)"
                          @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                          @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                          @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                          @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                          @reveal-section="revealConversationSection(item)"
                        />
                        <template v-if="(section.simpleFollowers[String(item.conversationId || '').trim()] || []).length > 0">
                          <ChatConversationItem
                            v-for="simpleItem in (section.simpleFollowers[String(item.conversationId || '').trim()] || [])"
                            :key="`simple-${simpleItem.conversationId}`"
                            :item="simpleItem"
                            :level="simpleItemLevel(simpleItem)"
                            :active-conversation-id="props.activeConversationId"
                            :user-alias="props.userAlias"
                            :user-avatar-url="props.userAvatarUrl"
                            :persona-name-map="props.personaNameMap"
                            :persona-avatar-url-map="props.personaAvatarUrlMap"
                            :pipeline-status-by-id="conversationStatusById"
                            :show-jump-to-section="true"
                            @select="(payload) => emit('select', payload)"
                            @rename="(payload) => emit('rename', payload)"
                            @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                            @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                            @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                            @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                            @reveal-section="revealConversationSection(simpleItem)"
                          />
                        </template>
                      </template>
                    </CollapsibleGroup>
                  </template>
                  <div
                    v-if="recentHiddenCount > 0 || recentExtraCount > 0"
                    class="mx-1 flex min-w-0 items-center gap-2 pb-1.5 pt-0.5"
                  >
                    <button
                      v-if="recentHiddenCount > 0"
                      type="button"
                      class="group flex h-7.5 items-center gap-2 rounded-lg px-2.5 text-left text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                      :title="t('chat.loadMore')"
                      @click.stop="loadMoreRecentConversations"
                    >
                      <span class="shrink-0" :style="conversationSectionLeadStyle"></span>
                      <span>{{ t("chat.loadMore") }}（{{ recentHiddenCount }}）</span>
                    </button>
                    <button
                      v-if="recentExtraCount > 0"
                      type="button"
                      class="flex flex-1 h-7.5 items-center rounded-lg px-2 text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                      :title="t('chat.collapseSection')"
                      @click.stop="collapseRecentConversations"
                    >
                      {{ t("chat.collapseSection") }}
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- 2. 分类名大区 -->
            <div v-if="displayedCategorySections.length > 0" class="super-section">
              <div class="mx-1 mt-2 mb-0.5 flex select-none items-center justify-between rounded px-1.5 py-0.5 text-sm text-base-content/60">
                <!-- 左边：折叠/展开按钮（只控制内容区开合） -->
                <button
                  type="button"
                  class="group flex items-center gap-1 min-w-0 rounded px-1 py-0.5 transition-colors hover:text-base-content/90"
                  :title="conversationGroupingLabel"
                  @click="toggleCategorySuperSection"
                >
                  <span class="truncate">{{ conversationGroupingLabel }}</span>
                  <ChevronRight
                    class="h-3 w-3 shrink-0 ml-0.5 transition-transform duration-200 ease-out opacity-0 group-hover:opacity-60"
                    :class="categorySuperCollapsed ? '' : 'rotate-90'"
                  />
                </button>

                <!-- 右边：分组切换按钮（只负责打开选项） -->
                <div class="shrink-0">
                  <EcallDropdown
                    v-model="groupingMenuOpen"
                    teleport
                    :match-trigger-width="false"
                    panel-class="w-40 p-1"
                    placement="bottom"
                  >
                    <template #trigger="{ toggle: toggleGroupingMenu }">
                      <button
                        type="button"
                        class="flex h-5 min-h-5 items-center rounded px-1 text-sm text-base-content/60 transition-colors hover:bg-base-300/60 hover:text-base-content"
                        :title="conversationGroupingLabel"
                        @click.stop="toggleGroupingMenu"
                      >
                        <component :is="currentGroupingIcon" class="h-3 w-3" />
                      </button>
                    </template>
                    <template #default="{ close: closeGroupingMenu }">
                      <ul class="menu w-full p-0">
                        <li v-for="option in conversationGroupingOptions" :key="option.value">
                          <button type="button" @click="selectConversationGrouping(option.value, closeGroupingMenu)">
                            <component :is="option.icon" class="h-3.5 w-3.5" />
                            <span>{{ option.label }}</span>
                            <Check v-if="conversationGrouping === option.value" class="ml-auto h-3.5 w-3.5 text-primary" />
                          </button>
                        </li>
                      </ul>
                    </template>
                  </EcallDropdown>
                </div>
              </div>

              <div class="super-section-shell" :class="{ 'is-collapsed': categorySuperCollapsed }">
                <div class="super-section-inner">
                  <template v-for="section in displayedCategorySections" :key="section.key">
                    <CollapsibleGroup
                      :ref="(el) => setConversationSectionElement(section.key, el)"
                      :title="section.title"
                      :model-value="isConversationSectionCollapsed(section.key)"
                      :icon="conversationSectionIcon(section)"
                      :avatar-url="conversationSectionAvatarUrl(section)"
                      :draggable="isConversationSectionDraggable(section)"
                      :drop-indicator="conversationSectionDragIndicator(section)"
                      @update:model-value="toggleConversationSection(section.key)"
                      @collapse-all="collapseAllConversationSections"
                      @after-enter="scheduleConversationListScrollbarUpdate"
                      @after-leave="scheduleConversationListScrollbarUpdate"
                      @dragstart="handleConversationSectionDragStart(section, $event)"
                      @dragover="handleConversationSectionDragOver(section, $event)"
                      @drop="handleConversationSectionDrop(section, $event)"
                      @dragend="handleConversationSectionDragEnd"
                    >
                      <template #actions>
                        <button
                          v-if="section.workspaceRootPath"
                          type="button"
                          class="btn btn-ghost btn-xs ml-auto h-6 min-h-6 w-6 min-w-6 shrink-0 p-0 text-base-content opacity-0 transition-opacity group-hover/section:opacity-100"
                          :title="t('chat.newConversation')"
                          @click.stop="createConversationInSection(section)"
                          @dblclick.stop
                        >
                          <SquarePen class="h-3.5 w-3.5" />
                        </button>
                      </template>
                      <template v-for="item in section.visibleItems" :key="item.conversationId">
                        <ChatConversationItem
                          :item="item"
                          :level="isSimpleConversationRows ? simpleConversationItemLevel(item) : 'full'"
                          :active-conversation-id="props.activeConversationId"
                          :user-alias="props.userAlias"
                          :user-avatar-url="props.userAvatarUrl"
                          :persona-name-map="props.personaNameMap"
                          :persona-avatar-url-map="props.personaAvatarUrlMap"
                          :pipeline-status-by-id="conversationStatusById"
                          :show-source-badge="false"
                          :compact-indicator="isSimpleConversationRows"
                          @select="(payload) => emit('select', payload)"
                          @rename="(payload) => emit('rename', payload)"
                          @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                          @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                          @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                          @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                          @reveal-section="revealConversationSection(item)"
                        />
                        <template v-if="(section.simpleFollowers[String(item.conversationId || '').trim()] || []).length > 0">
                          <ChatConversationItem
                            v-for="simpleItem in (section.simpleFollowers[String(item.conversationId || '').trim()] || [])"
                            :key="`simple-${simpleItem.conversationId}`"
                            :item="simpleItem"
                            :level="simpleItemLevel(simpleItem)"
                            :active-conversation-id="props.activeConversationId"
                            :user-alias="props.userAlias"
                            :user-avatar-url="props.userAvatarUrl"
                            :persona-name-map="props.personaNameMap"
                            :persona-avatar-url-map="props.personaAvatarUrlMap"
                            :pipeline-status-by-id="conversationStatusById"
                            @select="(payload) => emit('select', payload)"
                            @rename="(payload) => emit('rename', payload)"
                            @toggle-pin-conversation="(conversationId) => emit('togglePinConversation', conversationId)"
                            @archive-conversation="(conversationId) => emit('archiveConversation', conversationId)"
                            @export-conversation="(conversationId) => emit('exportConversation', conversationId)"
                            @delete-conversation="(conversationId) => emit('deleteConversation', conversationId)"
                          />
                        </template>
                      </template>
                      <div
                        v-if="section.hiddenItemCount > 0 || conversationSectionHasExtraItems(section.key)"
                        class="mx-1 flex min-w-0 items-center gap-2 pb-1.5 pt-0.5"
                      >
                        <button
                          v-if="section.hiddenItemCount > 0"
                          type="button"
                          class="group flex h-7.5 items-center gap-2 rounded-lg px-2.5 text-left text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                          :title="t('chat.loadMore')"
                          @click.stop="loadMoreConversationsInSection(section.key)"
                        >
                          <span class="shrink-0" :style="conversationSectionLeadStyle"></span>
                          <span>{{ t("chat.loadMore") }}（{{ section.hiddenItemCount }}）</span>
                        </button>
                        <button
                          v-if="conversationSectionHasExtraItems(section.key)"
                          type="button"
                          class="flex flex-1 h-7.5 items-center rounded-lg px-2 text-xs text-base-content/50 transition-colors hover:bg-base-300/50 hover:text-base-content active:bg-base-300/80"
                          :title="t('chat.collapseSection')"
                          @click.stop="collapseConversationSection(section.key)"
                        >
                          {{ t("chat.collapseSection") }}
                        </button>
                      </div>
                    </CollapsibleGroup>
                  </template>
                </div>
              </div>
            </div>

            <!-- 空状态 -->
            <div
              v-if="displayedRecentSections.length === 0 && displayedCategorySections.length === 0"
              class="px-3 py-4 text-center text-sm text-base-content/60"
            >
              {{ t("chat.conversationSearchEmpty") }}
            </div>
          </template>
        </div>
      </Transition>
    </ChatConversationFloatingScroll>
    <div class="shrink-0 px-2 py-2">
      <div v-if="showSearch" class="pb-1.5">
        <label class="input input-bordered input-sm flex h-8 min-w-0 items-center gap-2 bg-base-100">
          <Search class="h-3.5 w-3.5 opacity-60" />
          <input
            ref="searchInputRef"
            v-model="conversationSearchQuery"
            type="text"
            class="w-full bg-transparent outline-none"
            :placeholder="searchPlaceholder"
          />
        </label>
      </div>
      <div class="flex items-center justify-between gap-2">
        <div class="dropdown dropdown-top dropdown-start">
          <div
            tabindex="0"
            role="button"
            class="flex h-9 w-9 cursor-pointer items-center justify-center rounded-full bg-neutral text-neutral-content outline-none"
          >
            <img
              v-if="props.userAvatarUrl"
              :src="props.userAvatarUrl"
              :alt="props.userAlias || t('chat.userAvatarAlt')"
              class="h-9 w-9 rounded-full object-cover"
            />
            <span v-else class="text-sm font-bold">{{ userAvatarInitial }}</span>
          </div>
          <ul
            tabindex="0"
            class="dropdown-content menu z-50 mb-2 w-44 rounded-box border border-base-300 bg-base-100 p-1 text-base-content shadow-xl"
          >
            <li>
              <button type="button" @click="handleOpenSettings">
                <Settings class="h-3.5 w-3.5" />
                <span>{{ t("common.settings") }}</span>
              </button>
            </li>
            <li>
              <button type="button" @click="handleOpenBatchArchive">
                <Archive class="h-3.5 w-3.5" />
                <span>{{ t("chat.batchArchive.entryAction") }}</span>
              </button>
            </li>
          </ul>
        </div>
        <div class="flex items-center gap-1">
          <button
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0"
            :title="themeToggleTitle"
            @click="toggleTheme"
          >
            <Sun v-if="!darkMode" class="h-4 w-4" />
            <Moon v-else class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0"
            :class="showSearch ? 'text-primary' : 'text-base-content/55'"
            :title="t('chat.conversationFilter.title')"
            @click="showSearch = !showSearch"
          >
            <ListFilter class="h-4 w-4" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0 text-base-content/55"
            :title="t('chat.conversationSearchPanel.openTitle')"
            @click="emit('openConversationSearch')"
          >
            <Search class="h-4 w-4" />
          </button>
          <button
            v-for="item in systemNotificationItems"
            :key="`system-notification-${item.conversationId}`"
            type="button"
            class="btn btn-ghost btn-xs h-7 min-h-7 w-7 min-w-7 p-0"
            :class="isActiveSystemNotificationConversation(item)
              ? 'bg-base-100 text-base-content'
              : 'text-base-content/55'"
            :title="conversationDisplayTitle(item)"
            @click="selectSystemNotificationConversation(item)"
          >
            <Bell class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
    <dialog ref="batchArchiveDialogRef" class="modal" @close="closeBatchArchiveCard" @cancel.prevent="closeBatchArchiveCard">
      <div class="modal-box flex h-[min(88vh,44rem)] w-[min(94vw,64rem)] max-w-none flex-col overflow-hidden p-0">
        <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 px-5 py-4">
          <h3 class="text-base font-semibold">{{ t("chat.batchArchive.title") }}</h3>
        </div>
        <div class="flex min-h-0 flex-1 flex-col bg-base-200/35 px-5 py-4">
          <section class="shrink-0">
            <div class="text-sm font-semibold">{{ t("chat.batchArchive.conditionTitle") }}</div>
            <div class="mt-3 grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.8fr)]">
              <div class="space-y-2">
                <label class="block text-sm font-medium" for="batch-archive-days">
                  {{ t("chat.batchArchive.daysLabel") }}
                </label>
                <div class="flex items-center gap-2">
                  <input
                    id="batch-archive-days"
                    v-model.number="batchArchiveDays"
                    type="number"
                    min="1"
                    step="1"
                    class="input input-bordered w-28"
                  />
                  <span class="text-sm text-base-content/70">{{ t("chat.batchArchive.daysSuffix") }}</span>
                </div>
              </div>
              <div class="space-y-2">
                <label class="block text-sm font-medium" for="batch-archive-model">
                  {{ t("chat.batchArchive.modelLabel") }}
                </label>
                <ApiConfigPicker
                  id="batch-archive-model"
                  v-model="batchArchiveSelectedModelId"
                  :api-configs="batchArchiveApiConfigs"
                />
              </div>
            </div>
            <label class="mt-4 flex items-start gap-3 px-1 py-1">
              <input
                v-model="batchArchiveKeepOnePerWorkspace"
                type="checkbox"
                class="checkbox checkbox-sm mt-0.5"
              />
              <div class="min-w-0 space-y-1 text-sm">
                <div class="font-medium leading-5">{{ t("chat.batchArchive.keepOnePerWorkspace") }}</div>
                <div class="text-xs leading-5 text-base-content/65">{{ t("chat.batchArchive.keepOnePerWorkspaceHint") }}</div>
              </div>
            </label>
          </section>

          <section v-if="batchArchiveCardOpen" class="mt-5 flex min-h-0 flex-1 flex-col">
            <div class="flex items-center justify-between gap-3">
              <div class="text-sm font-semibold">
                {{ t("chat.batchArchive.previewTitle", { count: batchArchiveCandidateConversations.length }) }}
              </div>
            </div>
            <div v-if="batchArchiveCandidateConversations.length === 0" class="mt-3 px-1 py-4 text-sm text-base-content/60">
              {{ t("chat.batchArchive.empty") }}
            </div>
            <div v-else class="mt-3 min-h-0 flex-1 space-y-2 overflow-auto pr-1">
              <article
                v-for="item in batchArchiveCandidateConversations"
                :key="item.conversationId"
                class="flex items-center gap-3 rounded-lg border border-base-300 bg-base-100 px-3 py-2"
              >
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm shrink-0"
                  :checked="isBatchArchiveConversationSelected(item)"
                  @change="toggleBatchArchiveConversation(item, ($event.target as HTMLInputElement).checked)"
                />
                <Archive class="h-4 w-4 shrink-0 text-base-content/55" />
                <div class="min-w-0 flex-1">
                  <div class="truncate text-sm font-medium">{{ conversationDisplayTitle(item) }}</div>
                </div>
                <div class="badge badge-ghost max-w-28 shrink-0 truncate">
                  {{ conversationWorkspaceLabel(item) }}
                </div>
                <div class="w-24 shrink-0 text-right text-xs text-base-content/65">
                  {{ t("chat.batchArchive.olderThanDays", { count: conversationAgeDays(item) }) }}
                </div>
                <div class="w-12 shrink-0 text-right text-xs text-base-content/60">
                  {{ formatConversationTime(item.updatedAt) }}
                </div>
              </article>
            </div>
          </section>
        </div>
        <div class="flex shrink-0 items-center justify-between gap-3 border-t border-base-300 px-5 py-4">
          <div class="flex items-center gap-2">
            <button type="button" class="btn btn-ghost btn-sm" :disabled="batchArchiveCandidateConversations.length === 0" @click="selectAllBatchArchiveCandidates">
              {{ t("chat.batchArchive.selectAll") }}
            </button>
            <button type="button" class="btn btn-ghost btn-sm" :disabled="batchArchiveSelectedConversationIds.size === 0" @click="clearBatchArchiveSelection">
              {{ t("chat.batchArchive.selectNone") }}
            </button>
            <span class="text-xs text-base-content/60">
              {{ t("chat.batchArchive.selectedCount", { count: batchArchiveSelectedConversationIds.size }) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <button type="button" class="btn btn-sm" @click="closeBatchArchiveCard">
              {{ t("common.cancel") }}
            </button>
            <button
              type="button"
              class="btn btn-primary btn-sm"
              :disabled="batchArchiveStartDisabled"
              @click="submitBatchArchive"
            >
              <span v-if="batchArchiveSubmitting" class="loading loading-spinner loading-xs"></span>
              {{ t("chat.batchArchive.startArchive") }}
            </button>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeBatchArchiveCard">close</button>
      </form>
    </dialog>
  </aside>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { Archive, Bell, Check, ChevronDown, ChevronRight, Clock, Folder, LayoutList, ListFilter, Moon, Search, Settings, SquarePen, Sun, UserRound } from "@lucide/vue";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import ChatConversationItem from "./ChatConversationItem.vue";
import type { ApiConfigItem, ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import { stripPreviewMarkdown, stripToolcallMarkers } from "../../../utils/chat-message-semantics";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import { invokeTauri } from "../../../services/tauri-api";
import { usePipelineStatus } from "../../shell/composables/use-pipeline-status";
import { isDarkAppTheme, useAppTheme } from "../../shell/composables/use-app-theme";
import ApiConfigPicker from "../../config/components/ApiConfigPicker.vue";
import { formatConversationListTime } from "../utils/conversation-time";
import {
  aggregateConversationItems,
  conversationLastUsedMs,
} from "../utils/conversation-aggregation";
import {
  applyConversationSectionOrder,
  buildCategoryConversationSections,
  buildRecentConversationSections,
  buildRemoteConversationSections,
  canonicalWorkspaceRootForComparison,
  conversationCountWithinHours,
  CURRENT_PROJECT_SECTION_KEY,
  RECENT_CONVERSATION_SECTION_KEY,
  workspaceNameFromPath,
  type ConversationSection,
  type ConversationSectionGrouping,
  type ConversationSectionOrderState,
} from "../utils/conversation-sections";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import { simpleConversationItemLevel } from "../utils/conversation-item-display";
import ChatConversationFloatingScroll from "./ChatConversationFloatingScroll.vue";
import ChatTaskSidebarPanel from "./ChatTaskSidebarPanel.vue";
import SegmentedControl, { type SegmentedControlOption } from "../../config/components/SegmentedControl.vue";
import EcallDropdown from "../../shared/components/EcallDropdown.vue";

type ConversationSidebarTab = "local" | "contact" | "task";
type DisplayConversationSection = ConversationSection & {
  visibleItems: ChatConversationOverviewItem[];
  /** full 会话 id → 聚合其后的简单条目（同人格旧会话，按更新时间倒序） */
  simpleFollowers: Record<string, ChatConversationOverviewItem[]>;
  visibleCount: number;
  hiddenItemCount: number;
  totalItemCount: number;
};
type BatchArchiveConversationsOutput = {
  success: boolean;
  acceptedConversationIds: string[];
  skipped: Array<{ conversationId: string; reason: string }>;
  activeConversationId?: string;
};

const CONVERSATION_SECTION_UNUSED_DAYS = 7;
const CONVERSATION_SECTION_MIN_VISIBLE = 5;
const CONVERSATION_SECTION_LOAD_MORE_STEP = 10;
const CONVERSATION_SECTION_RESET_DELAY_MS = 30_000;
const CONVERSATION_GROUPING_STORAGE_KEY = "easy-call.chat.conversation-grouping.v1";
const LEGACY_COMPACT_VIEW_STORAGE_KEY = "easy-call.chat.conversation-compact-view.v1";

/** 读分组依据偏好：优先新键；无新键时迁移旧「精简视图」布尔键（1→工作目录，其余→混合） */
function readConversationGroupingPreference(): ConversationSectionGrouping {
  if (typeof window === "undefined") return "mixed";
  try {
    const stored = window.localStorage.getItem(CONVERSATION_GROUPING_STORAGE_KEY);
    if (stored === "persona" || stored === "workspace" || stored === "mixed") return stored;
    if (window.localStorage.getItem(LEGACY_COMPACT_VIEW_STORAGE_KEY) === "1") return "workspace";
  } catch {
    // 存储不可用（如隐私模式）时用默认值
  }
  return "mixed";
}

const props = defineProps<{
  items: ChatConversationOverviewItem[];
  activeConversationId: string;
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  activeTab: ConversationSidebarTab;
  chatModelOptions: ApiConfigItem[];
  toolReviewApiConfigId?: string;
  currentWorkspaceRootPath?: string;
}>();

const emit = defineEmits<{
  (e: "select", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }): void;
  (e: "rename", payload: { conversationId: string; title: string }): void;
  (e: "togglePinConversation", conversationId: string): void;
  (e: "archiveConversation", conversationId: string): void;
  (e: "exportConversation", conversationId: string): void;
  (e: "deleteConversation", conversationId: string): void;
  (e: "update:activeTab", value: ConversationSidebarTab): void;
  (e: "editTask", task: TaskEntry): void;
  (e: "batchArchiveCompleted", payload: { archivedConversationIds: string[]; activeConversationId?: string }): void;
  (e: "openSettings"): void;
  (e: "openConversationSearch"): void;
}>();

const { t, locale } = useI18n();
const { currentTheme, toggleTheme } = useAppTheme();
const darkMode = computed(() => isDarkAppTheme(currentTheme.value));
const themeToggleTitle = computed(() =>
  darkMode.value ? t("appearance.switchToLight") : t("appearance.switchToDark"),
);
/** 头像上拉菜单：DaisyUI dropdown 依赖焦点开合，选中菜单项后主动失焦收起 */
function closeUserMenu(event: MouseEvent) {
  (event.currentTarget as HTMLElement | null)?.closest(".dropdown")
    ?.querySelector<HTMLElement>("[role='button']")?.blur();
}

function handleOpenSettings(event: MouseEvent) {
  closeUserMenu(event);
  emit("openSettings");
}

function handleOpenBatchArchive(event: MouseEvent) {
  closeUserMenu(event);
  openBatchArchiveCard();
}

const conversationSearchQuery = ref("");
const showSearch = ref(false);
const conversationGrouping = ref<ConversationSectionGrouping>(readConversationGroupingPreference());
/** 人格 / 工作目录模式用简单行渲染；混合模式用富卡片 + 人格头像聚合 */
const isSimpleConversationRows = computed(() => conversationGrouping.value !== "mixed");
/** 人格分组依据：置顶会话留在所属人格分组内并排最前 */
const isPersonaGrouping = computed(() => conversationGrouping.value === "persona");

/** 分组依据下拉：EcallDropdown 自带点击外部关闭 */
const groupingMenuOpen = ref(false);
const conversationGroupingOptions = computed<Array<{
  value: ConversationSectionGrouping;
  label: string;
  icon: Component;
}>>(() => [
  { value: "mixed", label: t("chat.groupingByMixed"), icon: LayoutList },
  { value: "workspace", label: t("chat.groupingByWorkspace"), icon: Folder },
  { value: "persona", label: t("chat.groupingByPersona"), icon: UserRound },
]);
const conversationGroupingLabel = computed(() =>
  conversationGroupingOptions.value.find((option) => option.value === conversationGrouping.value)?.label || "",
);
const currentGroupingIcon = computed(() =>
  conversationGroupingOptions.value.find((option) => option.value === conversationGrouping.value)?.icon || LayoutList,
);

const RECENT_TIME_FILTER_KEY = "easy-call.chat.recent-time-filter.v1";
const recentTimeFilterOpen = ref(false);
const recentTimeFilterHours = ref<number>(readRecentTimeFilterPreference());
const RECENT_TIME_FILTER_HOUR_OPTIONS = [12, 24, 48, 72] as const;
const recentTimeFilterOptions = computed(() =>
  RECENT_TIME_FILTER_HOUR_OPTIONS.map((value) => ({
    value,
    label: t("chat.recentWithinHours", { count: value }),
  })),
);
const currentRecentTimeLabel = computed(() => {
  const found = recentTimeFilterOptions.value.find((o) => o.value === recentTimeFilterHours.value);
  return found ? found.label : t("chat.recentWithinHours", { count: 24 });
});

function readRecentTimeFilterPreference(): number {
  if (typeof window === "undefined") return 24;
  try {
    const val = window.localStorage.getItem(RECENT_TIME_FILTER_KEY);
    const n = Number(val);
    if ([12, 24, 48, 72].includes(n)) return n;
  } catch {}
  return 24;
}

function selectRecentTimeFilter(hours: number, close?: () => void) {
  recentTimeFilterHours.value = hours;
  try {
    window.localStorage.setItem(RECENT_TIME_FILTER_KEY, String(hours));
  } catch {}
  // 切换时间窗口时重置“加载更多”，避免旧计数与新过滤结果不匹配
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    "local:recent": 0,
  };
  scheduleConversationListScrollbarUpdate();
  close?.();
}

const RECENT_SUPER_COLLAPSED_KEY = "easy-call.chat.sidebar.recent-super-collapsed";
const CATEGORY_SUPER_COLLAPSED_KEY = "easy-call.chat.sidebar.category-super-collapsed";

function readSuperSectionCollapsed(key: string, defaultValue = false): boolean {
  if (typeof window === "undefined") return defaultValue;
  try {
    const val = window.localStorage.getItem(key);
    if (val !== null) return val === "true";
  } catch {}
  return defaultValue;
}

const isRecentSuperSectionCollapsed = ref(readSuperSectionCollapsed(RECENT_SUPER_COLLAPSED_KEY, false));
const isCategorySuperSectionCollapsed = ref(readSuperSectionCollapsed(CATEGORY_SUPER_COLLAPSED_KEY, false));

const recentSuperCollapsed = computed(() =>
  normalizedConversationSearchQuery.value ? false : isRecentSuperSectionCollapsed.value,
);
const categorySuperCollapsed = computed(() =>
  normalizedConversationSearchQuery.value ? false : isCategorySuperSectionCollapsed.value,
);

function writeSuperSectionCollapsed(key: string, collapsed: boolean) {
  try {
    window.localStorage.setItem(key, String(collapsed));
  } catch {}
}

function toggleRecentSuperSection() {
  isRecentSuperSectionCollapsed.value = !isRecentSuperSectionCollapsed.value;
  writeSuperSectionCollapsed(RECENT_SUPER_COLLAPSED_KEY, isRecentSuperSectionCollapsed.value);
  scheduleConversationListScrollbarUpdate();
}

function toggleCategorySuperSection() {
  isCategorySuperSectionCollapsed.value = !isCategorySuperSectionCollapsed.value;
  writeSuperSectionCollapsed(CATEGORY_SUPER_COLLAPSED_KEY, isCategorySuperSectionCollapsed.value);
  scheduleConversationListScrollbarUpdate();
}

function selectConversationGrouping(value: ConversationSectionGrouping, closeMenu?: () => void) {
  conversationGrouping.value = value;
  groupingMenuOpen.value = false;
  closeMenu?.();
}
const searchInputRef = ref<HTMLInputElement | null>(null);
const batchArchiveDialogRef = ref<HTMLDialogElement | null>(null);
const batchArchiveCardOpen = ref(false);

function syncBatchArchiveDialog() {
  const d = batchArchiveDialogRef.value;
  if (!d) return;
  if (batchArchiveCardOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(batchArchiveCardOpen, syncBatchArchiveDialog);
watch(batchArchiveDialogRef, syncBatchArchiveDialog);
const batchArchiveDays = ref(30);
const batchArchiveKeepOnePerWorkspace = ref(true);
const batchArchiveSelectedModelId = ref("");
const batchArchiveSelectedConversationIds = ref<Set<string>>(new Set());
const batchArchiveSubmitting = ref(false);
const conversationFloatingScrollRef = ref<InstanceType<typeof ChatConversationFloatingScroll> | null>(null);
const collapsedConversationSectionKeys = ref<Record<string, boolean>>({});
const conversationSectionOrders = ref<ConversationSectionOrderState>({ local: [], contact: [] });
const draggingConversationSectionKey = ref("");
const dragOverConversationSectionKey = ref("");
const dragOverConversationSectionPlacement = ref<"before" | "after">("before");
const savingConversationSectionOrder = ref(false);
const conversationSectionLoadMoreCounts = ref<Record<string, number>>({});
const conversationSectionResetTimers = new Map<ConversationSidebarTab, ReturnType<typeof setTimeout>>();
const conversationTabTransitionName = ref("conversation-tab-slide-left");
const activeConversationTab = computed({
  get: (): ConversationSidebarTab => {
    if (props.activeTab === "contact" || props.activeTab === "task") return props.activeTab;
    return "local";
  },
  set: (value: ConversationSidebarTab) => emit("update:activeTab", value),
});
const conversationTabOptions = computed<Array<SegmentedControlOption<ConversationSidebarTab>>>(() => [
  { value: "local", label: t("chat.localConversationTab") },
  { value: "contact", label: t("chat.contactConversationTab") },
  { value: "task", label: t("chat.taskConversationTab") },
]);
const { conversationStatusById, markConversationRead } = usePipelineStatus({
  activeConversationId: computed(() => String(props.activeConversationId || "").trim()),
});

const conversationPreviewCache = computed(() => new Map(
  props.items.map((item) => [String(item.conversationId || "").trim(), Array.isArray(item.previewMessages) ? item.previewMessages : []]),
));

/** 与分组构建同一口径：优先 lastMessageAt，缺省才用 updatedAt */
function recentCandidateRecencyMs(item: ChatConversationOverviewItem): number {
  const raw = String(item.lastMessageAt || item.updatedAt || "").trim();
  if (!raw) return 0;
  const time = Date.parse(raw);
  return Number.isFinite(time) ? time : 0;
}

const allRecentCandidateItems = computed<ChatConversationOverviewItem[]>(() => {
  if (activeConversationTab.value !== "local") return [];
  const normalizedActiveId = String(props.activeConversationId || "").trim();
  const seenIds = new Set<string>();
  return props.items
    .filter((item) => {
      if (String(item.kind || "local_unarchived").trim() === "remote_im_contact") return false;
      if (item.isSystemNotificationConversation) return false;
      if (item.isDraft && String(item.conversationId || "").trim() !== normalizedActiveId) return false;
      return true;
    })
    .sort((left, right) => recentCandidateRecencyMs(right) - recentCandidateRecencyMs(left))
    .filter((item) => {
      const id = String(item.conversationId || "").trim();
      if (!id || seenIds.has(id)) return false;
      seenIds.add(id);
      return true;
    });
});

// 初始展示量 = 时间窗口内活跃的会话，至少 CONVERSATION_SECTION_MIN_VISIBLE 条；
// 窗口之外更早的会话仍留在候选里，由「加载更多」逐步放出。
const recentBaseCount = computed(() =>
  Math.max(
    CONVERSATION_SECTION_MIN_VISIBLE,
    conversationCountWithinHours(allRecentCandidateItems.value, recentTimeFilterHours.value),
  ),
);
const recentExtraCount = computed(() =>
  Math.max(0, Number(conversationSectionLoadMoreCounts.value["local:recent"] || 0)),
);
const recentVisibleLimit = computed(() =>
  normalizedConversationSearchQuery.value
    ? allRecentCandidateItems.value.length
    : (recentBaseCount.value + recentExtraCount.value),
);
const recentHiddenCount = computed(() =>
  Math.max(0, allRecentCandidateItems.value.length - recentVisibleLimit.value),
);

function loadMoreRecentConversations() {
  const current = Number(conversationSectionLoadMoreCounts.value["local:recent"] || 0);
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    "local:recent": current + CONVERSATION_SECTION_LOAD_MORE_STEP,
  };
  scheduleConversationListScrollbarUpdate();
}

function collapseRecentConversations() {
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    "local:recent": 0,
  };
  scheduleConversationListScrollbarUpdate();
}

const rawRecentSections = computed<ConversationSection[]>(() => {
  if (activeConversationTab.value !== "local") return [];
  return buildRecentConversationSections(props.items, {
    titles: {
      recent: t("chat.recentConversations"),
      pinned: t("chat.systemNotifications"),
      other: t("chat.otherConversations"),
      defaultWorkspace: t("chat.defaultWorkspace"),
      currentProject: t("chat.currentProject"),
      unknownPersona: t("chat.unknownPersona"),
    },
    locale: locale.value,
    currentWorkspaceRootPath: props.currentWorkspaceRootPath,
    activeConversationId: props.activeConversationId,
    grouping: conversationGrouping.value,
    personaNameMap: props.personaNameMap,
    maxCount: recentVisibleLimit.value,
  });
});

const rawCategorySections = computed<ConversationSection[]>(() => {
  if (activeConversationTab.value !== "local") return [];
  const sections = buildCategoryConversationSections(props.items, {
    titles: {
      recent: t("chat.recentConversations"),
      pinned: t("chat.systemNotifications"),
      other: t("chat.otherConversations"),
      defaultWorkspace: t("chat.defaultWorkspace"),
      currentProject: t("chat.currentProject"),
      unknownPersona: t("chat.unknownPersona"),
    },
    locale: locale.value,
    grouping: conversationGrouping.value,
    personaNameMap: props.personaNameMap,
  });
  return applyConversationSectionOrder(sections, conversationSectionOrders.value.local).sections;
});

const rawContactSections = computed<ConversationSection[]>(() => {
  if (activeConversationTab.value !== "contact") return [];
  const visibleItems = props.items.filter((item) => String(item.kind || "").trim() === "remote_im_contact");
  const sections = buildRemoteConversationSections(visibleItems, {
    fallbackTitle: t("chat.otherConversations"),
    locale: locale.value,
    pinnedFirst: true,
  });
  return applyConversationSectionOrder(sections, conversationSectionOrders.value.contact).sections;
});

const normalizedConversationSearchQuery = computed(() =>
  String(conversationSearchQuery.value || "").trim().toLocaleLowerCase(),
);

const searchPlaceholder = computed(() =>
  activeConversationTab.value === "task"
    ? t("chat.taskSidebar.searchPlaceholder")
    : t("chat.conversationSearchPlaceholder"),
);

const userAvatarInitial = computed(() => {
  const text = String(props.userAlias || t("chat.userAvatarAlt")).trim();
  return text.charAt(0).toUpperCase() || "U";
});

const batchArchiveApiConfigs = computed(() => props.chatModelOptions.filter((item) => item.enableText));
const batchArchiveModelOptions = computed<Array<{ id: string }>>(() =>
  batchArchiveApiConfigs.value
    .map((item) => ({ id: String(item.id || "").trim() }))
    .filter((item) => !!item.id),
);

const batchArchiveCandidateConversations = computed(() => {
  const thresholdDays = Math.max(1, Math.round(Number(batchArchiveDays.value || 0)));
  const now = Date.now();
  const oldLocalConversations = props.items
    .filter((item) => {
      if (!isLocalConversation(item) || item.isSystemNotificationConversation) return false;
      const updatedAt = Date.parse(String(item.updatedAt || item.lastMessageAt || "").trim());
      if (!Number.isFinite(updatedAt)) return false;
      const ageDays = Math.floor((now - updatedAt) / 86_400_000);
      return ageDays >= thresholdDays;
    });
  if (!batchArchiveKeepOnePerWorkspace.value) {
    return sortBatchArchiveCandidates(oldLocalConversations);
  }
  const preservedConversationIds = new Set<string>();
  const newestByWorkspace = new Map<string, ChatConversationOverviewItem>();
  for (const item of oldLocalConversations) {
    const workspaceKey = conversationWorkspaceKey(item);
    const current = newestByWorkspace.get(workspaceKey);
    if (!current || conversationTimeValue(item) > conversationTimeValue(current)) {
      newestByWorkspace.set(workspaceKey, item);
    }
  }
  for (const item of newestByWorkspace.values()) {
    preservedConversationIds.add(String(item.conversationId || "").trim());
  }
  return sortBatchArchiveCandidates(oldLocalConversations.filter((item) =>
    !preservedConversationIds.has(String(item.conversationId || "").trim()),
  ));
});

const batchArchiveSelectedCandidateIds = computed(() => {
  const selectedIds = batchArchiveSelectedConversationIds.value;
  return batchArchiveCandidateConversations.value
    .map(batchArchiveConversationId)
    .filter((id) => !!id && selectedIds.has(id));
});

const batchArchiveStartDisabled = computed(() =>
  batchArchiveSubmitting.value
  || batchArchiveSelectedCandidateIds.value.length === 0
  || !String(batchArchiveSelectedModelId.value || "").trim(),
);

watch(
  () => [props.toolReviewApiConfigId, batchArchiveModelOptions.value.map((item) => item.id).join("|")] as const,
  () => {
    const quickModelId = String(props.toolReviewApiConfigId || "").trim();
    const optionIds = batchArchiveModelOptions.value.map((item) => item.id);
    if (quickModelId && optionIds.includes(quickModelId)) {
      batchArchiveSelectedModelId.value = quickModelId;
      return;
    }
    if (!optionIds.includes(batchArchiveSelectedModelId.value)) {
      batchArchiveSelectedModelId.value = optionIds[0] || "";
    }
  },
  { immediate: true },
);

watch(
  () => batchArchiveCandidateConversations.value.map((item) => String(item.conversationId || "").trim()).filter(Boolean),
  (candidateIds) => {
    const candidateSet = new Set(candidateIds);
    const next = new Set<string>();
    for (const id of batchArchiveSelectedConversationIds.value) {
      if (candidateSet.has(id)) next.add(id);
    }
    batchArchiveSelectedConversationIds.value = next;
  },
  { immediate: true },
);

function filterSectionBySearch(section: ConversationSection, query: string): ConversationSection | null {
  const filtered = section.items.filter((item) => conversationMatchesSearch(item, query));
  if (filtered.length === 0) return null;
  return { ...section, items: filtered };
}

const displayedRecentSections = computed<DisplayConversationSection[]>(() => {
  const query = normalizedConversationSearchQuery.value;
  return rawRecentSections.value
    .map((s) => (query ? filterSectionBySearch(s, query) : s))
    .filter((s): s is ConversationSection => s !== null)
    .map((s) => buildDisplayedConversationSection(s));
});

const displayedCategorySections = computed<DisplayConversationSection[]>(() => {
  const query = normalizedConversationSearchQuery.value;
  return rawCategorySections.value
    .map((s) => (query ? filterSectionBySearch(s, query) : s))
    .filter((s): s is ConversationSection => s !== null)
    .map((s) => buildDisplayedConversationSection(s));
});

const displayedContactSections = computed<DisplayConversationSection[]>(() => {
  const query = normalizedConversationSearchQuery.value;
  return rawContactSections.value
    .map((s) => (query ? filterSectionBySearch(s, query) : s))
    .filter((s): s is ConversationSection => s !== null)
    .map((s) => buildDisplayedConversationSection(s));
});

const allConversationSections = computed<ConversationSection[]>(() => {
  if (activeConversationTab.value === "contact") return rawContactSections.value;
  return [...rawRecentSections.value, ...rawCategorySections.value];
});

/** 系统通知会话不再作为可折叠分组，改为右下角操作栏的铃铛图标按钮。
 *  直接取原始会话列表，不依赖当前标签的分组结果——远程标签下不会生成 pinned 分组。 */
const systemNotificationItems = computed(() => {
  const query = normalizedConversationSearchQuery.value;
  return props.items.filter((item) => {
    if (!item.isSystemNotificationConversation) return false;
    return query ? conversationMatchesSearch(item, query) : true;
  });
});

/** 当前打开的是会话草稿时，高亮「新建会话」按钮 */
const activeConversationIsDraft = computed(() => {
  const activeId = String(props.activeConversationId || "").trim();
  if (!activeId) return false;
  return props.items.some((item) => String(item.conversationId || "").trim() === activeId && !!item.isDraft);
});

function isActiveSystemNotificationConversation(item: ChatConversationOverviewItem): boolean {
  const itemId = String(item.conversationId || "").trim();
  return !!itemId && itemId === String(props.activeConversationId || "").trim();
}

watch(
  () => props.activeConversationId,
  (conversationId) => markConversationRead(conversationId),
  { immediate: true },
);

watch(
  () => activeConversationTab.value,
  (nextValue, previousValue) => {
    if (previousValue && previousValue !== nextValue) {
      scheduleConversationSectionReset(previousValue);
    }
    clearConversationSectionResetTimer(nextValue);
    if (!previousValue || nextValue === previousValue) return;
    conversationTabTransitionName.value = conversationTabOrder(nextValue) > conversationTabOrder(previousValue)
      ? "conversation-tab-slide-left"
      : "conversation-tab-slide-right";
  },
);

onMounted(() => {
  void loadConversationSectionOrders();
});

onBeforeUnmount(() => {
  clearConversationSectionResetTimer("local");
  clearConversationSectionResetTimer("contact");
  clearConversationSectionResetTimer("task");
});

async function loadConversationSectionOrders() {
  try {
    const result = await invokeTauri<ConversationSectionOrderState>("conversation.sectionOrders.get");
    conversationSectionOrders.value = {
      local: Array.isArray(result?.local) ? result.local.map((item) => String(item || "").trim()).filter(Boolean) : [],
      contact: Array.isArray(result?.contact) ? result.contact.map((item) => String(item || "").trim()).filter(Boolean) : [],
    };
  } catch (error) {
    console.warn("[会话分组排序] 读取失败", { error });
  }
}

async function persistConversationSectionOrder(tab: "local" | "contact", orderedKeys: string[]) {
  savingConversationSectionOrder.value = true;
  try {
    const result = await invokeTauri<{ orderedKeys?: string[] }>("conversation.sectionOrders.save", {
      input: {
        tab,
        orderedKeys,
      },
    });
    conversationSectionOrders.value = {
      ...conversationSectionOrders.value,
      [tab]: Array.isArray(result?.orderedKeys)
        ? result.orderedKeys.map((item) => String(item || "").trim()).filter(Boolean)
        : orderedKeys,
    };
  } catch (error) {
    console.warn("[会话分组排序] 保存失败", { tab, error });
  } finally {
    savingConversationSectionOrder.value = false;
  }
}

function isConversationSectionDraggable(section: ConversationSection): boolean {
  if (normalizedConversationSearchQuery.value) return false;
  if (section.key.startsWith("recent:")) return false;
  return section.key !== "pinned"
    && section.key !== RECENT_CONVERSATION_SECTION_KEY
    && section.key !== CURRENT_PROJECT_SECTION_KEY;
}

function handleConversationSectionDragStart(section: ConversationSection, event: DragEvent) {
  handleConversationSectionDragEnd();
  if (!isConversationSectionDraggable(section)) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  draggingConversationSectionKey.value = section.key;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", section.key);
    const currentTarget = event.currentTarget;
    if (currentTarget instanceof HTMLElement) {
      event.dataTransfer.setDragImage(currentTarget, 16, 16);
    }
  }
}

function handleConversationSectionDragOver(section: ConversationSection, event: DragEvent) {
  if (!draggingConversationSectionKey.value || !isConversationSectionDraggable(section)) return;
  event.preventDefault();
  const currentTarget = event.currentTarget;
  if (currentTarget instanceof HTMLElement) {
    const rect = currentTarget.getBoundingClientRect();
    const offsetY = event.clientY - rect.top;
    dragOverConversationSectionPlacement.value = offsetY >= rect.height / 2 ? "after" : "before";
    dragOverConversationSectionKey.value = section.key;
  }
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = "move";
  }
}

function handleConversationSectionDragEnd() {
  draggingConversationSectionKey.value = "";
  dragOverConversationSectionKey.value = "";
  dragOverConversationSectionPlacement.value = "before";
}

function handleConversationSectionDrop(section: ConversationSection, event: DragEvent) {
  const draggingKey = String(draggingConversationSectionKey.value || "").trim();
  if (!draggingKey || draggingKey === section.key || !isConversationSectionDraggable(section)) {
    handleConversationSectionDragEnd();
    return;
  }
  event.preventDefault();
  const tab = activeConversationTab.value === "contact" ? "contact" : "local";
  const sourceSections = tab === "contact" ? rawContactSections.value : rawCategorySections.value;
  const draggableKeys = sourceSections
    .filter((item) => isConversationSectionDraggable(item))
    .map((item) => item.key);
  const fromIndex = draggableKeys.indexOf(draggingKey);
  const rawToIndex = draggableKeys.indexOf(section.key);
  const dropPlacement = dragOverConversationSectionKey.value === section.key
    ? dragOverConversationSectionPlacement.value
    : "before";
  const toIndex = dropPlacement === "after" ? rawToIndex + 1 : rawToIndex;
  if (fromIndex < 0 || toIndex < 0) {
    handleConversationSectionDragEnd();
    return;
  }
  const nextDraggableKeys = [...draggableKeys];
  const [moved] = nextDraggableKeys.splice(fromIndex, 1);
  const adjustedToIndex = fromIndex < toIndex ? toIndex - 1 : toIndex;
  nextDraggableKeys.splice(adjustedToIndex, 0, moved);
  handleConversationSectionDragEnd();
  const currentSavedOrder = conversationSectionOrders.value[tab] || [];
  const orderUnchanged = nextDraggableKeys.length === currentSavedOrder.length
    && nextDraggableKeys.every((key, index) => key === currentSavedOrder[index]);
  if (orderUnchanged) return;
  conversationSectionOrders.value = {
    ...conversationSectionOrders.value,
    [tab]: nextDraggableKeys,
  };
  void persistConversationSectionOrder(tab, nextDraggableKeys);
}

function conversationSectionDragIndicator(section: ConversationSection): "before" | "after" | null {
  if (dragOverConversationSectionKey.value !== section.key) return null;
  return dragOverConversationSectionPlacement.value;
}

function defaultVisibleConversationCount(section: ConversationSection): number {
  const items = Array.isArray(section.items) ? section.items : [];
  if (items.length <= CONVERSATION_SECTION_MIN_VISIBLE) return items.length;
  if (normalizedConversationSearchQuery.value) return items.length;
  if (section.key === "pinned") return items.length;
  if (section.key.startsWith("recent:") || section.key === RECENT_CONVERSATION_SECTION_KEY) {
    // 「最近」大区的可见量在上游按 maxCount（窗口内条数 + 加载更多步进）统一截断，
    // 这里不再二次截断，否则「加载更多」放出来的条目会被重新砍回去。
    return items.length;
  }
  const thresholdMs = Date.now() - CONVERSATION_SECTION_UNUSED_DAYS * 24 * 60 * 60 * 1000;
  const recentCount = items.reduce((count, item) => (
    conversationLastUsedMs(item) >= thresholdMs ? count + 1 : count
  ), 0);
  const activeConversationId = String(props.activeConversationId || "").trim();
  const activeIndex = activeConversationId
    ? items.findIndex((item) => String(item.conversationId || "").trim() === activeConversationId)
    : -1;
  return Math.min(
    items.length,
    Math.max(CONVERSATION_SECTION_MIN_VISIBLE, recentCount, activeIndex >= 0 ? activeIndex + 1 : 0),
  );
}

function conversationSectionLoadMoreKey(
  sectionKey: string,
  tab: ConversationSidebarTab = activeConversationTab.value,
): string {
  return `${tab}:${sectionKey}`;
}

/** 加载更多行的前导占位：与所在分组的会话行使用同一套兜底宽度，保证文字左缘对齐 */
const conversationSectionLeadStyle = computed(() => {
  const fallback = isSimpleConversationRows.value ? "1rem" : "2.5rem";
  return { width: `max(var(--ecall-section-lead, ${fallback}), ${fallback})` };
});

/** 置顶用箭头；最近子分组与工作区分组用文件夹；其余默认文件夹 */
function conversationSectionIcon(section: ConversationSection): "chevron" | "folder" {
  if (activeConversationTab.value === "contact") return "chevron";
  return section.key === "pinned" || section.key === RECENT_CONVERSATION_SECTION_KEY ? "chevron" : "folder";
}

/** 人格分组折叠头显示人格头像；其余分组仍用文件夹图标 */
function conversationSectionAvatarUrl(section: ConversationSection): string | undefined {
  const personaId = String(section.personaId || "").trim();
  if (!personaId) return undefined;
  return String(props.personaAvatarUrlMap?.[personaId] || "").trim() || undefined;
}

function buildDisplayedConversationSection(section: ConversationSection): DisplayConversationSection {
  const items = Array.isArray(section.items) ? section.items : [];
  const stateKey = conversationSectionLoadMoreKey(section.key);
  const baseVisibleCount = defaultVisibleConversationCount(section);
  const extraVisibleCount = Math.max(0, Number(conversationSectionLoadMoreCounts.value[stateKey] || 0));
  const visibleCount = Math.min(items.length, baseVisibleCount + extraVisibleCount);
  const sliced = items.slice(0, visibleCount);

  // 精简模式：不做 full 聚合，条目统一为简单行，但仍按同样规则截断并支持「加载更多」
  if (isSimpleConversationRows.value) {
    return {
      ...section,
      visibleItems: sliced,
      simpleFollowers: {},
      visibleCount: visibleCount,
      hiddenItemCount: Math.max(0, items.length - visibleCount),
      totalItemCount: items.length,
    };
  }
  const { reorderedItems, simpleFollowers } = aggregateConversationItems(sliced, {
    searchActive: !!normalizedConversationSearchQuery.value,
  });
  return {
    ...section,
    visibleItems: reorderedItems,
    simpleFollowers,
    visibleCount: visibleCount,
    hiddenItemCount: Math.max(0, items.length - visibleCount),
    totalItemCount: items.length,
  };
}

function loadMoreConversationsInSection(sectionKey: string) {
  const key = String(sectionKey || "").trim();
  if (!key) return;
  const stateKey = conversationSectionLoadMoreKey(key);
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    [stateKey]: Math.max(0, Number(conversationSectionLoadMoreCounts.value[stateKey] || 0)) + CONVERSATION_SECTION_LOAD_MORE_STEP,
  };
  scheduleConversationListScrollbarUpdate();
}

function conversationSectionHasExtraItems(sectionKey: string): boolean {
  const stateKey = conversationSectionLoadMoreKey(String(sectionKey || "").trim());
  return Number(conversationSectionLoadMoreCounts.value[stateKey] || 0) > 0;
}

function collapseConversationSection(sectionKey: string) {
  const key = String(sectionKey || "").trim();
  if (!key || !conversationSectionHasExtraItems(key)) return;
  const stateKey = conversationSectionLoadMoreKey(key);
  conversationSectionLoadMoreCounts.value = {
    ...conversationSectionLoadMoreCounts.value,
    [stateKey]: 0,
  };
  scheduleConversationListScrollbarUpdate();
}

function clearConversationSectionResetTimer(tab: ConversationSidebarTab) {
  const timer = conversationSectionResetTimers.get(tab);
  if (!timer) return;
  clearTimeout(timer);
  conversationSectionResetTimers.delete(tab);
}

function resetConversationSectionLoadMore(tab: ConversationSidebarTab) {
  if (tab === "task") return;
  const prefix = `${tab}:`;
  conversationSectionLoadMoreCounts.value = Object.fromEntries(
    Object.entries(conversationSectionLoadMoreCounts.value)
      .filter(([stateKey, value]) => !stateKey.startsWith(prefix) || !(Number(value) > 0)),
  );
  scheduleConversationListScrollbarUpdate();
}

function scheduleConversationSectionReset(tab: ConversationSidebarTab) {
  if (tab === "task") return;
  clearConversationSectionResetTimer(tab);
  conversationSectionResetTimers.set(tab, setTimeout(() => {
    conversationSectionResetTimers.delete(tab);
    if (activeConversationTab.value === tab) return;
    resetConversationSectionLoadMore(tab);
  }, CONVERSATION_SECTION_RESET_DELAY_MS));
}

watch(showSearch, async (visible) => {
  if (visible) {
    await nextTick();
    searchInputRef.value?.focus();
  } else {
    conversationSearchQuery.value = "";
  }
});

watch(conversationGrouping, (grouping) => {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(CONVERSATION_GROUPING_STORAGE_KEY, grouping);
  } catch {
    // 存储不可用（如隐私模式）时仅内存生效
  }
});

function isConversationSectionCollapsed(key: string): boolean {
  if (normalizedConversationSearchQuery.value) return false;
  const collapsed = collapsedConversationSectionKeys.value[key];
  if (collapsed !== undefined) return collapsed;
  if (key.startsWith("recent:")) return false;
  if (key === CURRENT_PROJECT_SECTION_KEY) return false;
  return true;
}

function toggleConversationSection(key: string) {
  const nextCollapsed = !isConversationSectionCollapsed(key);
  collapsedConversationSectionKeys.value = {
    ...collapsedConversationSectionKeys.value,
    [key]: nextCollapsed,
  };
  if (nextCollapsed) {
    const stateKey = conversationSectionLoadMoreKey(key);
    if (Number(conversationSectionLoadMoreCounts.value[stateKey] || 0) > 0) {
      // 折叠分组时清掉「加载更多」展开量，重新展开后回到默认可见数量。
      conversationSectionLoadMoreCounts.value = {
        ...conversationSectionLoadMoreCounts.value,
        [stateKey]: 0,
      };
    }
  }
  scheduleConversationListScrollbarUpdate();
}

function expandConversationSection(key: string) {
  if (!key || !isConversationSectionCollapsed(key)) return;
  collapsedConversationSectionKeys.value = {
    ...collapsedConversationSectionKeys.value,
    [key]: false,
  };
  scheduleConversationListScrollbarUpdate();
}

const conversationSectionElements = new Map<string, HTMLElement>();

function setConversationSectionElement(key: string, element: unknown) {
  const root = element instanceof HTMLElement
    ? element
    : (element as { $el?: HTMLElement | null } | null)?.$el ?? null;
  if (root) conversationSectionElements.set(key, root);
  else conversationSectionElements.delete(key);
}

function revealConversationSection(item: ChatConversationOverviewItem) {
  const conversationId = String(item.conversationId || "").trim();
  if (!conversationId) return;
  const section = rawCategorySections.value.find((entry) =>
    entry.items.some((candidate) => String(candidate.conversationId || "").trim() === conversationId),
  );
  if (!section) return;
  const wasCollapsed = isConversationSectionCollapsed(section.key);
  expandConversationSection(section.key);
  if (isCategorySuperSectionCollapsed.value) {
    isCategorySuperSectionCollapsed.value = false;
    writeSuperSectionCollapsed(CATEGORY_SUPER_COLLAPSED_KEY, false);
  }
  const element = conversationSectionElements.get(section.key);
  window.setTimeout(() => {
    if (element) conversationFloatingScrollRef.value?.scrollToElement(element);
  }, wasCollapsed ? 220 : 0);
}

function collapseAllConversationSections() {
  collapsedConversationSectionKeys.value = allConversationSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedConversationSectionKeys.value } as Record<string, boolean>);
  const tabPrefix = `${activeConversationTab.value}:`;
  conversationSectionLoadMoreCounts.value = Object.fromEntries(
    Object.entries(conversationSectionLoadMoreCounts.value)
      .filter(([stateKey, value]) => !stateKey.startsWith(tabPrefix) || !(Number(value) > 0)),
  );
  scheduleConversationListScrollbarUpdate();
}

function conversationTabOrder(value: ConversationSidebarTab): number {
  if (value === "task") return 2;
  if (value === "contact") return 1;
  return 0;
}

function requestTaskEdit(task: TaskEntry) {
  emit("editTask", task);
}

function scheduleConversationListScrollbarUpdate() {
  void nextTick(() => {
    requestAnimationFrame(() => conversationFloatingScrollRef.value?.updateThumb());
  });
}

function handleConversationTabTransitionSettled() {
  scheduleConversationListScrollbarUpdate();
}

function createConversationFromSidebar() {
  window.dispatchEvent(new CustomEvent("easy-call:open-draft-conversation"));
}

function selectSystemNotificationConversation(item: ChatConversationOverviewItem) {
  const conversationId = String(item.conversationId || "").trim();
  if (!conversationId) return;
  emit("select", {
    conversationId,
    kind: item.kind,
    remoteContactId: String(item.remoteContactId || "").trim() || undefined,
  });
}

function createConversationInSection(section: ConversationSection) {
  // 新建入口统一打开会话草稿；从文件夹分节新建时把该文件夹作为草稿工作区
  window.dispatchEvent(new CustomEvent("easy-call:open-draft-conversation", {
    detail: {
      workspaceRootPath: String(section.workspaceRootPath || "").trim() || undefined,
    },
  }));
}

function normalizedPreviewMessages(item: ChatConversationOverviewItem): ConversationPreviewMessage[] {
  return conversationPreviewCache.value.get(String(item.conversationId || "").trim()) || [];
}

function conversationMatchesSearch(item: ChatConversationOverviewItem, query: string): boolean {
  if (!query) return true;
  const title = conversationDisplayTitle(item).toLocaleLowerCase();
  if (title.includes(query)) return true;
  const previewTextBlock = normalizedPreviewMessages(item)
    .slice(-2)
    .map((preview) => previewText(preview).toLocaleLowerCase())
    .join("\n");
  return previewTextBlock.includes(query);
}

function isLocalConversation(item: ChatConversationOverviewItem): boolean {
  return item.kind !== "remote_im_contact";
}

function conversationDisplayTitle(item: ChatConversationOverviewItem): string {
  return resolveConversationDisplayTitle(item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
    systemNotificationLabel: t("chat.systemPersona"),
  });
}

function isRecentConversationSection(sectionKey: string): boolean {
  return sectionKey === RECENT_CONVERSATION_SECTION_KEY;
}

/** 简单项等级：未读或 7 天内有更新 → sim（摘要常显）；否则 → mini（摘要折叠、hover 展开） */
function simpleItemLevel(item: ChatConversationOverviewItem): "sim" | "mini" {
  return simpleConversationItemLevel(item);
}

function openBatchArchiveCard() {
  batchArchiveCardOpen.value = true;
  selectAllBatchArchiveCandidates();
}

function closeBatchArchiveCard() {
  if (batchArchiveSubmitting.value) return;
  batchArchiveCardOpen.value = false;
}

function batchArchiveConversationId(item: ChatConversationOverviewItem): string {
  return String(item.conversationId || "").trim();
}

function isBatchArchiveConversationSelected(item: ChatConversationOverviewItem): boolean {
  const id = batchArchiveConversationId(item);
  return !!id && batchArchiveSelectedConversationIds.value.has(id);
}

function toggleBatchArchiveConversation(item: ChatConversationOverviewItem, checked: boolean) {
  const id = batchArchiveConversationId(item);
  if (!id) return;
  const next = new Set(batchArchiveSelectedConversationIds.value);
  if (checked) {
    next.add(id);
  } else {
    next.delete(id);
  }
  batchArchiveSelectedConversationIds.value = next;
}

function selectAllBatchArchiveCandidates() {
  batchArchiveSelectedConversationIds.value = new Set(
    batchArchiveCandidateConversations.value
      .map(batchArchiveConversationId)
      .filter(Boolean),
  );
}

function clearBatchArchiveSelection() {
  batchArchiveSelectedConversationIds.value = new Set();
}

async function submitBatchArchive() {
  const conversationIds = batchArchiveSelectedCandidateIds.value;
  const reflectionApiConfigId = String(batchArchiveSelectedModelId.value || "").trim();
  if (conversationIds.length === 0 || !reflectionApiConfigId || batchArchiveSubmitting.value) return;
  batchArchiveSubmitting.value = true;
  try {
    const payload = { conversationIds, reflectionApiConfigId };
    const result = await invokeTauri<BatchArchiveConversationsOutput>("conversation.batchArchive", payload, 30_000);
    batchArchiveSelectedConversationIds.value = new Set();
    batchArchiveCardOpen.value = false;
    emit("batchArchiveCompleted", {
      archivedConversationIds: Array.isArray(result.acceptedConversationIds) ? result.acceptedConversationIds : [],
      activeConversationId: String(result.activeConversationId || "").trim() || undefined,
    });
    if (Array.isArray(result.skipped) && result.skipped.length > 0) {
      console.warn("[批量归档] 部分会话跳过", result.skipped);
    }
  } catch (error) {
    console.warn("[批量归档] 提交失败", error);
  } finally {
    batchArchiveSubmitting.value = false;
  }
}

function conversationAgeDays(item: ChatConversationOverviewItem): number {
  const updatedAt = conversationTimeValue(item);
  if (!Number.isFinite(updatedAt)) return 0;
  return Math.max(0, Math.floor((Date.now() - updatedAt) / 86_400_000));
}

function conversationTimeValue(item: ChatConversationOverviewItem): number {
  return Date.parse(String(item.updatedAt || item.lastMessageAt || "").trim()) || 0;
}

function sortBatchArchiveCandidates(items: ChatConversationOverviewItem[]): ChatConversationOverviewItem[] {
  return [...items].sort((left, right) => conversationTimeValue(left) - conversationTimeValue(right));
}

function conversationWorkspaceKey(item: ChatConversationOverviewItem): string {
  const rootPath = String(item.workspaceRootPath || "").trim();
  if (rootPath) return rootPath.toLocaleLowerCase();
  return `label:${conversationWorkspaceLabel(item).toLocaleLowerCase()}`;
}

function conversationWorkspaceLabel(item: ChatConversationOverviewItem): string {
  return String(item.workspaceLabel || workspaceNameFromPath(String(item.workspaceRootPath || "").trim()) || t("chat.defaultWorkspace")).trim();
}

function previewText(preview: ConversationPreviewMessage): string {
  const text = stripPreviewMarkdown(stripToolcallMarkers(preview.textPreview || ""));
  if (text) return text;
  if (preview.hasPdf) return t("chat.previewPdf");
  if (preview.hasImage) return t("chat.previewImage");
  if (preview.hasAudio) return t("chat.previewAudio");
  if (preview.hasAttachment) return t("chat.previewAttachment");
  return t("chat.conversationNoPreview");
}

function formatConversationTime(value?: string): string {
  return formatConversationListTime(value, locale.value);
}

</script>

<style scoped>
.conversation-tab-panel {
  min-height: 100%;
}

.conversation-time-container {
  container-type: inline-size;
}

@container (max-width: 229px) {
  .conversation-time-label {
    display: none;
  }
}

.conversation-tab-slide-left-enter-active,
.conversation-tab-slide-left-leave-active,
.conversation-tab-slide-right-enter-active,
.conversation-tab-slide-right-leave-active {
  transition:
    opacity 200ms ease,
    transform 200ms cubic-bezier(0.22, 1, 0.36, 1);
}

.conversation-tab-slide-left-enter-from,
.conversation-tab-slide-right-leave-to {
  opacity: 0;
  transform: translateX(12px);
}

.conversation-tab-slide-left-leave-to,
.conversation-tab-slide-right-enter-from {
  opacity: 0;
  transform: translateX(-12px);
}

.super-section-shell {
  display: grid;
  grid-template-rows: 1fr;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  transition: grid-template-rows 180ms cubic-bezier(0.22, 1, 0.36, 1);
}

.super-section-shell.is-collapsed {
  grid-template-rows: 0fr;
}

.super-section-inner {
  min-height: 0;
  min-width: 0;
  overflow: clip;
  visibility: visible;
  transition: visibility 180ms;
}

.super-section-shell.is-collapsed .super-section-inner {
  visibility: hidden;
}

@media (prefers-reduced-motion: reduce) {
  .super-section-shell {
    transition: none;
  }
}
</style>
