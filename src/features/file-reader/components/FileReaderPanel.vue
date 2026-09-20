<template>
  <div class="relative flex h-full min-h-0 flex-col bg-base-100 text-base-content">
    <PanelTabStrip
      v-if="showTabs"
      variant="file"
      :tabs="fileReaderPanelTabs"
      :active-key="activePath"
      :aria-label="t('fileReader.tabs')"
      :close-title="t('fileReader.close')"
      :close-left-title="t('fileReader.closeLeft')"
      :close-right-title="t('fileReader.closeRight')"
      :close-others-title="t('fileReader.closeOthers')"
      :context-menu-items="fileTabContextMenuItems"
      @select-tab="setActiveTab"
      @close-tab="closeTab"
      @close-tabs-to-left="closeTabsToLeftOf"
      @close-tabs-to-right="closeTabsToRightOf"
      @close-other-tabs="closeOtherTabs"
    >
      <template #leading>
        <slot name="tabLeadingActions">
          <button
            v-if="showPickFileButton"
            class="btn btn-ghost btn-sm shrink-0"
            type="button"
            :title="t('fileReader.openFile')"
            @click.stop="pickFile"
          >
            <FilePlus class="size-4" />
          </button>
        </slot>
      </template>
      <template #actions>
        <div v-if="localFileSystemAvailable" class="join shrink-0">
          <button
            class="btn btn-sm h-8 min-h-8 w-8 join-item border-0 bg-base-100/60 px-0 shadow-none hover:bg-base-300"
            type="button"
            :disabled="directoryTreeRoot ? directoryTreeRoot.loading : !directoryToggleTargetPath"
            :title="selectedDirectoryOpenTargetTitle"
            @click="openDirectoryAtTreeRoot()"
          >
            <img
              v-if="currentDirectoryOpenTarget.iconDataUrl"
              :src="currentDirectoryOpenTarget.iconDataUrl"
              alt=""
              class="h-4 w-4 shrink-0 object-contain"
            />
            <SquareTerminal v-else-if="currentDirectoryOpenTarget.type === 'shell'" class="h-4 w-4" />
            <Code2 v-else-if="currentDirectoryOpenTarget.type === 'vscode'" class="h-4 w-4" />
            <Folders v-else class="h-4 w-4" />
          </button>
          <div ref="directoryOpenTargetDropdownRef" class="dropdown dropdown-end z-20">
            <button
              class="btn btn-sm h-8 min-h-8 w-8 join-item border-0 bg-base-100/60 px-0 shadow-none hover:bg-base-300"
              type="button"
              :disabled="(directoryTreeRoot?.loading ?? false) || directoryOpenTargetsLoading"
              title="切换打开目标"
              @click.stop="toggleDirectoryOpenTargetDropdown"
            >
              <ChevronDown class="h-4 w-4" />
            </button>
            <ul v-if="directoryOpenTargetDropdownOpen" tabindex="0" class="dropdown-content menu z-50 mt-2 rounded-box border border-base-300 bg-base-100 p-1.5 text-sm shadow-xl" @click.stop>
              <li class="menu-title px-2 py-1 text-xs uppercase tracking-wide opacity-60">
                <span>打开当前目录</span>
              </li>
              <li v-for="item in directoryOpenTargets" :key="item.kind">
                <button
                  type="button"
                  class="flex min-h-9 w-52 items-center justify-between gap-3 rounded-btn px-3 py-2 text-left"
                  :class="selectedDirectoryOpenTargetKind === item.kind ? 'active' : ''"
                  :disabled="directoryTreeRoot ? directoryTreeRoot.loading : !directoryToggleTargetPath"
                  :title="item.label"
                  @click="selectDirectoryOpenTarget(item.kind)"
                >
                  <span class="flex min-w-0 items-center gap-2">
                    <img
                      v-if="item.iconDataUrl"
                      :src="item.iconDataUrl"
                      alt=""
                      class="h-4 w-4 shrink-0 object-contain"
                    />
                    <SquareTerminal v-else-if="item.type === 'shell'" class="h-4 w-4 shrink-0" />
                    <Code2 v-else-if="item.type === 'vscode'" class="h-4 w-4 shrink-0" />
                    <Folders v-else class="h-4 w-4 shrink-0" />
                    <span class="min-w-0 truncate">{{ item.label }}</span>
                  </span>
                  <Check v-if="selectedDirectoryOpenTargetKind === item.kind" class="h-4 w-4 shrink-0" />
                </button>
              </li>
            </ul>
          </div>
        </div>
        <button
          type="button"
          class="btn btn-sm btn-square shrink-0"
          :class="isFilesPanelOpen ? 'bg-base-300 text-primary border-base-300 shadow-sm' : 'btn-ghost'"
          :disabled="!directoryTreeRoot && !directoryToggleTargetPath"
          :title="t('fileReader.filesTab')"
          @click="toggleFilesPanel"
        >
          <Files class="size-4" />
        </button>
        <button
          type="button"
          class="btn btn-sm btn-square shrink-0"
          :class="isGitPanelOpen ? 'bg-base-300 text-primary border-base-300 shadow-sm' : 'btn-ghost'"
          :disabled="!directoryTreeRoot && !directoryToggleTargetPath && !gitPanelWorkspacePath"
          :title="t('fileReader.gitTab')"
          @click="toggleGitPanel"
        >
          <GitBranch class="size-4" />
        </button>
      </template>
    </PanelTabStrip>

    <div ref="fileReaderLayoutRoot" class="relative flex min-h-0 flex-1" :class="directoryOnly ? '' : 'flex-row-reverse'">
      <Transition name="file-reader-aside">
        <aside
          v-if="directoryTreeRoot"
          class="file-reader-aside-panel flex shrink-0 flex-col bg-base-200/35"
          :class="directoryOnly || directoryTreeFullscreen ? 'w-full' : ''"
          :style="directoryOnly || directoryTreeFullscreen ? undefined : { width: `${effectiveDirectoryTreeWidth}px` }"
        >
          <Transition name="file-reader-aside-content" mode="out-in">
            <div v-if="asideMode === 'files'" key="files" class="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div class="flex h-8 shrink-0 items-center gap-1.5 border-b border-base-300 bg-base-200/35 px-3 text-sm">
            <button
              v-if="localFileSystemAvailable"
              class="btn btn-ghost btn-xs min-w-0 flex-1 justify-start gap-1.5 overflow-hidden font-medium"
              :title="directoryTreeRoot.path"
              @click="openDirectoryInFileManager(directoryTreeRoot.path)"
              @contextmenu.prevent.stop="openPathOnlyContextMenu(directoryTreeRoot.path, $event)"
            >
              <Folders class="h-3.5 w-3.5 shrink-0 opacity-75" />
              <span class="min-w-0 truncate">{{ directoryTreeRoot.name }}</span>
            </button>
            <span
              v-else
              class="min-w-0 flex-1 truncate text-xs font-medium"
              :title="directoryTreeRoot.path"
              @contextmenu.prevent.stop="openPathOnlyContextMenu(directoryTreeRoot.path, $event)"
            >{{ directoryTreeRoot.name }}</span>
            <button
              v-if="!atProjectDirectoryRoot"
              type="button"
              class="btn btn-ghost btn-xs btn-square h-6 min-h-6 shrink-0"
              :title="t('fileReader.backToProjectDirectory')"
              @click="backToProjectDirectory"
            >
              <Home class="h-3.5 w-3.5" />
            </button>
          </div>
          <div class="px-2 pt-2">
            <label class="input input-bordered input-sm flex items-center gap-2">
              <Search class="h-4 w-4 shrink-0 opacity-60" />
              <input
                v-model="directoryTreeFilter"
                class="min-w-0 flex-1"
                type="search"
                :placeholder="t('fileReader.filterFiles')"
              />
            </label>
          </div>
          <OverlayScrollArea ref="directoryAreaRef" class="min-h-0 flex-1" scroller-class="file-reader-scroll-container min-h-0 h-full py-1 text-sm">
            <div v-if="directoryTreeRoot.loading" class="flex items-center gap-2 px-3 py-2 text-xs opacity-65">
              <span class="loading loading-spinner loading-xs"></span>
              {{ t('fileReader.loadingDirectory') }}
            </div>
            <div v-else-if="directoryTreeRoot.error" class="px-3 py-2 text-xs text-error">
              {{ directoryTreeRoot.error }}
          </div>
          <div v-else-if="visibleTreeRows.length === 0" class="px-3 py-2 text-xs opacity-60">
            {{ directoryTreeFilter.trim() ? t('fileReader.noMatches') : t('fileReader.emptyDirectory') }}
          </div>
          <FileTreeMenu
            v-else
            :entries="directoryTreeRoot.entries"
            :node-for="treeDirectoryNode"
            :expanded="isTreeDirectoryExpanded"
            :icon-for="fileTreeIconFor"
            :on-toggle-directory="onTreeDirectoryToggle"
            :on-open-entry="handleTreeEntryOpen"
            :on-context-menu="openPathOnlyContextMenu"
            :active-path="activePath"
            :force-open="!!directoryTreeFilter.trim()"
            :filter="directoryTreeFilter"
            :loading-text="t('fileReader.loadingDirectory')"
          />
        </OverlayScrollArea>
            </div>
            <div v-else key="git" class="flex min-h-0 flex-1 flex-col overflow-hidden">
              <GitPanel
                ref="gitPanelRef"
                :workspace-path="gitPanelWorkspacePath"
                :markdown-is-dark="markdownIsDark"
                :session-key="props.sessionKey"
                :sync-workspace-branch="props.syncWorkspaceBranch"
                @open-diff="openGitDiffTab"
              />
            </div>
          </Transition>
        </aside>
      </Transition>
      <main v-if="!directoryOnly" v-show="!directoryTreeFullscreen" class="flex min-h-0 flex-1 flex-col overflow-hidden bg-base-100">
        <div
          v-if="activeTab"
          class="relative flex h-8 shrink-0 items-center gap-2 border-b border-base-300 bg-base-100 px-3 text-sm text-base-content/60"
        >
          <div class="relative min-w-0 flex-1">
            <div
              ref="addressScroller"
              class="file-reader-address-scroll flex min-w-0 items-center gap-1 overflow-x-auto overflow-y-hidden"
              @scroll="updateAddressScrollState"
              @wheel="handleAddressWheel"
              @contextmenu.prevent.stop="openAddressContextMenu"
            >
              <template v-for="segment in activePathSegments" :key="segment.key">
                <span v-if="segment.index > 0" class="shrink-0 text-base-content/35">›</span>
                <button
                  type="button"
                  class="inline-flex shrink-0 items-center rounded px-1.5 py-1 hover:bg-base-200 hover:text-base-content"
                  :title="t('fileReader.previewDirectory', { path: segment.path })"
                  @click="showHoverDirectoryTree(segment.path, $event)"
                >
                  {{ segment.label }}
                </button>
              </template>
              <span v-if="activePathSegments.length > 0" class="shrink-0 text-base-content/35">›</span>
              <span
                class="inline-flex shrink-0 items-center rounded px-1.5 py-1 font-medium text-base-content/80"
                :title="activeTab.diffSource?.path || activeTab.path"
              >
                {{ activeTab.title }}
              </span>
            </div>
            <div
              v-if="addressScrollState.scrollable"
              class="pointer-events-none absolute inset-x-0 bottom-0 h-px"
            >
              <div
                class="file-reader-address-scrollbar-thumb h-px rounded-full bg-primary/55"
                :style="addressScrollbarThumbStyle"
              ></div>
            </div>
          </div>
          <button
            v-if="activeTab?.kind === 'diff' && activeTab.diffSource?.path && !isMultiFileDiff"
            class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0"
            type="button"
            title="打开源文件"
            @click.stop="openDiffTargetFile"
          >
            <FileText class="h-4 w-4" />
          </button>
          <button
            class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0"
            type="button"
            :disabled="!activeTab"
            :title="t('fileReader.openWithDefault')"
            @click.stop="openWithDefaultProgram"
          >
            <ExternalLink class="h-4 w-4" />
          </button>
          <button class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0" type="button" :disabled="!activeTab" :title="t('fileReader.refresh')" @click.stop="refreshActiveTab">
            <RefreshCw class="h-4 w-4" />
          </button>
          <button
            v-if="activeTab && canToggleRawMode(activeTab)"
            class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0"
            type="button"
            :title="isTabRawMode(activeTab) ? t('fileReader.switchToRendered') : t('fileReader.switchToRaw')"
            :aria-label="isTabRawMode(activeTab) ? t('fileReader.currentRawView') : t('fileReader.currentRenderedView')"
            @click.stop="toggleActiveRawMode"
          >
            <Code2 v-if="isTabRawMode(activeTab)" class="h-4 w-4" />
            <Eye v-else class="h-4 w-4" />
          </button>
        </div>
        <div class="relative min-h-0 flex-1">
          <div
            ref="contentScroller"
            class="file-reader-content-stage h-full min-h-0 overflow-hidden"
          >
            <div v-if="!activeTab" class="flex h-full items-center justify-center text-sm text-base-content/55">
              <slot name="empty">
                <span>{{ t('fileReader.noFileOpen') }}</span>
              </slot>
            </div>
            <div v-else-if="activeTab.loading" class="flex h-full items-center justify-center gap-3 text-sm text-base-content/65">
              <span class="loading loading-spinner loading-sm"></span>
              {{ t('fileReader.loadingFile') }}
            </div>
            <div v-else-if="activeTab.error" class="m-4 rounded-box border border-error/30 bg-error/10 p-4 text-sm text-error">
              <div>{{ activeTab.error }}</div>
              <div v-if="localFileSystemAvailable" class="mt-3 flex gap-2">
                <button
                  type="button"
                  class="btn btn-sm"
                  :title="directoryFromPath(activeTab.path)"
                  @click.stop="openDirectoryInFileManager(directoryFromPath(activeTab.path))"
                >
                  <Folders class="h-3.5 w-3.5" />
                  {{ t('fileReader.openContainingFolder') }}
                </button>
              </div>
            </div>
            <div
              v-else-if="isPreviewMediaTab(activeTab) && !activeMediaSourceUrl"
              class="flex h-full items-center justify-center px-6 text-center text-sm text-base-content/60"
            >
              {{ t('fileReader.localMediaUnavailable') }}
            </div>
            <div
              v-else-if="activeTab.kind === 'image'"
              class="file-reader-media-stage h-full overflow-auto"
              @contextmenu.prevent="openPathOnlyContextMenu(activeTab.path, $event)"
            >
              <img
                class="file-reader-media-image"
                :src="activeMediaSourceUrl"
                :alt="activeTab.title"
                @error="handleMediaLoadError(activeTab)"
              />
            </div>
            <div
              v-else-if="activeTab.kind === 'audio'"
              class="flex h-full items-center justify-center px-6"
              @contextmenu.prevent="openPathOnlyContextMenu(activeTab.path, $event)"
            >
              <audio
                class="w-full max-w-3xl"
                :src="activeMediaSourceUrl"
                controls
                preload="metadata"
                @error="handleMediaLoadError(activeTab)"
              ></audio>
            </div>
            <div
              v-else-if="activeTab.kind === 'video'"
              class="file-reader-media-stage h-full bg-base-200/35"
              @contextmenu.prevent="openPathOnlyContextMenu(activeTab.path, $event)"
            >
              <video
                class="file-reader-media-video"
                :src="activeMediaSourceUrl"
                controls
                preload="metadata"
                @error="handleMediaLoadError(activeTab)"
              ></video>
            </div>
            <div
              v-else-if="activeTab.kind === 'unsupported'"
              class="flex h-full items-center justify-center px-6 text-center"
              @contextmenu.prevent="openPathOnlyContextMenu(activeTab.path, $event)"
            >
              <div class="max-w-md text-sm text-base-content/65">
                <img :src="resolvePathIcon(activeTab.path)" alt="" class="file-reader-tree-icon mx-auto mb-3 h-8 w-8 object-contain opacity-80" />
                <div class="font-medium text-base-content">{{ t('fileReader.unsupportedPreviewTitle') }}</div>
                <div class="mt-1">{{ t('fileReader.unsupportedPreviewDescription') }}</div>
              </div>
            </div>
            <div
              v-else-if="activeTab.kind === 'markdown' && !isTabRawMode(activeTab)"
              ref="markdownScroller"
              class="file-reader-content file-reader-markdown-scroller mx-auto h-full w-full max-w-300 overflow-auto px-4 py-4"
              @scroll="handleContentScroll"
              @click="openMarkdownFileLink"
              @mouseup="captureCurrentTextSelection"
              @keyup="captureCurrentTextSelection"
              @contextmenu.prevent="openActiveFileContextMenu"
            >
              <AppMarkdownRenderer
                class="ecall-markdown-content max-w-none"
                :text="activeMarkdownSource"
                :is-dark="markdownIsDark"
                variant="document"
                :local-image-base-path="directoryFromPath(activeTab.path)"
                @open-image-preview="openMarkdownImagePreview"
              />
            </div>
            <MultiFileDiffView
              v-else-if="activeTab.kind === 'diff' && isMultiFileDiff"
              class="h-full min-h-0"
              :diff-text="activeTab.content"
              :is-dark="markdownIsDark"
              :context-lines="Math.max(1, activeTab.diffSource?.context || 3)"
              @expand-file-gap="expandFileDiffContext"
              @open-file="openMultiFileTarget"
            />
            <ToolReviewCodePreview
              v-else-if="activeTab.kind === 'diff'"
              :code="activeTab.content"
              mode="patch"
              :is-dark="markdownIsDark"
              :show-line-numbers="true"
              :context-lines="Math.max(1, activeTab.diffSource?.context || 3)"
              @expand-gap="expandGitDiffContext"
            />
            <div
              v-else-if="activeTab.virtualized"
              class="relative h-full min-h-0"
              @mouseenter="!isTabRawMode(activeTab) && virtualCodeScrollbarRef?.reveal()"
              @mouseleave="!isTabRawMode(activeTab) && virtualCodeScrollbarRef?.hide()"
            >
              <div
                ref="virtualCodeScroller"
                class="file-reader-code-virtual-scroller h-full min-h-0 overflow-auto"
                :class="[
                  isTabRawMode(activeTab) ? 'file-reader-code-virtual-scroller-raw' : 'file-reader-code-virtual-scroller-shiki',
                  fileReaderLineWrapEnabled ? 'file-reader-code-virtual-scroller-wrap' : 'file-reader-code-virtual-scroller-nowrap',
                ]"
                @scroll="handleContentScroll"
                @mouseup="captureCurrentTextSelection"
                @keyup="captureCurrentTextSelection"
                @contextmenu.prevent="openActiveFileContextMenu"
              >
                <Virtualizer
                  v-if="virtualCodeScroller"
                  :data="activeVirtualCodeBlocks"
                  :scroll-ref="virtualCodeScroller"
                  :item-size="(activeTab?.blockLineCount || 120) * FILE_READER_VIRTUAL_BLOCK_LINE_HEIGHT_PX"
                  class="file-reader-code-virtual-canvas min-w-0 w-full"
                >
                  <template #default="{ item: block }">
                    <div
                      :key="block.key"
                      :data-file-block-key="block.key"
                      :data-start-line="block.startLine"
                      :data-end-line="block.endLine"
                      class="file-reader-code-virtual-row"
                      :style="{ '--file-reader-code-gutter-ch': String(virtualCodeLineNumberDigits) }"
                    >
                      <div
                        class="file-reader-code-virtual-block"
                        :class="isTabRawMode(activeTab) ? 'file-reader-code-virtual-block-raw' : 'file-reader-code-virtual-block-shiki'"
                        :style="{ '--file-reader-code-gutter-ch': String(virtualCodeLineNumberDigits) }"
                      >
                        <div
                          v-for="(lineHtml, lineIndex) in getVirtualBlockLines(block)"
                          :key="`${block.key}-${lineIndex}`"
                          :data-line-number="block.startLine + lineIndex"
                          class="file-reader-code-virtual-line"
                        >
                          <div
                            aria-hidden="true"
                            class="file-reader-code-virtual-line-number"
                            :class="isTabRawMode(activeTab) ? 'file-reader-code-virtual-gutter-raw' : 'file-reader-code-virtual-gutter-shiki'"
                          >
                            {{ block.startLine + lineIndex }}
                          </div>
                          <div
                            class="file-reader-code-virtual-line-content"
                            v-html="lineHtml"
                          ></div>
                        </div>
                      </div>
                    </div>
                  </template>
                </Virtualizer>
              </div>
              <FloatingScrollbar
                v-if="!isTabRawMode(activeTab)"
                ref="virtualCodeScrollbarRef"
                :target="virtualCodeScroller"
                variant="code-dark"
              />
              <FloatingScrollbar
                v-if="!isTabRawMode(activeTab) && !fileReaderLineWrapEnabled"
                :target="virtualCodeScroller"
                variant="code-dark"
                orientation="horizontal"
              />
            </div>
            <div
              v-else
              ref="plainTextScroller"
              class="file-reader-content h-full overflow-auto"
              @scroll="handleContentScroll"
              @mouseup="captureCurrentTextSelection"
              @keyup="captureCurrentTextSelection"
              @contextmenu.prevent="openActiveFileContextMenu"
            >
              <pre :class="['file-reader-raw-pre', 'min-h-full', 'p-4', activeTab.kind === 'code' && fileReaderLineWrapEnabled ? 'file-reader-code-wrap' : '']">{{ activeTab.content }}</pre>
            </div>
          </div>
        </div>
      </main>
      <main v-else-if="!directoryTreeRoot" class="flex min-h-0 flex-1 items-center justify-center bg-base-100 px-4 text-center text-sm text-base-content/55">
        <slot name="empty">
          <span>{{ t('fileReader.noWorkspace') }}</span>
        </slot>
      </main>
      <div
        v-if="directoryTreeRoot && !directoryOnly && !directoryTreeFullscreen"
        class="file-reader-resize-handle absolute bottom-0 top-0 z-10"
        :class="activeDirectoryTreeResize ? 'is-active' : ''"
        :style="{ right: `${effectiveDirectoryTreeWidth - 4}px` }"
        role="separator"
        aria-orientation="vertical"
        :aria-valuemin="FILE_READER_DIRECTORY_TREE_MIN_WIDTH"
        :aria-valuemax="FILE_READER_DIRECTORY_TREE_MAX_WIDTH"
        :aria-valuenow="effectiveDirectoryTreeWidth"
        tabindex="0"
        @pointerdown="startDirectoryTreeResize"
        @keydown.left.prevent="adjustDirectoryTreeWidthByKeyboard(-16)"
        @keydown.right.prevent="adjustDirectoryTreeWidthByKeyboard(16)"
      ></div>
    </div>

    <div
      v-if="fileDragActive"
      class="pointer-events-none fixed inset-0 z-40 flex items-center justify-center bg-base-100/70 backdrop-blur-[1px]"
    >
      <div class="rounded-box border border-primary/30 bg-base-100 px-5 py-3 text-sm font-medium text-primary shadow-lg">
        {{ t('fileReader.dropToOpen') }}
      </div>
    </div>

    <div v-if="actionErrorMessage" class="toast toast-end toast-bottom z-50">
      <div class="alert alert-error max-w-xl text-sm shadow-lg">
        <span>{{ actionErrorMessage }}</span>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="selectionAction"
        class="fixed z-1290 flex items-center gap-1 rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
        :style="{ left: `${selectionAction.x}px`, top: `${selectionAction.y}px` }"
        @pointerdown.stop
        @contextmenu.prevent.stop
      >
        <button
          type="button"
          class="btn btn-ghost btn-xs h-7 min-h-7 px-2"
          @mousedown.prevent
          @click.stop="copySelectionActionText"
        >
          <Copy class="h-3.5 w-3.5" />
          {{ t('common.copy') }}
        </button>
        <button
          type="button"
          class="btn btn-primary btn-xs h-7 min-h-7 px-2"
          @mousedown.prevent
          @click.stop="addSelectionActionToChat"
        >
          <MessageSquarePlus class="h-3.5 w-3.5" />
          {{ t('fileReader.addToChat') }}
        </button>
      </div>
      <template v-if="hoverDirectoryTreeVisible">
        <div
          class="fixed z-1199"
          :style="hoverDirectoryBridgeStyle"
          @mouseenter="cancelHideHoverDirectoryTree"
          @mouseleave="hideHoverDirectoryTree"
        ></div>
        <div
          ref="hoverDirectoryTreeRef"
          class="fixed z-1200 flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-xl"
          :style="hoverDirectoryTreeStyle"
          @pointerdown.stop
          @mouseenter="cancelHideHoverDirectoryTree"
          @mouseleave="hideHoverDirectoryTree"
        >
          <OverlayScrollArea
            class="min-h-0 flex-1"
            scroller-class="h-full py-1 text-sm"
          >
            <div v-if="hoverDirectoryTreeRoot?.loading" class="flex items-center gap-2 px-3 py-2 text-xs opacity-65">
              <span class="loading loading-spinner loading-xs"></span>
              {{ t('fileReader.loadingDirectory') }}
            </div>
            <div v-else-if="hoverDirectoryTreeRoot?.error" class="px-3 py-2 text-xs text-error">
              {{ hoverDirectoryTreeRoot.error }}
            </div>
            <div v-else-if="hoverDirectoryTreeRoot && hoverDirectoryTreeRoot.entries.length === 0" class="px-3 py-2 text-xs opacity-60">
              {{ t('fileReader.emptyDirectory') }}
            </div>
            <FileTreeMenu
              v-else-if="hoverDirectoryTreeRoot"
              :entries="hoverDirectoryTreeRoot.entries"
              :node-for="hoverDirectoryNodeFor"
              :expanded="isHoverDirectoryExpanded"
              :icon-for="fileTreeIconFor"
              :on-toggle-directory="onHoverDirectoryToggle"
              :on-open-entry="openFileFromHoverTree"
              :on-context-menu="openPathOnlyContextMenu"
              :force-open="!!directoryTreeFilter.trim()"
              :filter="directoryTreeFilter"
              :loading-text="t('fileReader.loadingDirectory')"
            />
          </OverlayScrollArea>
        </div>
      </template>
      <div
        v-if="contextMenuOpen && contextMenuTarget"
        class="fixed z-1300 w-52 rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
        :style="{ left: `${contextMenuPosition.x}px`, top: `${contextMenuPosition.y}px` }"
        @pointerdown.stop
        @contextmenu.prevent.stop
      >
        <button
          v-if="contextMenuTarget.kind !== 'address'"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="copyContextMenuFilePath"
        >
          {{ t('fileReader.copyFilePath') }}
        </button>
        <button
          v-if="localFileSystemAvailable && contextMenuTarget.kind !== 'address'"
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="openContextMenuDirectory"
        >
          {{ t('fileReader.openContainingFolder') }}
        </button>
        <template v-if="contextMenuTarget.kind === 'file'">
          <button
            type="button"
            class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
            :disabled="!contextMenuTarget.selectedText"
            @click.stop="copyContextMenuSelectedText"
          >
            {{ t('fileReader.copySelectedText') }}
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
            :disabled="!contextMenuTarget.lineReference"
            @click.stop="copyContextMenuLineReference"
          >
            {{ t('fileReader.copySelectedLineReference') }}
          </button>
        </template>
        <template v-else-if="contextMenuTarget.kind === 'address'">
          <button
            type="button"
            class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
            @click.stop="openContextMenuShell"
          >
            {{ t('fileReader.openShellHere') }}
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
            @click.stop="openContextMenuDirectory"
          >
            {{ t('fileReader.openContainingFolder') }}
          </button>
        </template>
      </div>
    </Teleport>
    <ChatImagePreviewDialog
      :open="imagePreviewOpen"
      :data-url="imagePreviewDataUrl"
      :zoom="imagePreviewZoom"
      :min-zoom="IMAGE_PREVIEW_MIN_ZOOM"
      :max-zoom="IMAGE_PREVIEW_MAX_ZOOM"
      :offset-x="previewOffsetX"
      :offset-y="previewOffsetY"
      :dragging="previewDragging"
      :rotation="imagePreviewRotation"
      :local-path="imagePreviewLocalPath"
      :copy-status="imagePreviewCopyStatus"
      :save-status="imagePreviewSaveStatus"
      @close="closeImagePreview"
      @zoom-in="zoomInPreview"
      @zoom-out="zoomOutPreview"
      @reset="resetPreviewZoom"
      @wheel="onPreviewWheel"
      @pointer-down="onPreviewPointerDown"
      @pointer-move="onPreviewPointerMove"
      @pointer-up="onPreviewPointerUp"
      @rotate="rotatePreviewClockwise"
      @copy-image="handleCopyMarkdownImage"
      @save-image="handleSaveMarkdownImage"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ChevronDown, Check, Code2, Copy, Eye, ExternalLink, FilePlus, Files, FileText, Folders, GitBranch, Home, MessageSquarePlus, RefreshCw, Search, SquareTerminal } from "@lucide/vue";
import {
  copyTransportChatImageToClipboard,
  getTransportCapabilities,
  gitPanelDiff,
  gitPanelShow,
  invokeTauri,
  listTransportFileReaderDirectoryOpenTargets,
  listenCurrentTransportFileDrop,
  onTransportNotification,
  openTransportFileDialog,
  openTransportFileReaderDirectoryTarget,
  openTransportFileReaderWindow,
  openTransportFileWithDefaultProgram,
  openTransportLocalDirectory,
  readTransportChatImage,
  resolveLocalFileUrl,
  saveTransportChatImageAs,
  updateTransportFileReaderWatchTargets,
} from "../../../services/tauri-api";
import { AppMarkdownRenderer, initKatex } from "../../chat/markdown";
import ChatImagePreviewDialog from "../../chat/components/dialogs/ChatImagePreviewDialog.vue";
import { useChatImagePreview } from "../../chat/composables/use-chat-image-preview";
import { isAbsoluteLocalPath, isAssistantSpacePath, normalizeLocalLinkHref, parseLocalFileReference } from "../../chat/utils/local-link";
import { Virtualizer } from "virtua/vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import { useFileReaderAppearance } from "../../shell/composables/use-file-reader-appearance";
import { FILE_READER_VIRTUAL_BLOCK_LINE_HEIGHT_PX } from "../constants";
import PanelTabStrip from "../../shared/components/PanelTabStrip.vue";
import FileTreeMenu from "./FileTreeMenu.vue";
import GitPanel from "./GitPanel.vue";
import MultiFileDiffView from "../../chat/components/MultiFileDiffView.vue";
import ToolReviewCodePreview from "../../chat/components/ToolReviewCodePreview.vue";
import { isMultiFileDiffText, replaceFileSection } from "../utils/multiFileDiff";
import { useI18n } from "vue-i18n";
import type { IdeContextReferenceItem } from "../../../types/app";
import {
  buildFileReaderContextMeta,
  buildFileReaderContextReference,
  buildFileReaderSelectionContextReference,
  fileReaderLineReference,
  resolveFileReaderSelectionActionPosition,
  resolveFileReaderSelectedLineRange,
} from "../file-reader-context";
import { useFileReaderVirtualCode } from "../composables/use-file-reader-virtual-code";
import { resolveFileTreeIcon } from "../file-tree-icons";
import type {
  DirectoryNode,
  FileReaderDirectoryEntry,
  FileReaderDirectoryPayload,
  FileReaderFileBlockPayload,
  FileReaderFilePayload,
  FileReaderSessionState,
  FileReaderWatchEventPayload,
  FileReaderWatchTarget,
  FileTab,
  GitDiffTabSource,
  TreeRow,
} from "../types";
import {
  directoryFromPath,
  directoryPathChain,
  extensionFromPath,
  fileKindFromPath,
  formatLineSuffix,
  isPreviewMediaKind,
  isTextFileKind,
  normalizeDirectoryEntries,
  normalizePath,
  normalizeSelectedText,
  resolveVisibleLineRange,
  sameNormalizedPath,
  splitContentLines,
  stripMarkdownHtmlComments,
  titleFromPath,
} from "../utils";

const { t } = useI18n();

initKatex();

// ==================== Props ====================

const props = withDefaults(defineProps<{
  initialRootPath?: string;
  initialOpenPath?: string;
  showTabs?: boolean;
  showPickFileButton?: boolean;
  showTabLocalFileActions?: boolean;
  directoryOnly?: boolean;
  enableGlobalDrop?: boolean;
  markdownIsDark?: boolean;
  customMarkstreamId?: string;
  /** 宿主（外层右侧栏）判定为窄屏浮层态：目录树打开时占满宿主、内容区归零，由外层决定而非面板自测宽度 */
  narrowOverlay?: boolean;
  sessionKey?: string;
  legacySessionKey?: string;
  /** 透传给 Git 面板：面板内切换分支成功后按仓库目录同步会话的工作分支记录 */
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
}>(), {
  showTabs: true,
  showPickFileButton: true,
  showTabLocalFileActions: false,
  directoryOnly: false,
  enableGlobalDrop: true,
  markdownIsDark: false,
  customMarkstreamId: "file-reader-markstream",
  sessionKey: "",
  legacySessionKey: "",
  narrowOverlay: false,
});

const emit = defineEmits<{
  (e: "openPath", path: string): void;
  (e: "selectPath", path: string): void;
  (e: "captureContextReference", reference: IdeContextReferenceItem): void;
  (e: "addContextReference", reference: IdeContextReferenceItem): void;
  (e: "clearSelectionContextReference"): void;
  (e: "clearContextReferences", paths?: string[]): void;
}>();

type FileReaderContextMenuTarget = {
  kind: "file" | "path" | "address";
  path: string;
  selectedText: string;
  lineReference: string;
};

type FileReaderSelectionAction = {
  selectedText: string;
  reference: IdeContextReferenceItem;
  x: number;
  y: number;
};

// ==================== Constants ====================

const localFileSystemAvailable = getTransportCapabilities().localFileSystem;
const FILE_READER_DIRECTORY_TREE_MIN_WIDTH = 220;
const FILE_READER_DIRECTORY_TREE_MAX_WIDTH = 640;
const FILE_READER_DIRECTORY_TREE_DEFAULT_WIDTH = 320;
const FILE_READER_CONTENT_MIN_WIDTH = 320;
const FILE_READER_DIRECTORY_TREE_COLLAPSE_EDGE_RATIO = 0.08;
const FILE_READER_DIRECTORY_TREE_RESIZE_MOVE_THRESHOLD = 4;

// ==================== State ====================

type DirectoryOpenTargetOption = {
  kind: string;
  label: string;
  type: "shell" | "vscode" | "explorer";
  iconDataUrl?: string;
};

type DirectoryOpenTargetsResult = {
  options?: DirectoryOpenTargetOption[];
};

const FILE_READER_OPEN_TARGET_STORAGE_KEY = "easy-call.file-reader.directory-open-target.v1";

const tabs = ref<FileTab[]>([]);
const activePath = ref("");
const asideMode = ref<"files" | "git">("files");
const gitPanelRef = ref<InstanceType<typeof GitPanel> | null>(null);
/** 待应用的 git 标签页：面板未挂载时先记住（out-in 过渡下首次打开是延迟挂载的），挂载后应用 */
const pendingGitTab = ref<"commits" | "stashes" | "branches" | null>(null);

// flush: post —— GitPanel 的 onMounted 会先按存储恢复标签页，这里必须排在它之后才能覆盖
watch(gitPanelRef, (panel) => {
  if (!panel || !pendingGitTab.value) return;
  panel.setActiveTab(pendingGitTab.value);
  pendingGitTab.value = null;
}, { flush: "post" });
const actionErrorMessage = ref("");
const contextMenuOpen = ref(false);
const contextMenuPosition = ref({ x: 0, y: 0 });
const contextMenuTarget = ref<FileReaderContextMenuTarget | null>(null);
const selectionAction = ref<FileReaderSelectionAction | null>(null);
const directoryRootPath = ref("");
const directoryTreeFilter = ref("");
const directoryOpenTargetsLoading = ref(false);
const directoryOpenTargetOptions = ref<DirectoryOpenTargetOption[]>([]);
const selectedDirectoryOpenTargetKind = ref("explorer");
const directoryOpenTargetDropdownOpen = ref(false);
const directoryOpenTargetDropdownRef = ref<HTMLElement | null>(null);
const directoryNodes = ref<Record<string, DirectoryNode>>({});
const imagePreviewCopyStatus = ref<"idle" | "doing">("idle");
const imagePreviewSaveStatus = ref<"idle" | "doing">("idle");
const fileDragActive = ref(false);
const fileReaderLayoutRoot = ref<HTMLElement | null>(null);
const addressScroller = ref<HTMLElement | null>(null);
const contentScroller = ref<HTMLElement | null>(null);
const markdownScroller = ref<HTMLElement | null>(null);
const virtualCodeScroller = ref<HTMLElement | null>(null);
const plainTextScroller = ref<HTMLElement | null>(null);
const directoryAreaRef = ref<InstanceType<typeof OverlayScrollArea> | null>(null);
const virtualCodeScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
const addressScrollState = ref({ scrollable: false, left: 0, clientWidth: 0, scrollWidth: 0 });
const showTabs = computed(() => props.showTabs !== false && !props.directoryOnly);
const showPickFileButton = computed(() => props.showPickFileButton !== false && !props.directoryOnly);
const directoryOnly = computed(() => !!props.directoryOnly);
const fileReaderWatchSessionId = computed(() => String(props.sessionKey || props.customMarkstreamId || "file-reader").trim());
const directoryTreeWidth = ref(FILE_READER_DIRECTORY_TREE_DEFAULT_WIDTH);
const activeDirectoryTreeResize = ref(false);
const fileReaderLayoutWidth = ref(0);

const hoverDirectoryTreeVisible = ref(false);
const hoverDirectoryTreeRoot = ref<DirectoryNode | null>(null);
const hoverDirectoryTreeNodes = ref<Record<string, DirectoryNode>>({});
const hoverDirectoryTreeStyle = ref<Record<string, string>>({ left: "0px", top: "0px", width: "280px" });
const hoverDirectoryTreeRef = ref<HTMLElement | null>(null);
const hoverDirectoryTreeAnchor = ref("");
let hoverHideTimer: number | null = null;

let unlistenFileDrop: (() => void) | null = null;
let unlistenFileReaderWatch: (() => void) | null = null;
let restoringSessionId = 0;
let suppressSessionPersist = false;
let sessionRestoreResolve: (() => void) | null = null;
let sessionRestorePromise: Promise<void> | null = null;
let lastCapturedSelectionKey = "";
let lastCapturedVisibleRangeKey = "";
let visibleRangeCaptureTimer = 0;
let watchTargetUpdateTimer = 0;
let autoRefreshFileTimer = 0;
let autoRefreshDirectoryTimer = 0;
const pendingAutoRefreshDirectoryPaths = new Set<string>();
let directoryTreeResizeStartX = 0;
let directoryTreeResizeStartWidth = 0;
let directoryTreeResizePointerId: number | null = null;
let directoryTreeResizeHandle: HTMLElement | null = null;
let directoryTreeResizePreviousBodyCursor = "";
let directoryTreeResizePreviousBodyUserSelect = "";
let fileReaderLayoutResizeObserver: ResizeObserver | null = null;
let directoryTreeResizeMoved = false;
let directoryTreeResizeRaf = 0;
let pendingDirectoryTreeResizeWidth: number | null = null;

// ==================== Computed ====================

const activeTab = computed(() => tabs.value.find((tab) => tab.path === activePath.value) || tabs.value[0] || null);

const isMultiFileDiff = computed(() => {
  const tab = activeTab.value;
  if (!tab || tab.kind !== "diff") return false;
  return isMultiFileDiffText(String(tab.content || ""));
});

const activeMediaSourceUrl = computed(() => {
  const tab = activeTab.value;
  if (!tab || !isPreviewMediaKind(tab.kind)) return "";
  try {
    return resolveLocalFileUrl(tab.path);
  } catch {
    return "";
  }
});

const {
  activeVirtualCodeBlocks,
  virtualCodeLineNumberDigits,
  blockContentLines,
  clearFileBlockCaches,
  resetVirtualCodeCaches,
  migrateVirtualCodeCaches,
  collectVirtualizedVisibleContent,
  ensureVirtualCodeBlockLoaded,
} = useFileReaderVirtualCode({
  activeTab,
  markdownIsDark: computed(() => props.markdownIsDark),
  virtualCodeScroller,
  isRawMode: isTabRawMode,
  requestFileBlock: requestFileReaderFileBlock,
});

// virtua 自动通过 ResizeObserver 测量，换行或宽度变化无需手工触发
function remeasureVirtualCodeRows() {}

function getVirtualBlockLines(block: import("../types").VirtualCodeBlock) {
  void ensureVirtualCodeBlockLoaded(block);
  return blockContentLines(block.key, block.lineCount);
}
const {
  fileReaderLineWrapEnabled,
  toggleFileReaderLineWrapEnabled,
} = useFileReaderAppearance();
const {
  imagePreviewOpen,
  imagePreviewDataUrl,
  imagePreviewLocalPath,
  imagePreviewZoom,
  imagePreviewRotation,
  IMAGE_PREVIEW_MIN_ZOOM,
  IMAGE_PREVIEW_MAX_ZOOM,
  previewOffsetX,
  previewOffsetY,
  previewDragging,
  zoomInPreview,
  zoomOutPreview,
  resetPreviewZoom,
  rotatePreviewClockwise,
  onPreviewWheel,
  openImagePreview,
  closeImagePreview,
  onPreviewPointerDown,
  onPreviewPointerMove,
  onPreviewPointerUp,
} = useChatImagePreview();

const directoryTreeRoot = computed(() => {
  const rootPath = normalizePath(directoryRootPath.value);
  return rootPath ? directoryNodes.value[rootPath] || null : null;
});

// 当前目录树根是否就是项目根锚点：直接以当前会话工作区为准，无中间态
const atProjectDirectoryRoot = computed(() => {
  const rootPath = normalizePath(directoryRootPath.value);
  const projectPath = initialDirectoryPath.value;
  return !rootPath || !projectPath || sameNormalizedPath(rootPath, projectPath);
});

function backToProjectDirectory() {
  const projectPath = initialDirectoryPath.value;
  if (projectPath && !atProjectDirectoryRoot.value) {
    void openDirectoryTree(projectPath);
  }
}

// Git 面板跟随目录树根；声明在 directoryTreeRoot 之后避免 TDZ
const gitPanelWorkspacePath = computed(() => String(props.initialRootPath || directoryTreeRoot.value?.path || "").trim());

const isFilesPanelOpen = computed(() => !!directoryRootPath.value && asideMode.value === "files");
const isGitPanelOpen = computed(() => !!directoryRootPath.value && asideMode.value === "git");

const hoverDirectoryBridgeStyle = computed(() => {
  const style = hoverDirectoryTreeStyle.value;
  const width = parseInt(style.width || "280", 10);
  return {
    left: style.left,
    top: `${parseInt(style.top || "0", 10) - 4}px`,
    width: `${width}px`,
    height: "8px",
  };
});

function hoverDirectoryNodeFor(path: string) {
  return hoverDirectoryTreeNodes.value[normalizePath(path)] || null;
}

function isHoverDirectoryExpanded(path: string) {
  return !!hoverDirectoryTreeNodes.value[normalizePath(path)]?.expanded;
}

function isHoverDirectoryCollapsed(path: string) {
  const node = hoverDirectoryTreeNodes.value[normalizePath(path)];
  return node !== undefined && node.loaded && !node.loading && !node.expanded;
}

function resolvePathIcon(path: string) {
  return resolveFileTreeIcon(path, false, false);
}

const fileReaderPanelTabs = computed(() =>
  tabs.value.map((tab) => ({
    key: tab.path,
    label: tab.title,
    title: tab.path,
    iconSrc: tab.kind === "diff" ? undefined : resolvePathIcon(tab.path),
    icon: tab.kind === "diff" ? GitBranch : undefined,
    closeable: true,
  })),
);

const fileTabContextMenuItems = computed(() => {
  if (!props.showTabLocalFileActions || !localFileSystemAvailable) return [];
  return [
    { label: t('fileReader.openInDocumentBrowser'), onClick: openInDocumentBrowser },
    { label: t('fileReader.openContainingFolder'), onClick: openDirectoryInFileManager },
    { label: t('fileReader.openWithDefault'), onClick: openPathWithDefaultProgram },
  ];
});

/** 文件树条目图标：目录按展开态取开/合图标（主树与 hover 预览树共用） */
function fileTreeIconFor(entry: FileReaderDirectoryEntry, expanded: boolean) {
  return resolveFileTreeIcon(entry.path, entry.isDirectory, expanded);
}

const activeMarkdownSource = computed(() => {
  const tab = activeTab.value;
  if (!tab) return "";
  if (isTabRawMode(tab)) return "";
  return tab.kind === "markdown" ? stripMarkdownHtmlComments(tab.content) : "";
});

const addressScrollbarThumbStyle = computed(() => {
  const state = addressScrollState.value;
  if (!state.scrollable || state.clientWidth <= 0 || state.scrollWidth <= 0) {
    return { width: "0px", transform: "translateX(0)" };
  }
  const width = Math.max(20, Math.round((state.clientWidth / state.scrollWidth) * state.clientWidth));
  const maxLeft = Math.max(1, state.scrollWidth - state.clientWidth);
  const maxThumbLeft = Math.max(0, state.clientWidth - width);
  const left = Math.round((state.left / maxLeft) * maxThumbLeft);
  return { width: `${width}px`, transform: `translateX(${left}px)` };
});

const visibleTreeRows = computed<TreeRow[]>(() => {
  const root = directoryTreeRoot.value;
  if (!root || root.loading || root.error) return [];
  return flattenDirectoryEntries(root.entries, 0, directoryTreeFilter.value);
});

// 目录树「占满态」：由外层传入的窄屏浮层判定（narrowOverlay）驱动，面板不再自测宽度断手机。
// 占满 = 树 w-full、内容区归零，纯 flex 让位，不叠浮层遮罩。
const directoryTreeFullscreen = computed(
  () => !directoryOnly.value && props.narrowOverlay === true && !!directoryTreeRoot.value,
);
const effectiveDirectoryTreeWidth = computed(() => {
  if (directoryTreeFullscreen.value) return fileReaderLayoutWidth.value;
  return clampDirectoryTreeWidth(directoryTreeWidth.value);
});

const activePathSegments = computed(() => {
  const tab = activeTab.value;
  if (!tab) return [];
  // diff tab 的 path 是合成的 git-diff:xxx，不对应真实磁盘路径，路径栏只显示相对文件名
  if (tab.kind === "diff") return [];
  const normalized = normalizePath(tab.path);
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length <= 1) return [];
  const dirs = parts.slice(0, -1);
  return dirs.map((label, index) => {
    const head = dirs[0]?.endsWith(":") ? `${dirs[0]}/` : dirs[0] || "";
    const path = index === 0 ? head : [head.replace(/\/$/, ""), ...dirs.slice(1, index + 1)].join("/");
    return { key: `${index}:${label}`, index, label, path };
  });
});

const activeDirectoryPath = computed(() => activePathSegments.value[activePathSegments.value.length - 1]?.path || "");
const initialDirectoryPath = computed(() => normalizePath(props.initialRootPath || ""));
const directoryToggleTargetPath = computed(() => {
  const currentRoot = normalizePath(directoryRootPath.value);
  if (currentRoot) return currentRoot;
  if (initialDirectoryPath.value) return initialDirectoryPath.value;
  return props.directoryOnly ? activeDirectoryPath.value : "";
});

// ==================== Watchers ====================

watch(() => props.initialOpenPath, (path) => {
  if (path) {
    void openPath(path);
  }
}, { immediate: true });

watch([() => props.sessionKey, () => props.initialRootPath], ([nextKey, nextRootPath], [prevKey, prevRootPath]) => {
  const prevSessionKey = String(prevKey || "").trim();
  const nextSessionKey = String(nextKey || "").trim();
  // 会话切换：持久化旧会话并完整恢复新会话（带新工作区兜底）
  if (nextSessionKey !== prevSessionKey) {
    if (prevSessionKey) {
      persistFileReaderSession(prevSessionKey);
    }
    if (nextSessionKey) {
      void restoreFileReaderSession(nextSessionKey, nextRootPath);
      return;
    }
    void restoreFileReaderSession("", nextRootPath);
    return;
  }
  // 同会话仅工作区变化不整量 restore：Home 直接跟随 initialDirectoryPath 计算，无需额外同步
  void prevRootPath;
}, { immediate: true });

watch(
  [tabs, activePath, directoryRootPath, asideMode],
  () => {
    persistFileReaderSession();
    scheduleFileReaderWatchTargetUpdate();
  },
  { deep: true },
);

watch(visibleTreeRows, () => scheduleFileReaderWatchTargetUpdate());

// ==================== Address Scroll ====================

function updateAddressScrollState() {
  const el = addressScroller.value;
  if (!el) {
    addressScrollState.value = { scrollable: false, left: 0, clientWidth: 0, scrollWidth: 0 };
    return;
  }
  addressScrollState.value = {
    scrollable: el.scrollWidth > el.clientWidth + 1,
    left: el.scrollLeft,
    clientWidth: el.clientWidth,
    scrollWidth: el.scrollWidth,
  };
}

function scheduleAddressScrollStateUpdate() {
  void nextTick(() => updateAddressScrollState());
}

function handleAddressWheel(event: WheelEvent) {
  const el = addressScroller.value;
  if (!el) return;
  event.preventDefault();
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  el.scrollLeft += delta;
  updateAddressScrollState();
}

function measureFileReaderLayoutWidth() {
  const el = fileReaderLayoutRoot.value;
  fileReaderLayoutWidth.value = el ? Math.round(el.getBoundingClientRect().width) : 0;
}

function clampDirectoryTreeWidth(width: number): number {
  const normalizedWidth = Math.round(Number(width) || FILE_READER_DIRECTORY_TREE_DEFAULT_WIDTH);
  const layoutWidth = fileReaderLayoutWidth.value;
  if (layoutWidth <= 0 || directoryOnly.value) {
    return Math.min(FILE_READER_DIRECTORY_TREE_MAX_WIDTH, Math.max(FILE_READER_DIRECTORY_TREE_MIN_WIDTH, normalizedWidth));
  }
  const layoutMax = Math.max(
    FILE_READER_DIRECTORY_TREE_MIN_WIDTH,
    layoutWidth - FILE_READER_CONTENT_MIN_WIDTH,
  );
  return Math.min(
    Math.min(FILE_READER_DIRECTORY_TREE_MAX_WIDTH, layoutMax),
    Math.max(FILE_READER_DIRECTORY_TREE_MIN_WIDTH, normalizedWidth),
  );
}

function setDirectoryTreeWidth(width: number) {
  directoryTreeWidth.value = clampDirectoryTreeWidth(width);
}

function startDirectoryTreeResize(event: PointerEvent) {
  if (event.button !== 0 || directoryOnly.value || directoryTreeFullscreen.value) return;
  event.preventDefault();
  activeDirectoryTreeResize.value = true;
  directoryTreeResizeStartX = event.clientX;
  directoryTreeResizeStartWidth = effectiveDirectoryTreeWidth.value;
  directoryTreeResizeMoved = false;
  pendingDirectoryTreeResizeWidth = null;
  if (directoryTreeResizeRaf) {
    window.cancelAnimationFrame(directoryTreeResizeRaf);
    directoryTreeResizeRaf = 0;
  }
  directoryTreeResizePointerId = Number.isFinite(event.pointerId) ? event.pointerId : null;
  directoryTreeResizeHandle = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  directoryTreeResizeHandle?.setPointerCapture?.(event.pointerId);
  directoryTreeResizePreviousBodyCursor = document.body.style.cursor;
  directoryTreeResizePreviousBodyUserSelect = document.body.style.userSelect;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  window.addEventListener("pointermove", handleDirectoryTreeResizeMove);
  window.addEventListener("pointerup", stopDirectoryTreeResize, { once: true });
  window.addEventListener("pointercancel", stopDirectoryTreeResize, { once: true });
}

function handleDirectoryTreeResizeMove(event: PointerEvent) {
  if (!activeDirectoryTreeResize.value) return;
  if (!directoryTreeResizeMoved && Math.abs(event.clientX - directoryTreeResizeStartX) >= FILE_READER_DIRECTORY_TREE_RESIZE_MOVE_THRESHOLD) {
    directoryTreeResizeMoved = true;
  }
  const delta = directoryOnly.value ? 0 : directoryTreeResizeStartX - event.clientX;
  const targetWidth = clampDirectoryTreeWidth(directoryTreeResizeStartWidth + delta);
  // rAF 节流：拖拽时每帧最多提交一次宽度，避免 pointermove 高频触发 Vue 响应式 + Virtualizer 全量 getBoundingClientRect
  pendingDirectoryTreeResizeWidth = targetWidth;
  if (directoryTreeResizeRaf) return;
  directoryTreeResizeRaf = window.requestAnimationFrame(() => {
    directoryTreeResizeRaf = 0;
    if (pendingDirectoryTreeResizeWidth === null) return;
    const nextWidth = pendingDirectoryTreeResizeWidth;
    pendingDirectoryTreeResizeWidth = null;
    directoryTreeWidth.value = nextWidth;
  });
}

function shouldCollapseDirectoryTreeFromClientX(clientX: number): boolean {
  const root = fileReaderLayoutRoot.value;
  if (!root || directoryOnly.value) return false;
  const rect = root.getBoundingClientRect();
  if (!(rect.width > 0)) return false;
  const threshold = Math.max(24, rect.width * FILE_READER_DIRECTORY_TREE_COLLAPSE_EDGE_RATIO);
  return clientX >= rect.right - threshold;
}

function stopDirectoryTreeResize(event?: PointerEvent) {
  const shouldCollapse = !!event
    && event.type === "pointerup"
    && directoryTreeResizeMoved
    && shouldCollapseDirectoryTreeFromClientX(event.clientX);
  const restoreWidthOnCollapse = clampDirectoryTreeWidth(directoryTreeResizeStartWidth);
  window.removeEventListener("pointermove", handleDirectoryTreeResizeMove);
  window.removeEventListener("pointerup", stopDirectoryTreeResize);
  window.removeEventListener("pointercancel", stopDirectoryTreeResize);
  document.body.style.cursor = directoryTreeResizePreviousBodyCursor;
  document.body.style.userSelect = directoryTreeResizePreviousBodyUserSelect;
  if (directoryTreeResizeHandle && directoryTreeResizePointerId !== null && directoryTreeResizeHandle.hasPointerCapture?.(directoryTreeResizePointerId)) {
    directoryTreeResizeHandle.releasePointerCapture(directoryTreeResizePointerId);
  }
  directoryTreeResizeHandle = null;
  directoryTreeResizePointerId = null;
  // 拖拽结束：刷掉 rAF 队列中未提交的宽度，避免丢最后一帧
  if (directoryTreeResizeRaf) {
    window.cancelAnimationFrame(directoryTreeResizeRaf);
    directoryTreeResizeRaf = 0;
  }
  if (pendingDirectoryTreeResizeWidth !== null) {
    directoryTreeWidth.value = pendingDirectoryTreeResizeWidth;
    pendingDirectoryTreeResizeWidth = null;
  }
  activeDirectoryTreeResize.value = false;
  if (shouldCollapse) {
    directoryTreeWidth.value = restoreWidthOnCollapse;
    closeDirectoryTree();
  } else {
    persistFileReaderSession();
    // 拖拽期间暂停了 Virtualizer 测量（见 handleMeasureVirtualCodeRow），仅在换行模式下高度会随宽度变化，结束后再补量一次
    if (fileReaderLineWrapEnabled.value) {
      void nextTick(() => remeasureVirtualCodeRows());
    }
  }
  directoryTreeResizeMoved = false;
}

function adjustDirectoryTreeWidthByKeyboard(delta: number) {
  setDirectoryTreeWidth(effectiveDirectoryTreeWidth.value + delta);
}

// ==================== Auto Refresh Watch ====================

function scheduleFileReaderWatchTargetUpdate() {
  if (watchTargetUpdateTimer) window.clearTimeout(watchTargetUpdateTimer);
  watchTargetUpdateTimer = window.setTimeout(() => {
    watchTargetUpdateTimer = 0;
    void updateFileReaderWatchTargets();
  }, 250);
}

async function updateFileReaderWatchTargets() {
  const sessionId = fileReaderWatchSessionId.value;
  if (!sessionId) return;
  const targets = collectFileReaderWatchTargets();
  try {
    await updateTransportFileReaderWatchTargets({ sessionId, targets });
  } catch (error) {
    console.warn("[文件阅读器] 更新自动刷新监听目标失败", error);
  }
}

function collectFileReaderWatchTargets(): FileReaderWatchTarget[] {
  const targets: FileReaderWatchTarget[] = [];
  const active = activeTab.value;
  if (active?.loaded && !active.loading && !active.error && active.path) {
    const normalizedActive = normalizePath(active.path);
    if (normalizedActive && !isGitInternalPath(normalizedActive)) {
      targets.push({ path: normalizedActive, kind: "file" });
    }
  }
  const root = directoryTreeRoot.value;
  if (root?.path) {
    const normalizedRoot = normalizePath(root.path);
    if (normalizedRoot && !isGitInternalPath(normalizedRoot)) {
      targets.push({ path: normalizedRoot, kind: "directory" });
    }
  }
  for (const node of Object.values(directoryNodes.value)) {
    if (!node?.expanded) continue;
    const normalized = normalizePath(node.path);
    if (!normalized || isGitInternalPath(normalized)) continue;
    targets.push({ path: normalized, kind: "directory" });
  }
  const seen = new Set<string>();
  return targets.filter((target) => {
    const normalized = normalizePath(target.path);
    if (!normalized || isGitInternalPath(normalized)) return false;
    const key = `${target.kind}:${normalized.toLowerCase()}`;
    if (seen.has(key)) return false;
    seen.add(key);
    // 归一后回写，避免大小写/斜杠差异导致后端重复
    target.path = normalized;
    return true;
  });
}

async function startFileReaderWatchListener() {
  stopFileReaderWatchListener();
  try {
    unlistenFileReaderWatch = onTransportNotification<FileReaderWatchEventPayload>("fileReader.watchChanged", (payload) => {
      handleFileReaderWatchEvent(payload);
    });
  } catch (error) {
    console.warn("[文件阅读器] 监听自动刷新事件失败", error);
  }
}

function stopFileReaderWatchListener() {
  unlistenFileReaderWatch?.();
  unlistenFileReaderWatch = null;
}

function handleFileReaderWatchEvent(payload: FileReaderWatchEventPayload) {
  if (String(payload?.sessionId || "").trim() !== fileReaderWatchSessionId.value) return;
  const changedPath = normalizePath(payload?.path || "");
  if (!changedPath) return;
  const active = activeTab.value;
  if (active && sameNormalizedPath(changedPath, active.path)) {
    scheduleAutoRefreshActiveTab();
    return;
  }
  if (String(payload?.kind || "").trim() === "directory" && directoryTreeRoot.value && isPathRelevantToVisibleDirectory(changedPath)) {
    scheduleAutoRefreshDirectoryNode(changedPath);
  }
}

function scheduleAutoRefreshActiveTab() {
  if (autoRefreshFileTimer) window.clearTimeout(autoRefreshFileTimer);
  autoRefreshFileTimer = window.setTimeout(() => {
    autoRefreshFileTimer = 0;
    const active = activeTab.value;
    if (active?.path) void openPath(active.path);
  }, 350);
}

function isGitInternalPath(path: string): boolean {
  const normalized = normalizePath(path).replace(/\\/g, "/").toLowerCase();
  return normalized.endsWith("/.git") || normalized.includes("/.git/");
}

function scheduleAutoRefreshDirectoryNode(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  if (isGitInternalPath(normalizedPath)) return;
  pendingAutoRefreshDirectoryPaths.add(normalizedPath);
  if (autoRefreshDirectoryTimer) window.clearTimeout(autoRefreshDirectoryTimer);
  autoRefreshDirectoryTimer = window.setTimeout(() => {
    autoRefreshDirectoryTimer = 0;
    const paths = Array.from(pendingAutoRefreshDirectoryPaths);
    pendingAutoRefreshDirectoryPaths.clear();
    for (const directoryPath of paths) {
      if (isGitInternalPath(directoryPath)) continue;
      if (!isPathRelevantToVisibleDirectory(directoryPath)) continue;
      const node = treeDirectoryNode(directoryPath);
      const root = directoryTreeRoot.value;
      if (!node && !sameNormalizedPath(directoryPath, root?.path || "")) continue;
      void loadDirectory(directoryPath, node?.expanded ?? true);
    }
  }, 500);
}

function isPathRelevantToVisibleDirectory(path: string) {
  const normalizedPath = normalizePath(path);
  if (isGitInternalPath(normalizedPath)) return false;
  const root = directoryTreeRoot.value;
  if (root && sameNormalizedPath(normalizedPath, root.path)) return true;
  return visibleTreeRows.value.some((row) => row.kind === "entry" && sameNormalizedPath(normalizedPath, row.entry.path));
}

// ==================== Context Menu ====================

function contextMenuPositionFromEvent(event: MouseEvent, options: { width?: number; height?: number } = {}) {
  const width = options.width || 208;
  const height = options.height || 128;
  return {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8)),
  };
}

function openPathOnlyContextMenu(path: string, event: MouseEvent) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  contextMenuPosition.value = contextMenuPositionFromEvent(event, { height: localFileSystemAvailable ? 76 : 42 });
  contextMenuTarget.value = {
    kind: "path",
    path: normalizedPath,
    selectedText: "",
    lineReference: "",
  };
  contextMenuOpen.value = true;
}

function openAddressContextMenu(event: MouseEvent) {
  if (!localFileSystemAvailable) {
    closeContextMenu();
    return;
  }
  const tab = activeTab.value;
  const currentDirectory = normalizePath(tab ? directoryFromPath(tab.path) : "");
  if (!currentDirectory) return;
  contextMenuPosition.value = contextMenuPositionFromEvent(event, { height: 76 });
  contextMenuTarget.value = {
    kind: "address",
    path: currentDirectory,
    selectedText: "",
    lineReference: "",
  };
  contextMenuOpen.value = true;
}

function openActiveFileContextMenu(event: MouseEvent) {
  const tab = activeTab.value;
  if (!tab || tab.loading || tab.error) return;
  const selectionContext = readCurrentFileSelection();
  contextMenuPosition.value = contextMenuPositionFromEvent(event, { height: localFileSystemAvailable ? 160 : 128 });
  contextMenuTarget.value = {
    kind: "file",
    path: normalizePath(tab.path),
    selectedText: selectionContext?.selectedText || "",
    lineReference: selectionContext ? fileReaderLineReference(tab.path, selectionContext.lineRange) : "",
  };
  contextMenuOpen.value = true;
}

function closeContextMenu() {
  contextMenuOpen.value = false;
  contextMenuTarget.value = null;
}

function readCurrentFileSelection(): { selectedText: string; lineRange: { startLine: number; endLine: number } } | null {
  const tab = activeTab.value;
  const scroller = activeContentScroller();
  if (!tab || !scroller || tab.loading || tab.error) return null;
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  if (!scroller.contains(range.commonAncestorContainer)) return null;
  const selectedText = normalizeSelectedText(selection.toString());
  if (!selectedText) return null;
  return {
    selectedText,
    lineRange: resolveFileReaderSelectedLineRange(tab, scroller, selectedText, range),
  };
}

function selectionActionAnchor(range: Range) {
  const rects = Array.from(range.getClientRects()).filter((rect) => rect.width > 0 || rect.height > 0);
  const rect = rects[rects.length - 1] || range.getBoundingClientRect();
  return { x: rect.right, y: rect.bottom };
}

function closeSelectionAction() {
  selectionAction.value = null;
}

async function copyTextToClipboard(text: string) {
  const value = String(text || "");
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.actionCopy'), value, error);
  }
}

function copyContextMenuFilePath() {
  const target = contextMenuTarget.value;
  closeContextMenu();
  void copyTextToClipboard(target?.path || "");
}

function copyContextMenuSelectedText() {
  const target = contextMenuTarget.value;
  closeContextMenu();
  void copyTextToClipboard(target?.selectedText || "");
}

function copyContextMenuLineReference() {
  const target = contextMenuTarget.value;
  closeContextMenu();
  void copyTextToClipboard(target?.lineReference || "");
}

function copySelectionActionText() {
  const target = selectionAction.value;
  closeSelectionAction();
  void copyTextToClipboard(target?.selectedText || "");
}

function addSelectionActionToChat() {
  const target = selectionAction.value;
  closeSelectionAction();
  if (!target) return;
  emit("addContextReference", target.reference);
  emit("clearSelectionContextReference");
  lastCapturedSelectionKey = "";
  window.getSelection()?.removeAllRanges();
}

function openContextMenuShell() {
  const target = contextMenuTarget.value;
  closeContextMenu();
  void openDirectoryWithTarget(target?.path || "");
}

function openContextMenuDirectory() {
  const target = contextMenuTarget.value;
  closeContextMenu();
  void openDirectoryInFileManager(target?.path || "");
}

// ==================== Context Capture ====================

function captureCurrentTextSelection() {
  const tab = activeTab.value;
  const scroller = activeContentScroller();
  if (!tab || !isTextFileKind(tab.kind) || !scroller || tab.loading || tab.error) return;
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
    closeSelectionAction();
    lastCapturedSelectionKey = "";
    emit("clearSelectionContextReference");
    return;
  }
  const range = selection.getRangeAt(0);
  if (!scroller.contains(range.commonAncestorContainer)) {
    closeSelectionAction();
    lastCapturedSelectionKey = "";
    emit("clearSelectionContextReference");
    return;
  }

  const selectedText = normalizeSelectedText(selection.toString());
  if (!selectedText) {
    closeSelectionAction();
    return;
  }

  const lineRange = resolveFileReaderSelectedLineRange(tab, scroller, selectedText, range);
  const meta = buildFileReaderContextMeta(tab, props.initialRootPath || "");
  const capturedAt = new Date().toISOString();
  const reference = buildFileReaderSelectionContextReference({
    tab,
    initialRootPath: props.initialRootPath || "",
    lineRange,
    selectedText,
    capturedAt,
    t: (key, params) => String(t(key, params ?? {})),
  });
  const actionAnchor = selectionActionAnchor(range);
  selectionAction.value = {
    selectedText,
    reference,
    ...resolveFileReaderSelectionActionPosition({
      anchorX: actionAnchor.x,
      anchorY: actionAnchor.y,
      containerRect: scroller.getBoundingClientRect(),
    }),
  };
  const selectionKey = [
    meta.filePath,
    lineRange.startLine || "",
    lineRange.endLine || "",
    selectedText,
  ].join("\n");
  if (selectionKey === lastCapturedSelectionKey) return;
  lastCapturedSelectionKey = selectionKey;
  emit("captureContextReference", reference);
}

function handleContentScroll() {
  closeSelectionAction();
  captureVisibleRangeContext();
}

function captureVisibleRangeContext() {
  if (visibleRangeCaptureTimer) window.clearTimeout(visibleRangeCaptureTimer);
  visibleRangeCaptureTimer = window.setTimeout(() => {
    visibleRangeCaptureTimer = 0;
    captureVisibleRangeContextNow();
  }, 80);
}

function captureVisibleRangeContextNow(options: { force?: boolean } = {}) {
  const tab = activeTab.value;
  const scroller = activeContentScroller();
  if (!tab || !isTextFileKind(tab.kind) || !scroller || tab.loading || tab.error || !tab.loaded) return;
  const totalLines = tab.virtualized ? Math.max(1, tab.totalLines) : splitContentLines(tab.content).length;
  if (totalLines === 0) return;
  const lineRange = resolveVisibleLineRange(scroller, totalLines);
  const content = tab.virtualized
    ? collectVirtualizedVisibleContent(tab, lineRange)
    : splitContentLines(tab.content).slice(lineRange.startLine - 1, lineRange.endLine).join("\n").trim();
  if (!content) return;
  const meta = buildFileReaderContextMeta(tab, props.initialRootPath || "");
  const visibleRangeKey = [meta.filePath, lineRange.startLine, lineRange.endLine, content].join("\n");
  if (!options.force && visibleRangeKey === lastCapturedVisibleRangeKey) return;
  lastCapturedVisibleRangeKey = visibleRangeKey;
  const capturedAt = new Date().toISOString();
  emitContextReference({
    tab,
    source: "visible_range",
    lineRange,
    content: content.slice(0, 20_000),
    displayLabel: `${meta.relativePath || tab.title}${formatLineSuffix(lineRange.startLine, lineRange.endLine)}`,
    capturedAt,
  });
}

function emitContextReference(input: {
  tab: FileTab;
  source: "selection" | "visible_range";
  lineRange: { startLine?: number; endLine?: number };
  content: string;
  displayLabel: string;
  capturedAt: string;
}) {
  emit("captureContextReference", buildFileReaderContextReference({
    ...input,
    initialRootPath: props.initialRootPath || "",
    t: (key, params) => String(t(key, params ?? {})),
  }));
}

function activeContentScroller() {
  const tab = activeTab.value;
  if (!tab) return contentScroller.value;
  if (tab.virtualized) return virtualCodeScroller.value;
  if (tab.kind === "markdown" && !tab.rawMode) return markdownScroller.value;
  if (isTextFileKind(tab.kind)) return plainTextScroller.value;
  return contentScroller.value;
}

function isPreviewMediaTab(tab: FileTab | null | undefined) {
  return !!tab && isPreviewMediaKind(tab.kind);
}

function handleMediaLoadError(tab: FileTab) {
  const current = activeTab.value;
  if (!current || !sameNormalizedPath(current.path, tab.path)) return;
  tab.loaded = true;
  tab.loading = false;
  tab.error = t('fileReader.mediaLoadFailed');
  replaceTabState(tab);
}

async function resetActiveContentScrollToTop() {
  closeSelectionAction();
  lastCapturedSelectionKey = "";
  lastCapturedVisibleRangeKey = "";
  await nextTick();
  const scroller = activeContentScroller();
  if (!scroller) return;
  scroller.scrollTop = 0;
  scroller.scrollLeft = 0;
}

async function resetScrollAndCaptureFirstPage() {
  await resetActiveContentScrollToTop();
  captureVisibleRangeContextNow({ force: true });
}

function canToggleRawMode(tab: FileTab | null | undefined) {
  return !!tab && tab.kind === "markdown";
}

function isTabRawMode(tab: FileTab | null | undefined) {
  if (!tab) return false;
  if (tab.kind === "markdown") return tab.rawMode;
  return false;
}

// ==================== Helpers ====================

function readFileReaderSessionState(key = props.sessionKey): FileReaderSessionState {
  const storageKey = String(key || "").trim();
  if (!storageKey || typeof window === "undefined") return {};
  try {
    const legacyStorageKey = String(props.legacySessionKey || "").trim();
    return JSON.parse(window.localStorage.getItem(storageKey) || (legacyStorageKey ? window.localStorage.getItem(legacyStorageKey) : "") || "{}") as FileReaderSessionState;
  } catch {
    return {};
  }
}

function hasStoredFileReaderSession(key = props.sessionKey): boolean {
  const storageKey = String(key || "").trim();
  if (!storageKey || typeof window === "undefined") return false;
  try {
    if (window.localStorage.getItem(storageKey) !== null) return true;
    const legacyStorageKey = String(props.legacySessionKey || "").trim();
    if (legacyStorageKey && window.localStorage.getItem(legacyStorageKey) !== null) return true;
  } catch {
    return false;
  }
  return false;
}

function persistFileReaderSession(key = props.sessionKey) {
  if (suppressSessionPersist) return;
  const storageKey = String(key || "").trim();
  if (!storageKey || typeof window === "undefined") return;
  const uniqueTabs = Array.from(new Set(tabs.value.map((tab) => tab.path).filter((path) => {
    // diff 标签是伪路径（git-diff:...），不持久化；普通路径再做归一化
    return !String(path || "").startsWith("git-diff:");
  }))).map((path) => normalizePath(path));
  const state: FileReaderSessionState = {
    tabs: uniqueTabs,
    // diff 标签不持久化，activePath 若指向 git-diff 伪路径则不保存
    activePath: String(activePath.value || "").startsWith("git-diff:") ? "" : normalizePath(activePath.value),
    directoryRootPath: normalizePath(directoryRootPath.value),
    directoryTreeWidth: effectiveDirectoryTreeWidth.value,
    asideMode: asideMode.value,
  };
  window.localStorage.setItem(storageKey, JSON.stringify(state));
}

async function restoreFileReaderSession(key = props.sessionKey, fallbackRootPath = props.initialRootPath) {
  const storageKey = String(key || "").trim();
  const restoreId = ++restoringSessionId;
  suppressSessionPersist = true;
  sessionRestorePromise = new Promise<void>((resolve) => {
    sessionRestoreResolve = resolve;
  });
  try {
    tabs.value = [];
    activePath.value = "";
    resetVirtualCodeCaches();
    directoryRootPath.value = "";
    directoryTreeWidth.value = FILE_READER_DIRECTORY_TREE_DEFAULT_WIDTH;
    directoryTreeFilter.value = "";
    directoryNodes.value = {};

    const initialRoot = normalizePath(fallbackRootPath || "");
    if (!storageKey) {
      if (initialRoot) {
        directoryRootPath.value = initialRoot;
        await loadDirectory(initialRoot, true);
      }
      return;
    }
    const state = readFileReaderSessionState(storageKey);
    if (restoreId !== restoringSessionId) return;

    // 恢复会话级左侧栏模式（文件 / git），非法值忽略保持默认
    if (state.asideMode === "files" || state.asideMode === "git") {
      asideMode.value = state.asideMode;
    }

    const restoredTabs = Array.from(new Set((state.tabs || []).filter((path) => Boolean(path) && !String(path).startsWith("git-diff:")).map((path) => normalizePath(path))));
    tabs.value = restoredTabs.map((path) => createRestoredTab(path));
    // 会话里保存的 activePath 不可能是 git-diff 伪路径（持久化时已跳过），防御性过滤
    const restoredActiveRaw = String(state.activePath || "");
    const restoredActivePath = restoredActiveRaw.startsWith("git-diff:") ? "" : normalizePath(restoredActiveRaw);
    activePath.value = restoredActivePath && restoredTabs.includes(restoredActivePath) ? restoredActivePath : restoredTabs[0] || "";
    setDirectoryTreeWidth(state.directoryTreeWidth || FILE_READER_DIRECTORY_TREE_DEFAULT_WIDTH);

    const restoredDirectoryRoot = normalizePath(state.directoryRootPath || "");

    await nextTick();
    if (restoreId !== restoringSessionId) return;
    suppressSessionPersist = false;

    if (activePath.value) {
      if (restoredDirectoryRoot) {
        directoryRootPath.value = restoredDirectoryRoot;
        await loadDirectory(restoredDirectoryRoot, true);
      }
      if (restoreId !== restoringSessionId) return;
      await openPath(activePath.value);
      return;
    }

    // 没有打开任何文件：自动展开目录（优先上次目录，否则工作区根）
    // 修复：切换会话后"侧边栏是否打开"丢失 —— 之前无论是否存过会话，无文件时都会 fallback 到 initialRoot，导致"已关闭"被改写为"已打开"
    // 现在区分"首次打开该会话（无存储）"与"已存储且显式关闭（directoryRootPath==''）"，后者保持关闭
    const hasStoredSession = hasStoredFileReaderSession(storageKey);
    const hasStoredDirectoryPreference = hasStoredSession && Object.prototype.hasOwnProperty.call(state, "directoryRootPath");
    const rootToOpen = restoredDirectoryRoot || (hasStoredDirectoryPreference ? "" : initialRoot);
    if (rootToOpen) {
      directoryRootPath.value = rootToOpen;
      await loadDirectory(rootToOpen, true);
    }
  } finally {
    if (restoreId === restoringSessionId) {
      suppressSessionPersist = false;
      scheduleAddressScrollStateUpdate();
    }
    sessionRestoreResolve?.();
    sessionRestoreResolve = null;
    sessionRestorePromise = null;
  }
}

/** 会话恢复进行中时返回其完成 Promise；无恢复进行时立即完成。 */
function whenSessionRestored(): Promise<void> {
  return sessionRestorePromise || Promise.resolve();
}

function createRestoredTab(path: string): FileTab {
  const normalizedPath = normalizePath(path);
  return {
    path: normalizedPath,
    title: titleFromPath(normalizedPath),
    extension: extensionFromPath(normalizedPath),
    kind: fileKindFromPath(normalizedPath),
    content: "",
    rawMode: false,
    forcePlain: false,
    virtualized: false,
    totalLines: 0,
    blockLineCount: 0,
    loaded: false,
    loading: false,
    error: "",
  };
}

function replaceTabState(tab: FileTab, matchPath = tab.path) {
  const normalizedMatchPath = normalizePath(matchPath);
  tabs.value = tabs.value.map((item) => item.path === normalizedMatchPath ? { ...tab } : item);
}

function upsertLoadingTab(path: string, reuseActiveTab = false) {
  const normalizedPath = normalizePath(path);
  const existing = tabs.value.find((tab) => tab.path === normalizedPath);
  if (existing) {
    existing.loading = true;
    existing.error = "";
    existing.loaded = false;
    activePath.value = existing.path;
    replaceTabState(existing);
    scheduleAddressScrollStateUpdate();
    return existing;
  }
  const current = activeTab.value;
  if (reuseActiveTab && current && !current.loading) {
    const previousPath = current.path;
    clearFileBlockCaches(previousPath);
    current.path = normalizedPath;
    current.title = titleFromPath(normalizedPath);
    current.extension = "";
    current.kind = "code";
    current.content = "";
    current.rawMode = false;
    current.forcePlain = false;
    current.virtualized = false;
    current.totalLines = 0;
    current.blockLineCount = 0;
    current.loaded = false;
    current.loading = true;
    current.error = "";
    activePath.value = normalizedPath;
    replaceTabState(current, previousPath);
    scheduleAddressScrollStateUpdate();
    return current;
  }
  const tab: FileTab = {
    path: normalizedPath, title: titleFromPath(normalizedPath), extension: "",
    kind: "code", content: "", rawMode: false, forcePlain: false, virtualized: false, totalLines: 0, blockLineCount: 0, loaded: false, loading: true, error: "",
  };
  tabs.value = [...tabs.value, tab];
  activePath.value = normalizedPath;
  scheduleAddressScrollStateUpdate();
  return tab;
}

function setActiveTab(path: string) {
  const normalizedPath = normalizePath(path);
  const changed = !sameNormalizedPath(activePath.value, normalizedPath);
  activePath.value = normalizedPath;
  scheduleAddressScrollStateUpdate();
  const tab = tabs.value.find((item) => item.path === normalizedPath);
  if (tab && !tab.loaded && !tab.loading) {
    void openPath(normalizedPath);
  } else if (changed) {
    void resetScrollAndCaptureFirstPage();
  } else {
    void nextTick(() => captureVisibleRangeContextNow());
  }
}

function toggleActiveRawMode() {
  const tab = activeTab.value;
  if (!tab || !canToggleRawMode(tab)) return;
  tab.rawMode = !tab.rawMode;
  replaceTabState(tab);
}

function openOrActivatePath(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  // 树内点击已打开的文件走 setActiveTab，不经过 openPath，这里同样要收起覆盖模式下的目录
  collapseDirectoryTreeForOverlayOpen();
  const existing = tabs.value.some((tab) => tab.path === normalizedPath);
  if (existing) {
    setActiveTab(normalizedPath);
    return;
  }
  const current = activeTab.value;
  const sameDirectory = !!current && directoryFromPath(current.path) === directoryFromPath(normalizedPath);
  void openPath(normalizedPath, { reuseActiveTab: sameDirectory });
}

function handleTreeEntryOpen(entry: FileReaderDirectoryEntry) {
  // 目录由 menu details 原生折叠处理，这里只处理文件条目
  const normalizedPath = normalizePath(entry.path);
  if (!normalizedPath) return;
  if (props.directoryOnly) {
    emit("selectPath", normalizedPath);
    return;
  }
  openOrActivatePath(normalizedPath);
}

function migrateTabPath(tab: FileTab, fromPath: string, toPath: string) {
  if (fromPath === toPath) return tab;
  const normalizedFromPath = normalizePath(fromPath);
  const duplicated = tabs.value.find((item) => item !== tab && item.path === toPath);
  if (duplicated) {
    tabs.value = tabs.value.filter((item) => item.path !== normalizedFromPath);
    clearFileBlockCaches(fromPath);
    activePath.value = toPath;
    scheduleAddressScrollStateUpdate();
    void resetScrollAndCaptureFirstPage();
    return duplicated;
  }
  migrateVirtualCodeCaches(fromPath, toPath);
  tab.path = toPath;
  activePath.value = toPath;
  replaceTabState(tab, fromPath);
  scheduleAddressScrollStateUpdate();
  return tab;
}

function reportFileReaderActionFailure(action: string, path: string, error: unknown) {
  const detail = error instanceof Error ? error.message : String(error);
  console.error(`[文件阅读器] ${action}失败`, { path, error });
  actionErrorMessage.value = t('fileReader.actionFailed', { action, path, detail });
  window.setTimeout(() => {
    if (actionErrorMessage.value === t('fileReader.actionFailed', { action, path, detail })) {
      actionErrorMessage.value = "";
    }
  }, 4500);
}

// ==================== Public API ====================

async function scrollActiveFileToLine(line: number) {
  const tab = activeTab.value;
  if (!tab || !Number.isFinite(line) || line < 1) return;
  if (tab.kind === "markdown" && !tab.rawMode) {
    tab.rawMode = true;
    replaceTabState(tab);
  }
  await nextTick();
  await nextTick();
  const scroller = activeContentScroller();
  if (!scroller) return;
  const totalLines = Math.max(1, tab.totalLines || splitContentLines(tab.content).length);
  const progress = Math.min(1, Math.max(0, (line - 1) / Math.max(1, totalLines - 1)));
  scroller.scrollTop = progress * Math.max(0, scroller.scrollHeight - scroller.clientHeight);
  captureVisibleRangeContextNow({ force: true });
}

async function openPath(path: string, options: { reuseActiveTab?: boolean; targetLine?: number; revealInDirectoryTree?: boolean } = {}) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  // revealInDirectoryTree 需要在目录树中定位，此时不能收起目录
  if (!options.revealInDirectoryTree) collapseDirectoryTreeForOverlayOpen();
  const shouldResetScrollAfterOpen = !sameNormalizedPath(activePath.value, normalizedPath);
  const current = tabs.value.find((tab) => tab.path === normalizedPath);
  if (current?.loading) {
    activePath.value = normalizedPath;
    scheduleAddressScrollStateUpdate();
    if (shouldResetScrollAfterOpen) {
      void resetScrollAndCaptureFirstPage();
    }
    if (options.revealInDirectoryTree) {
      await revealPathInDirectoryTree(normalizedPath);
    }
    return;
  }
  let tab = upsertLoadingTab(normalizedPath, !!options.reuseActiveTab);
  const localKind = fileKindFromPath(normalizedPath);
  if (!isTextFileKind(localKind)) {
    tab.title = titleFromPath(normalizedPath);
    tab.extension = extensionFromPath(normalizedPath);
    tab.kind = localKind;
    tab.content = "";
    tab.rawMode = false;
    tab.forcePlain = false;
    tab.virtualized = false;
    tab.totalLines = 0;
    tab.blockLineCount = 0;
    tab.loaded = true;
    tab.error = "";
    tab.loading = false;
    activePath.value = normalizedPath;
    replaceTabState(tab);
    clearFileBlockCaches(normalizedPath);
    scheduleAddressScrollStateUpdate();
    if (shouldResetScrollAfterOpen) {
      void resetActiveContentScrollToTop();
    }
    emit("openPath", normalizedPath);
    if (options.revealInDirectoryTree) {
      await revealPathInDirectoryTree(normalizedPath);
    }
    return;
  }
  try {
    const payload = await requestFileReaderFile(normalizedPath);
    const resolvedPath = normalizePath(payload.path || normalizedPath);
    tab = migrateTabPath(tab, normalizedPath, resolvedPath);
    tab.title = payload.name || titleFromPath(resolvedPath);
    tab.extension = String(payload.extension || extensionFromPath(resolvedPath)).toLowerCase();
    tab.kind = String(payload.kind || "code");
    tab.content = String(payload.content || "");
    tab.rawMode = false;
    tab.forcePlain = !!payload.forcePlain;
    tab.virtualized = !!payload.virtualized;
    tab.totalLines = Number(payload.totalLines || 0);
    tab.blockLineCount = Number(payload.blockLineCount || 0);
    tab.loaded = true;
    tab.error = "";
    tab.loading = false;
    activePath.value = resolvedPath;
    replaceTabState(tab);
    clearFileBlockCaches(resolvedPath);
    scheduleAddressScrollStateUpdate();
    if (shouldResetScrollAfterOpen) {
      void resetScrollAndCaptureFirstPage();
    } else {
      void nextTick(() => captureVisibleRangeContextNow({ force: true }));
    }
    emit("openPath", resolvedPath);
    if (options.revealInDirectoryTree) {
      await revealPathInDirectoryTree(resolvedPath);
    }
    if (options.targetLine) {
      await scrollActiveFileToLine(options.targetLine);
    }
  } catch (error) {
    tab.loaded = true;
    tab.loading = false;
    tab.error = error instanceof Error ? error.message : String(error);
    replaceTabState(tab);
  }
}

async function openMarkdownFileLink(event: MouseEvent) {
  const anchor = (event.target as HTMLElement | null)?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  const rawHref = anchor.getAttribute("data-href") || anchor.getAttribute("href") || "";
  const href = normalizeLocalLinkHref(rawHref);
  if (!href || href === "#" || href.startsWith("#") || /^https?:\/\//i.test(href)) return;

  const reference = parseLocalFileReference(href);
  const referencedPath = reference?.path || href;
  const tab = activeTab.value;
  if (!tab) return;
  const targetPath = isAbsoluteLocalPath(referencedPath)
    ? referencedPath
    : `${directoryFromPath(tab.path)}/${referencedPath.replace(/^\.\//, "")}`;
  event.preventDefault();
  event.stopPropagation();
  await openPath(targetPath, { targetLine: reference?.line });
}

async function openMarkdownImagePreview(payload: { src?: string; localPath?: string; alt?: string }) {
  const src = String(payload?.src || "").trim();
  const localPath = String(payload?.localPath || "").trim();
  if (src) {
    openImagePreview({ mime: "image/png", dataUrl: src, localPath });
    return;
  }
  if (localPath) {
    if (isAssistantSpacePath(localPath)) {
      try {
        const result = await readTransportChatImage({ path: localPath, mime: "image/png", original: true });
        const dataUrl = String(result?.dataUrl || "").trim();
        if (dataUrl) openImagePreview({ mime: result?.mime || "image/png", dataUrl, localPath });
      } catch (error) {
        console.warn("[文件浏览器预览] Assistant Space 图片原图加载失败", { path: localPath, error });
      }
      return;
    }
    openImagePreview({ mime: "image/png", dataUrl: resolveLocalFileUrl(localPath), localPath });
  }
}

async function handleCopyMarkdownImage(path: string) {
  imagePreviewCopyStatus.value = "doing";
  try {
    await copyTransportChatImageToClipboard(path);
  } catch (error) {
    console.warn("[文件浏览器预览] 复制图片失败", error);
  } finally {
    imagePreviewCopyStatus.value = "idle";
  }
}

async function handleSaveMarkdownImage(path: string) {
  imagePreviewSaveStatus.value = "doing";
  try {
    await saveTransportChatImageAs(path);
  } catch (error) {
    console.warn("[文件浏览器预览] 保存图片失败", error);
  } finally {
    imagePreviewSaveStatus.value = "idle";
  }
}

async function openDroppedPaths(paths: string[]) {
  const normalizedPaths = paths.map((path) => normalizePath(path)).filter(Boolean);
  for (const path of normalizedPaths) {
    await openPath(path);
  }
}

function closeTab(path: string) {
  closeTabsByPaths([path], { preferredActivePath: path });
}

function closeTabsToLeftOf(path: string) {
  const index = tabs.value.findIndex((tab) => tab.path === path);
  if (index <= 0) return;
  closeTabsByPaths(
    tabs.value.slice(0, index).map((tab) => tab.path),
    { preferredActivePath: path },
  );
}

function closeTabsToRightOf(path: string) {
  const index = tabs.value.findIndex((tab) => tab.path === path);
  if (index < 0) return;
  closeTabsByPaths(
    tabs.value.slice(index + 1).map((tab) => tab.path),
    { preferredActivePath: path },
  );
}

function closeOtherTabs(path: string) {
  closeTabsByPaths(
    tabs.value.filter((tab) => tab.path !== path).map((tab) => tab.path),
    { preferredActivePath: path },
  );
}

function closeTabsByPaths(paths: string[], options?: { preferredActivePath?: string }) {
  const normalizedPaths = new Set(paths.map((path) => normalizePath(path)).filter(Boolean));
  if (normalizedPaths.size === 0) return;
  const currentTabs = tabs.value;
  const firstRemovedIndex = currentTabs.findIndex((tab) => normalizedPaths.has(normalizePath(tab.path)));
  if (firstRemovedIndex < 0) return;

  const nextTabs = currentTabs.filter((tab) => !normalizedPaths.has(normalizePath(tab.path)));
  const preferredActivePath = normalizePath(options?.preferredActivePath || "");
  const activeWillRemain = activePath.value && nextTabs.some((tab) => tab.path === activePath.value);

  tabs.value = nextTabs;
  for (const removedPath of normalizedPaths) {
    clearFileBlockCaches(removedPath);
  }
  emit("clearContextReferences", [...normalizedPaths]);

  if (activeWillRemain) return;

  const nextActivePath =
    (preferredActivePath && nextTabs.find((tab) => tab.path === preferredActivePath)?.path)
    || nextTabs[Math.max(0, firstRemovedIndex - 1)]?.path
    || nextTabs[firstRemovedIndex]?.path
    || nextTabs[0]?.path
    || "";

  activePath.value = nextActivePath;
  scheduleAddressScrollStateUpdate();

  const nextTab = nextTabs.find((tab) => tab.path === nextActivePath);
  if (nextTab && !nextTab.loaded && !nextTab.loading) {
    void openPath(nextTab.path);
  } else {
    void resetScrollAndCaptureFirstPage();
  }
}

function refreshActiveTab() {
  const tab = activeTab.value;
  if (!tab || tab.kind === "diff") return;
  void openPath(tab.path);
}

async function openWithDefaultProgram() {
  const tab = activeTab.value;
  if (!tab) return;
  await openPathWithDefaultProgram(tab.path);
}

async function openInDocumentBrowser(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await openTransportFileReaderWindow(normalizedPath);
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.actionOpenDocumentBrowser'), normalizedPath, error);
  }
}

async function openPathWithDefaultProgram(path: string) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await openTransportFileWithDefaultProgram(normalizedPath);
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.actionOpenDefault'), normalizedPath, error);
  }
}

// ==================== Git 面板 ====================

function switchToGitMode() {
  asideMode.value = "git";
}

function buildGitDiffTabKey(source: GitDiffTabSource) {
  const hashPart = source.hash ? `:${source.hash}` : "";
  return `git-diff:${source.workspacePath}:${source.staged ? "staged" : "worktree"}${hashPart}:${source.path}`;
}

function buildGitDiffTabTitle(source: GitDiffTabSource) {
  if (source.hash) {
    // 聚合模式（path === hash）：标题显示"全部更改"，避免 hash 重复
    if (source.path === source.hash) {
      return `${source.hash.slice(0, 7)} ${t('gitPanel.viewCommitChanges')}`;
    }
    return `${source.hash.slice(0, 7)} ${titleFromPath(source.path)}`;
  }
  const base = titleFromPath(source.path);
  return source.staged ? `${base} (已暂存)` : `${base} (更改)`;
}

async function openGitDiffTab(source: GitDiffTabSource) {
  if (!localFileSystemAvailable) return;
  const workspacePath = normalizePath(source.workspacePath);
  if (!workspacePath) return;
  // 未跟踪文件：git diff 无内容，直接打开文件本体
  if (source.untracked) {
    await openPath(`${workspacePath}/${source.path}`);
    return;
  }
  const tabPath = buildGitDiffTabKey(source);
  try {
    const result = source.hash
      ? await gitPanelShow(workspacePath, source.hash, source.path === source.hash ? "" : source.path, source.context)
      : await gitPanelDiff({ workspacePath, path: source.path, staged: source.staged, context: source.context });
    const diffText = String(result?.diff || "");
    if (!diffText.trim()) {
      // 空 diff 是正常提示而非操作失败，直接展示文案，不套 actionFailed 模板
      const message = t('fileReader.noDiffContent');
      actionErrorMessage.value = message;
      window.setTimeout(() => {
        if (actionErrorMessage.value === message) actionErrorMessage.value = "";
      }, 4500);
      return;
    }
    const existing = tabs.value.find((tab) => tab.path === tabPath);
    const tab: FileTab = existing || {
      path: tabPath,
      title: buildGitDiffTabTitle(source),
      extension: "diff",
      kind: "diff",
      content: "",
      rawMode: true,
      forcePlain: true,
      virtualized: false,
      totalLines: 0,
      blockLineCount: 0,
      loaded: true,
      loading: false,
      error: "",
    };
    tab.title = buildGitDiffTabTitle(source);
    tab.kind = "diff";
    tab.extension = "diff";
    tab.rawMode = true;
    tab.forcePlain = true;
    tab.virtualized = false;
    tab.totalLines = 0;
    tab.blockLineCount = 0;
    tab.loaded = true;
    tab.loading = false;
    tab.error = "";
    tab.content = diffText;
    tab.diffSource = { ...source, workspacePath };
    if (!existing) {
      tabs.value = [...tabs.value, tab];
    } else {
      replaceTabState(tab);
    }
    // 同会话同时只保留一个 diff 文件：关闭其他已打开的 diff tab
    const otherDiffTabs = tabs.value.filter((item) => item.kind === "diff" && item.path !== tabPath);
    if (otherDiffTabs.length > 0) {
      closeTabsByPaths(
        otherDiffTabs.map((item) => item.path),
        { preferredActivePath: tabPath },
      );
    }
    activePath.value = tabPath;
    scheduleAddressScrollStateUpdate();
    void resetActiveContentScrollToTop();
    if (asideMode.value !== "git") {
      asideMode.value = "git";
    }
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.loadGitDiffFailed'), source.path, error);
  }
}

/** 点击 diff 中"展开后续"条：增大 -U 上下文重新拉取并刷新当前 tab */
async function expandGitDiffContext() {
  const active = activeTab.value;
  if (!active || active.kind !== "diff" || !active.diffSource) return;
  const source = active.diffSource;
  const nextContext = Math.max(
    source.context ? source.context + 30 : 33,
    33,
  );
  await openGitDiffTab({ ...source, context: nextContext });
}

/** 多文件 diff：仅展开单个文件内的隐藏行（按文件重拉并局部替换） */
async function expandFileDiffContext(payload: { path: string; nextContext: number }) {
  const active = activeTab.value;
  if (!active || active.kind !== "diff" || !active.diffSource) return;
  const source = active.diffSource;
  const targetPath = String(payload.path || "").trim();
  const nextContext = Math.max(1, Number(payload.nextContext) || 33);
  if (!targetPath) return;
  try {
    let newSegment = "";
    if (source.hash) {
      const result = await gitPanelShow(source.workspacePath, source.hash, targetPath, nextContext);
      newSegment = String(result?.diff || "");
    } else {
      const result = await gitPanelDiff({ workspacePath: source.workspacePath, path: targetPath, staged: source.staged, context: nextContext });
      newSegment = String(result?.diff || "");
    }
    if (!newSegment.trim()) return;
    const updated = replaceFileSection(String(active.content || ""), targetPath, newSegment);
    // 更新当前 diff tab 的文本与上下文标记
    const nextTab: FileTab = { ...active, content: updated, diffSource: { ...source, context: Math.max(source.context || 3, nextContext) } };
    replaceTabState(nextTab);
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.loadGitDiffFailed'), targetPath, error);
  }
}

function openDiffTargetFile() {
  const tab = activeTab.value;
  const source = tab?.diffSource;
  if (!tab || tab.kind !== "diff" || !source?.path || !source.workspacePath) return;
  const targetPath = normalizePath(`${source.workspacePath}/${source.path}`);
  if (!targetPath) return;
  void openPath(targetPath);
}

function openMultiFileTarget(payload: { path: string }) {
  const tab = activeTab.value;
  const source = tab?.diffSource;
  const filePath = String(payload?.path || "").trim();
  const workspacePath = normalizePath(source?.workspacePath || "");
  if (!filePath || !workspacePath) return;
  const targetPath = normalizePath(`${workspacePath}/${filePath}`);
  if (!targetPath) return;
  void openPath(targetPath);
}

async function pickFile() {
  const picked = await openTransportFileDialog({ multiple: false, directory: false, title: t('fileReader.openFile') });
  if (!picked || Array.isArray(picked)) return;
  await openPath(String(picked));
}

// ==================== Directory Tree ====================

async function requestFileReaderDirectory(path: string): Promise<FileReaderDirectoryPayload> {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) {
    return {
      path: "",
      name: "",
      entries: [],
    };
  }
  return await invokeTauri<FileReaderDirectoryPayload>("fileReader.directory.list", { path: normalizedPath });
}

async function requestFileReaderFile(path: string): Promise<FileReaderFilePayload> {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) {
    return {
      path: "",
      name: "",
      extension: "",
      kind: "code",
      content: "",
      forcePlain: false,
      virtualized: false,
      totalLines: 0,
      blockLineCount: 0,
    };
  }
  return await invokeTauri<FileReaderFilePayload>("fileReader.readFile", { path: normalizedPath });
}

async function requestFileReaderFileBlock(path: string, startLine: number, lineCount: number): Promise<FileReaderFileBlockPayload> {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) {
    return { path: "", startLine: 1, endLine: 1, content: "" };
  }
  return await invokeTauri<FileReaderFileBlockPayload>("fileReader.readFileBlock", {
    path: normalizedPath,
    startLine,
    lineCount,
  });
}

async function loadDirectory(path: string, expanded: boolean): Promise<boolean> {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return false;
  updateDirectoryNode(normalizedPath, { loading: true, error: "", expanded });
  try {
    const payload = await requestFileReaderDirectory(normalizedPath);
    const resolvedPath = normalizePath(payload.path || normalizedPath);
    if (directoryRootPath.value === normalizedPath) {
      directoryRootPath.value = resolvedPath;
    }
    updateDirectoryNode(resolvedPath, {
      name: String(payload.name || titleFromPath(resolvedPath)),
      entries: normalizeDirectoryEntries(payload.entries || []),
      loaded: true, loading: false, error: "", expanded,
    });
    return true;
  } catch (error) {
    updateDirectoryNode(normalizedPath, {
      loaded: false, loading: false,
      error: error instanceof Error ? error.message : String(error), expanded,
    });
    return false;
  }
}

async function openDirectoryTree(path: string, options: { switchToFiles?: boolean } = {}): Promise<boolean> {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return false;
  if (options.switchToFiles !== false) {
    asideMode.value = "files";
  }
  directoryRootPath.value = normalizedPath;
  return await loadDirectory(normalizedPath, true);
}

async function revealPathInDirectoryTree(path: string) {
  const rootPath = normalizePath(directoryRootPath.value);
  const targetDirectoryPath = directoryFromPath(path);
  const chain = directoryPathChain(rootPath, targetDirectoryPath);
  if (chain.length === 0) return;

  directoryTreeFilter.value = "";
  for (const directoryPath of chain) {
    const node = treeDirectoryNode(directoryPath);
    if (!node?.loaded || node.error) {
      await loadDirectory(directoryPath, true);
    } else {
      updateDirectoryNode(directoryPath, { expanded: true, error: "" });
    }
  }

  await nextTick();
  const targetPath = normalizePath(path);
  const scroller = directoryAreaRef.value?.scrollerRef ?? null;
  if (!scroller) return;
  // menu 树行高不再固定：直接定位到目标行元素
  const target = scroller.querySelector(`a[data-tree-path="${CSS.escape(targetPath)}"]`);
  if (!(target instanceof HTMLElement)) return;
  target.scrollIntoView({ block: "center" });
}

function closeDirectoryTree() {
  directoryRootPath.value = "";
  directoryTreeFilter.value = "";
}

// 覆盖模式（浮层）下目录树会占满面板、内容区被隐藏，打开或切换文件后收起目录，避免新文件看不到。
function collapseDirectoryTreeForOverlayOpen() {
  if (props.narrowOverlay !== true) return;
  if (!directoryRootPath.value) return;
  closeDirectoryTree();
}

// ==================== Hover Directory Tree ====================

async function showHoverDirectoryTree(path: string, event: MouseEvent) {
  if (hoverHideTimer) { window.clearTimeout(hoverHideTimer); hoverHideTimer = null; }

  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;

  hoverDirectoryTreeAnchor.value = normalizedPath;
  hoverDirectoryTreeVisible.value = true;

  await nextTick();

  const target = event.target as HTMLElement;
  const rect = target.getBoundingClientRect();
  const panelWidth = 280;
  const panelHeight = Math.round(window.innerHeight * 0.8);
  const gap = 4;

  let left = rect.left;
  if (left + panelWidth > window.innerWidth) {
    left = window.innerWidth - panelWidth - 8;
  }
  if (left < 8) left = 8;

  let top = rect.bottom + gap;
  if (top + panelHeight > window.innerHeight) {
    top = Math.max(8, rect.top - panelHeight - gap);
  }

  hoverDirectoryTreeStyle.value = {
    left: `${left}px`,
    top: `${top}px`,
    width: `${panelWidth}px`,
    height: `${panelHeight}px`,
  };

  if (hoverDirectoryTreeNodes.value[normalizedPath]?.loaded) {
    hoverDirectoryTreeRoot.value = hoverDirectoryTreeNodes.value[normalizedPath];
    return;
  }

  await loadDirectoryForHover(normalizedPath);
}

function dismissHoverDirectoryTree() {
  if (hoverHideTimer) { window.clearTimeout(hoverHideTimer); hoverHideTimer = null; }
  hoverDirectoryTreeVisible.value = false;
  hoverDirectoryTreeRoot.value = null;
  hoverDirectoryTreeAnchor.value = "";
}

function hideHoverDirectoryTree() {
  hoverHideTimer = window.setTimeout(dismissHoverDirectoryTree, 150);
}

function cancelHideHoverDirectoryTree() {
  if (hoverHideTimer) { window.clearTimeout(hoverHideTimer); hoverHideTimer = null; }
}

async function loadDirectoryForHover(path: string) {
  const dirName = path.split(/[\\/]/).filter(Boolean).pop() || path;
  hoverDirectoryTreeRoot.value = updateHoverDirectoryNode(path, {
    name: dirName,
    entries: [],
    loaded: false,
    loading: true,
    error: "",
    expanded: true,
  });

  try {
    const payload = await requestFileReaderDirectory(path);
    const resolvedPath = normalizePath(payload.path || path);
    const resolvedName = String(payload.name || dirName);
    const normalizedEntries = normalizeDirectoryEntries(payload.entries || []);
    hoverDirectoryTreeRoot.value = updateHoverDirectoryNode(resolvedPath, {
      name: resolvedName,
      entries: normalizedEntries,
      loaded: true,
      loading: false,
      error: "",
      expanded: true,
    });
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    hoverDirectoryTreeRoot.value = updateHoverDirectoryNode(path, {
      name: dirName,
      entries: [],
      loaded: true,
      loading: false,
      error: errorMsg,
      expanded: true,
    });
  }
}

function updateHoverDirectoryNode(path: string, patch: Partial<DirectoryNode>) {
  const normalizedPath = normalizePath(path);
  const current = hoverDirectoryTreeNodes.value[normalizedPath] || {
    path: normalizedPath,
    name: titleFromPath(normalizedPath),
    entries: [],
    loaded: false,
    loading: false,
    error: "",
    expanded: false,
  };
  const next = { ...current, ...patch, path: normalizedPath };
  hoverDirectoryTreeNodes.value = {
    ...hoverDirectoryTreeNodes.value,
    [normalizedPath]: next,
  };
  if (hoverDirectoryTreeRoot.value?.path === normalizedPath) {
    hoverDirectoryTreeRoot.value = next;
  }
  return next;
}

/** details 原生切换：折叠直接落状态；展开时已加载则置展开，否则置 loading 并拉取子目录 */
function onHoverDirectoryToggle(entry: FileReaderDirectoryEntry, open: boolean) {
  const normalizedPath = normalizePath(entry.path);
  const node = hoverDirectoryTreeNodes.value[normalizedPath];

  if (!open) {
    updateHoverDirectoryNode(normalizedPath, { expanded: false });
    return;
  }
  if (node?.loaded) {
    updateHoverDirectoryNode(normalizedPath, { expanded: true });
    return;
  }
  updateHoverDirectoryNode(normalizedPath, {
    name: String(entry.name || titleFromPath(normalizedPath)),
    entries: [],
    loaded: false,
    error: "",
    loading: true,
    expanded: true,
  });
  void loadHoverSubDirectory(entry);
}

async function loadHoverSubDirectory(entry: FileReaderDirectoryEntry) {
  const normalizedPath = normalizePath(entry.path);
  try {
    const payload = await requestFileReaderDirectory(normalizedPath);
    const normalizedEntries = normalizeDirectoryEntries(payload.entries || []);
    updateHoverDirectoryNode(normalizedPath, { entries: normalizedEntries, loaded: true, loading: false, error: "", expanded: true });
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    updateHoverDirectoryNode(normalizedPath, { loading: false, loaded: true, error: errorMsg });
  }
}

async function openFileFromHoverTree(entry: FileReaderDirectoryEntry) {
  hideHoverDirectoryTree();
  await openPath(entry.path);
}

async function toggleDirectoryTree() {
  if (directoryTreeRoot.value) {
    closeDirectoryTree();
    return;
  }
  const path = directoryToggleTargetPath.value;
  if (path) {
    await openDirectoryTree(path);
  }
}

async function toggleFilesPanel() {
  if (directoryTreeRoot.value && asideMode.value === "files") {
    closeDirectoryTree();
    return;
  }
  if (!directoryTreeRoot.value) {
    const path = directoryToggleTargetPath.value;
    if (path) {
      await openDirectoryTree(path);
      return;
    }
  }
  asideMode.value = "files";
}

async function openGitPanel(tab?: "commits" | "stashes" | "branches") {
  const alreadyOpen = !!directoryTreeRoot.value && asideMode.value === "git";
  if (tab) {
    // 面板已挂载就直接切；未挂载则记下来，等它挂载后由 gitPanelRef 监听补上
    if (gitPanelRef.value) gitPanelRef.value.setActiveTab(tab);
    else pendingGitTab.value = tab;
  } else {
    // 不带目标的打开是常规入口，不能沿用上一次残留的目标标签页
    pendingGitTab.value = null;
  }
  if (alreadyOpen) return;
  if (!directoryTreeRoot.value) {
    const path = directoryToggleTargetPath.value;
    if (path) {
      await openDirectoryTree(path);
      // openDirectoryTree 默认切到 files，这里改回 git
      asideMode.value = "git";
      return;
    }
    if (gitPanelWorkspacePath.value) {
      asideMode.value = "git";
      // 无目录树时仅切模式，aside 会通过 gitPanelWorkspacePath 展示
      // 临时用 git 工作区作为占位根，避免 aside 仍隐藏
      directoryRootPath.value = gitPanelWorkspacePath.value;
      await loadDirectory(gitPanelWorkspacePath.value, true);
      asideMode.value = "git";
      return;
    }
  }
  asideMode.value = "git";
}

async function toggleGitPanel() {
  if (directoryTreeRoot.value && asideMode.value === "git") {
    closeDirectoryTree();
    return;
  }
  await openGitPanel();
}

function readStoredDirectoryOpenTargetKind() {
  if (typeof window === "undefined") return "";
  try {
    return String(window.localStorage.getItem(FILE_READER_OPEN_TARGET_STORAGE_KEY) || "").trim();
  } catch {
    return "";
  }
}

function storeDirectoryOpenTargetKind(kind: string) {
  if (typeof window === "undefined") return;
  const normalized = String(kind || "").trim();
  try {
    if (normalized) {
      window.localStorage.setItem(FILE_READER_OPEN_TARGET_STORAGE_KEY, normalized);
    } else {
      window.localStorage.removeItem(FILE_READER_OPEN_TARGET_STORAGE_KEY);
    }
  } catch {
    // 忽略本地存储失败
  }
}

const DEFAULT_DIRECTORY_OPEN_TARGETS: DirectoryOpenTargetOption[] = [
  { kind: "explorer", label: "资源管理器", type: "explorer" },
  { kind: "vscode", label: "VS Code", type: "vscode" },
];

function normalizeDirectoryOpenTargetLabel(item: DirectoryOpenTargetOption): string {
  const raw = String(item.label || "").trim();
  if (!raw) return "打开目标";
  if (item.type !== "shell") return raw;
  return raw.replace(/\s*\([^()]*\)\s*$/, "").trim() || raw;
}

function normalizeDirectoryOpenTargetOptions(options: DirectoryOpenTargetOption[]): DirectoryOpenTargetOption[] {
  const seen = new Set<string>();
  return options
    .filter((item) => {
      const kind = String(item.kind || "").trim();
      if (!kind || kind === "auto" || seen.has(kind)) return false;
      seen.add(kind);
      return true;
    })
    .map((item): DirectoryOpenTargetOption => ({
      kind: String(item.kind || "").trim(),
      label: normalizeDirectoryOpenTargetLabel(item),
      type: item.type === "vscode" || item.type === "explorer" ? item.type : "shell",
      iconDataUrl: String(item.iconDataUrl || "").trim() || undefined,
    }));
}

const directoryOpenTargets = computed<DirectoryOpenTargetOption[]>(() => {
  const normalized = normalizeDirectoryOpenTargetOptions(directoryOpenTargetOptions.value);
  return normalized.length ? normalized : DEFAULT_DIRECTORY_OPEN_TARGETS;
});

function normalizeDirectoryOpenTargetKind(kind: string, options = directoryOpenTargets.value) {
  const normalized = String(kind || "").trim();
  if (normalized && options.some((item) => item.kind === normalized)) return normalized;
  return options[0]?.kind || "explorer";
}

function currentDirectoryOpenTargetKind() {
  return normalizeDirectoryOpenTargetKind(selectedDirectoryOpenTargetKind.value);
}

const currentDirectoryOpenTarget = computed<DirectoryOpenTargetOption>(() => {
  const currentKind = currentDirectoryOpenTargetKind();
  return directoryOpenTargets.value.find((item) => item.kind === currentKind)
    || directoryOpenTargets.value[0]
    || DEFAULT_DIRECTORY_OPEN_TARGETS[0];
});

const selectedDirectoryOpenTargetTitle = computed(() => `用 ${currentDirectoryOpenTarget.value.label} 打开当前目录`);

async function loadDirectoryOpenTargets() {
  if (!localFileSystemAvailable) return;
  directoryOpenTargetsLoading.value = true;
  try {
    const payload = await listTransportFileReaderDirectoryOpenTargets<DirectoryOpenTargetsResult>();
    directoryOpenTargetOptions.value = normalizeDirectoryOpenTargetOptions(Array.isArray(payload.options) ? payload.options : []);
  } catch {
    directoryOpenTargetOptions.value = [];
  } finally {
    const stored = readStoredDirectoryOpenTargetKind();
    const fallbackKind = directoryOpenTargets.value[0]?.kind || "explorer";
    const nextKind = normalizeDirectoryOpenTargetKind(stored || fallbackKind);
    selectedDirectoryOpenTargetKind.value = nextKind;
    if (!stored || stored !== nextKind) {
      storeDirectoryOpenTargetKind(nextKind);
    }
    directoryOpenTargetsLoading.value = false;
  }
}

async function selectDirectoryOpenTarget(kind: string) {
  const nextKind = normalizeDirectoryOpenTargetKind(kind);
  selectedDirectoryOpenTargetKind.value = nextKind;
  storeDirectoryOpenTargetKind(nextKind);
  directoryOpenTargetDropdownOpen.value = false;
  await openDirectoryAtTreeRoot(nextKind);
}

function toggleDirectoryOpenTargetDropdown() {
  directoryOpenTargetDropdownOpen.value = !directoryOpenTargetDropdownOpen.value;
}

function closeDirectoryOpenTargetDropdown() {
  directoryOpenTargetDropdownOpen.value = false;
}

async function openDirectoryAtTreeRoot(kind = currentDirectoryOpenTargetKind()) {
  closeDirectoryOpenTargetDropdown();
  if (!localFileSystemAvailable) return;
  const root = directoryTreeRoot.value;
  const path = root ? root.path : directoryToggleTargetPath.value;
  if (!path) return;
  await openDirectoryWithTarget(path, kind);
}

async function openDirectoryWithTarget(path: string, targetKind = currentDirectoryOpenTargetKind()) {
  if (!localFileSystemAvailable) return;
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await openTransportFileReaderDirectoryTarget(normalizedPath, targetKind);
  } catch (error) {
    reportFileReaderActionFailure("打开当前目录", normalizedPath, error);
  }
}

async function openDirectoryInFileManager(path: string) {
  if (!localFileSystemAvailable) return;
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  try {
    await openTransportLocalDirectory(normalizedPath);
  } catch (error) {
    reportFileReaderActionFailure(t('fileReader.actionOpenDirectory'), normalizedPath, error);
  }
}

/** details 原生切换：折叠直接落状态；展开时已加载则置展开，否则懒加载（loadDirectory 成功即展开） */
function onTreeDirectoryToggle(entry: FileReaderDirectoryEntry, open: boolean) {
  const normalizedPath = normalizePath(entry.path);
  const node = treeDirectoryNode(normalizedPath);
  if (!open) {
    updateDirectoryNode(normalizedPath, { expanded: false });
    return;
  }
  if (node?.loaded) {
    updateDirectoryNode(normalizedPath, { expanded: true, error: "" });
    return;
  }
  void loadDirectory(normalizedPath, true);
}

function updateDirectoryNode(path: string, patch: Partial<DirectoryNode>) {
  const normalizedPath = normalizePath(path);
  if (!normalizedPath) return;
  const current = directoryNodes.value[normalizedPath] || {
    path: normalizedPath, name: titleFromPath(normalizedPath), entries: [],
    loaded: false, loading: false, error: "", expanded: false,
  };
  directoryNodes.value = {
    ...directoryNodes.value,
    [normalizedPath]: { ...current, ...patch, path: normalizedPath },
  };
}

function treeDirectoryNode(path: string) {
  return directoryNodes.value[normalizePath(path)] || null;
}

function isTreeDirectoryExpanded(path: string) {
  return !!treeDirectoryNode(path)?.expanded;
}

function flattenDirectoryEntries(entries: FileReaderDirectoryEntry[], depth: number, filter = ""): TreeRow[] {
  const normalizedFilter = filter.trim().toLowerCase();
  const rows: TreeRow[] = [];
  for (const entry of entries) {
    const normalizedPath = normalizePath(entry.path);
    const childRows = entry.isDirectory ? flattenDirectoryEntries(treeDirectoryNode(normalizedPath)?.entries || [], depth + 1, filter) : [];
    const matchesFilter = !normalizedFilter || entry.name.toLowerCase().includes(normalizedFilter);
    if (normalizedFilter && !matchesFilter && childRows.length === 0) continue;
    rows.push({ kind: "entry", key: `entry:${normalizedPath}`, depth, entry: { ...entry, path: normalizedPath } });
    if (normalizedFilter) {
      rows.push(...childRows);
      continue;
    }
    if (!entry.isDirectory || !isTreeDirectoryExpanded(normalizedPath)) continue;
    const node = treeDirectoryNode(normalizedPath);
    if (!node || node.loading) {
      rows.push({ kind: "status", key: `loading:${normalizedPath}`, depth: depth + 1, text: t('fileReader.loadingDirectory') });
    } else if (node.error) {
      rows.push({ kind: "status", key: `error:${normalizedPath}`, depth: depth + 1, text: node.error });
    } else if (node.loaded && node.entries.length === 0) {
      rows.push({ kind: "status", key: `empty:${normalizedPath}`, depth: depth + 1, text: t('fileReader.emptyDirectory') });
    } else if (node.loaded) {
      rows.push(...flattenDirectoryEntries(node.entries, depth + 1));
    }
  }
  return rows;
}

function handleGlobalPointerDown(event: PointerEvent) {
  const target = event.target instanceof Node ? event.target : null;
  if (target && directoryOpenTargetDropdownRef.value?.contains(target)) return;
  closeDirectoryOpenTargetDropdown();
  dismissHoverDirectoryTree();
  closeContextMenu();
  closeSelectionAction();
}

function handleGlobalEscape(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeDirectoryOpenTargetDropdown();
    dismissHoverDirectoryTree();
    closeContextMenu();
    closeSelectionAction();
  }
}

function handleFileReaderLineWrapShortcut(event: KeyboardEvent) {
  if (
    event.code !== "KeyZ"
    || !event.altKey
    || event.ctrlKey
    || event.metaKey
    || event.shiftKey
    || activeTab.value?.kind !== "code"
  ) return;
  const target = event.target instanceof Element ? event.target : null;
  if (target?.closest("input, textarea, [contenteditable='true']")) return;
  event.preventDefault();
  toggleFileReaderLineWrapEnabled();
}

watch(fileReaderLineWrapEnabled, () => {
  void nextTick(() => remeasureVirtualCodeRows());
});

// ==================== Lifecycle ====================

onMounted(async () => {
  window.addEventListener("resize", updateAddressScrollState);
  window.addEventListener("pointerdown", handleGlobalPointerDown);
  window.addEventListener("keydown", handleGlobalEscape);
  window.addEventListener("keydown", handleFileReaderLineWrapShortcut);
  measureFileReaderLayoutWidth();
  if (typeof ResizeObserver !== "undefined" && fileReaderLayoutRoot.value) {
    fileReaderLayoutResizeObserver = new ResizeObserver(() => {
      if (activeDirectoryTreeResize.value) return;
      measureFileReaderLayoutWidth();
      setDirectoryTreeWidth(directoryTreeWidth.value);
    });
    fileReaderLayoutResizeObserver.observe(fileReaderLayoutRoot.value);
  }
  void loadDirectoryOpenTargets();
  void startFileReaderWatchListener();
  scheduleFileReaderWatchTargetUpdate();
  if (props.enableGlobalDrop === false) return;
  try {
    unlistenFileDrop = await listenCurrentTransportFileDrop((payload) => {
      if (payload.type === "enter" || payload.type === "over") {
        fileDragActive.value = true;
        return;
      }
      fileDragActive.value = false;
      if (payload.type === "drop") {
        void openDroppedPaths(payload.paths);
      }
    });
  } catch (error) {
    console.error("[文件阅读器] 注册拖入打开失败", error);
  }
});

onBeforeUnmount(() => {
  emit("clearContextReferences");
  if (directoryTreeResizeRaf) {
    window.cancelAnimationFrame(directoryTreeResizeRaf);
    directoryTreeResizeRaf = 0;
  }
  pendingDirectoryTreeResizeWidth = null;
  window.removeEventListener("resize", updateAddressScrollState);
  window.removeEventListener("pointerdown", handleGlobalPointerDown);
  window.removeEventListener("keydown", handleGlobalEscape);
  window.removeEventListener("keydown", handleFileReaderLineWrapShortcut);
  stopDirectoryTreeResize();
  fileReaderLayoutResizeObserver?.disconnect();
  fileReaderLayoutResizeObserver = null;
  if (watchTargetUpdateTimer) window.clearTimeout(watchTargetUpdateTimer);
  if (autoRefreshFileTimer) window.clearTimeout(autoRefreshFileTimer);
  if (autoRefreshDirectoryTimer) window.clearTimeout(autoRefreshDirectoryTimer);
  if (visibleRangeCaptureTimer) window.clearTimeout(visibleRangeCaptureTimer);
  pendingAutoRefreshDirectoryPaths.clear();
  stopFileReaderWatchListener();
  void updateTransportFileReaderWatchTargets({
    sessionId: fileReaderWatchSessionId.value,
    targets: [],
  }).catch(() => {});
  unlistenFileDrop?.();
});

// ==================== Expose ====================

defineExpose({
  openPath,
  setActiveTab,
  closeTab,
  closeTabsToLeftOf,
  closeTabsToRightOf,
  closeOtherTabs,
  openDirectoryTree,
  closeDirectoryTree,
  openGitPanel,
  whenSessionRestored,
  tabs,
  activePath,
  directoryRootPath,
});
</script>

<style scoped>
.file-reader-address-scroll {
  scrollbar-width: none;
}
.file-reader-address-scroll::-webkit-scrollbar {
  display: none;
}
.file-reader-scroll-container {
  scrollbar-width: none;
}
.file-reader-scroll-container::-webkit-scrollbar {
  display: none;
}
.file-reader-tree-icon {
  filter:
    drop-shadow(0 0 0.35px rgb(255 255 255 / 0.45))
    drop-shadow(0 0 0.45px rgb(15 23 42 / 0.22));
}
.file-reader-resize-handle {
  width: 8px;
  cursor: col-resize;
  background: transparent;
  transition: background-color 160ms ease, opacity 160ms ease;
  opacity: 1;
}
.file-reader-resize-handle:hover,
.file-reader-resize-handle:focus-visible,
.file-reader-resize-handle.is-active {
  opacity: 1;
  background:
    linear-gradient(
      to left,
      transparent 0,
      transparent 2px,
      color-mix(in srgb, var(--color-primary) 70%, transparent) 2px,
      color-mix(in srgb, var(--color-primary) 70%, transparent) 5px,
      transparent 5px,
      transparent 100%
    );
  outline: none;
}
.file-reader-content-scroller {
  scrollbar-width: thin;
  scrollbar-color: color-mix(in srgb, currentColor 28%, transparent) transparent;
}
.file-reader-content-scroller::-webkit-scrollbar {
  display: block;
  width: 10px;
  height: 10px;
}
.file-reader-content-scroller::-webkit-scrollbar-thumb {
  background-color: color-mix(in srgb, currentColor 28%, transparent);
  border-radius: 999px;
}
.file-reader-content-scroller::-webkit-scrollbar-track {
  background: transparent;
}
.file-reader-address-scrollbar-thumb {
  opacity: 0;
  transition: opacity 160ms ease;
}
.file-reader-address-scroll:hover + div .file-reader-address-scrollbar-thumb,
.file-reader-address-scroll:focus-within + div .file-reader-address-scrollbar-thumb {
  opacity: 1;
}
.file-reader-content :deep(.markdown-body),
.file-reader-content :deep(.markstream-body) {
  max-width: none;
}
.file-reader-content :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
  color: inherit;
  font-family: inherit;
  font-size: var(--app-text-base-size);
  line-height: 1.65;
}
.file-reader-content :deep(.ecall-markdown-content :where(hr,.hr-node)) {
  margin: 0.75rem 0;
}
.file-reader-content :deep(.ecall-markdown-content :where(table,.table-node)) {
  width: 100%;
  font-size: var(--app-text-sm-size);
}
.file-reader-media-stage {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  padding: 1rem;
}
.file-reader-media-image {
  display: block;
  max-width: 100%;
  max-height: calc(100vh - 6rem);
  width: auto;
  height: auto;
  object-fit: contain;
}
.file-reader-media-video {
  display: block;
  width: 100%;
  height: 100%;
  max-height: calc(100vh - 4rem);
  object-fit: contain;
  background: #000;
}
.file-reader-raw-main {
  min-height: 0;
}
.file-reader-raw-scroller {
  min-height: 100%;
  overflow: auto;
  background: transparent;
}
.file-reader-raw-pre {
  min-height: 100%;
  margin: 0;
  padding: 0.75rem 1rem;
  white-space: pre;
  font-family: var(--app-code-font-family);
  color: inherit;
  background: transparent;
}
.file-reader-code-virtual-scroller {
  min-height: 100%;
  overflow: auto;
}
.file-reader-code-virtual-scroller-raw {
  overflow-x: auto;
}
.file-reader-code-virtual-scroller-shiki {
  background: var(--color-base-100);
  scrollbar-width: none;
  -ms-overflow-style: none;
}
.file-reader-code-virtual-scroller-shiki::-webkit-scrollbar {
  width: 0;
  height: 0;
}
.file-reader-code-virtual-scroller-wrap {
  overflow-x: hidden;
}
.file-reader-code-virtual-scroller-nowrap {
  overflow: auto;
}
.file-reader-code-wrap {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.file-reader-code-virtual-canvas {
  min-width: 100%;
}
.file-reader-code-virtual-row {
  position: relative;
  width: 100%;
}
.file-reader-code-virtual-block {
  width: 100%;
  min-width: 0;
  font-family: var(--app-code-font-family);
  font-size: var(--app-text-sm-size);
  line-height: 21px;
}
.file-reader-code-virtual-line {
  display: grid;
  grid-template-columns: calc(var(--file-reader-code-gutter-ch, 2) * 1ch + 1rem) minmax(0, 1fr);
  align-items: stretch;
  min-width: 0;
}
.file-reader-code-virtual-line-number {
  position: sticky;
  left: 0;
  z-index: 2;
  width: 100%;
  box-sizing: border-box;
  min-height: 21px;
  padding-right: 0.25rem;
  color: rgb(100 116 139 / 0.92);
  line-height: inherit;
  text-align: right;
  user-select: none;
}
.file-reader-code-virtual-gutter-raw {
  background: color-mix(in srgb, var(--color-base-200) 60%, var(--color-base-100));
}
.file-reader-code-virtual-gutter-shiki {
  background: color-mix(in srgb, var(--color-base-200) 60%, var(--color-base-100));
}
.file-reader-code-virtual-line-content {
  display: block;
  width: 100%;
  min-width: 0;
  min-height: 21px;
  padding: 0 8px;
  font: inherit;
  line-height: inherit;
  white-space: pre;
  overflow-wrap: normal;
  word-break: normal;
}
.file-reader-code-virtual-scroller-wrap .file-reader-code-virtual-line-content {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.file-reader-code-virtual-line-content :deep(span) {
  font-family: inherit !important;
}
.file-reader-code-virtual-line-content :deep(.file-reader-code-empty-line) {
  display: inline-block;
  width: 0;
  opacity: 0;
  pointer-events: none;
}

/* 侧边栏（文件/Git）伸缩与内容切换：自写 Transition，不用 daisyUI drawer（overlay 语义与现有 flex 挤压布局冲突） */
.file-reader-aside-enter-active,
.file-reader-aside-leave-active {
  overflow: hidden;
  transition:
    width 220ms cubic-bezier(0.2, 0, 0, 1),
    opacity 180ms ease;
}
.file-reader-aside-enter-from,
.file-reader-aside-leave-to {
  width: 0 !important;
  opacity: 0;
}
.file-reader-aside-panel {
  overflow: hidden;
}
.file-reader-aside-content-enter-active {
  transition:
    opacity 160ms ease,
    transform 160ms ease;
}
.file-reader-aside-content-leave-active {
  transition:
    opacity 120ms ease,
    transform 120ms ease;
}
.file-reader-aside-content-enter-from {
  opacity: 0;
  transform: translateX(8px);
}
.file-reader-aside-content-leave-to {
  opacity: 0;
  transform: translateX(-8px);
}
@media (prefers-reduced-motion: reduce) {
  .file-reader-aside-enter-active,
  .file-reader-aside-leave-active,
  .file-reader-aside-content-enter-active,
  .file-reader-aside-content-leave-active {
    transition: none !important;
  }
  .file-reader-aside-enter-from,
  .file-reader-aside-leave-to,
  .file-reader-aside-content-enter-from,
  .file-reader-aside-content-leave-to {
    transform: none !important;
  }
}
</style>
