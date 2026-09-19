<template>
  <div class="flex h-full min-h-0 w-full flex-col bg-base-200/35 text-base-content" @click="closeCommitCard">
    <!-- 操作提示：进行中 / 成功 / 失败 -->
    <div v-if="toast" class="pointer-events-none absolute inset-x-0 top-10 z-50 flex justify-center px-4">
      <div
        class="pointer-events-auto max-w-full rounded px-3 py-1.5 text-xs shadow-lg"
        :class="toast.kind === 'error' ? 'bg-error text-error-content' : toast.kind === 'success' ? 'bg-success text-success-content' : 'bg-neutral text-neutral-content'"
      >
        {{ toast.message }}
      </div>
    </div>

    <!-- 未检测到 git 或非仓库（且没有可显示的仓库列表时） -->
    <div v-if="detectError && repos.length === 0 && !reposLoading" class="flex h-full min-h-0 flex-col items-center justify-center gap-2 px-4 text-center">
      <SquareTerminal class="h-8 w-8 opacity-50" />
      <div class="text-sm font-medium">{{ detectError }}</div>
      <div v-if="detectChecked" class="max-w-56 text-xs leading-relaxed text-base-content/55">
        {{ t('gitPanel.notRepositoryHint') }}
      </div>
    </div>

    <template v-else>
      <!-- 仓库栏：折叠条（标题=当前仓库名）+ 仓库列表（懒加载，可切换） -->
      <div class="flex min-h-0 shrink-0 flex-col overflow-hidden">
        <GitSectionBar v-model="repoCollapsed">
          <template #default>
            <span class="max-w-40 truncate text-xs font-medium opacity-70">{{ currentRepoName }}</span>
          </template>
          <template #actions>
            <button
              type="button"
              class="btn btn-ghost btn-xs h-6 min-h-6 w-6 px-0"
              :title="t('gitPanel.refresh')"
              :disabled="reposLoading || busy"
              @click="refreshRepos"
            >
              <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': reposLoading }" />
            </button>
          </template>
        </GitSectionBar>
        <div v-if="!repoCollapsed" class="git-panel-scroller max-h-44 min-h-0 overflow-y-auto py-1">
          <div v-if="reposLoading" class="px-3 py-2 text-xs opacity-50">{{ t('gitPanel.loading') }}</div>
          <template v-else>
            <button
              v-for="repo in repos"
              :key="repo.path"
              type="button"
              class="flex w-full items-center gap-1.5 rounded px-2 py-1 text-left text-xs hover:bg-base-300/40"
              :class="{ 'bg-primary/10 text-primary': isCurrentRepo(repo.path) }"
              :disabled="busy"
              @click="switchRepo(repo.path)"
            >
              <GitBranch class="h-3 w-3 shrink-0 opacity-60" />
              <span class="min-w-0 flex-1 truncate">{{ repo.name }}</span>
              <span v-if="isCurrentRepo(repo.path)" class="shrink-0 opacity-50">{{ t('gitPanel.currentRepo') }}</span>
            </button>
            <div v-if="repos.length === 0" class="px-3 py-2 text-xs opacity-50">{{ t('gitPanel.noRepos') }}</div>
          </template>
        </div>
      </div>

      <!-- 上栏：折叠条 + 提交输入框 + 更改/暂存双树 -->
      <div class="flex min-h-0 flex-col overflow-hidden" :class="{ 'flex-1': !changesCollapsed }">
        <GitSectionBar v-model="changesCollapsed">
          <template #default>
            <span class="text-xs font-medium opacity-70">
              {{ t('gitPanel.changes') }}
              <span v-if="statusTruncated" class="tabular-nums opacity-50">1000+</span>
              <span v-else-if="totalChanges > 0" class="tabular-nums opacity-50">{{ totalChanges }}</span>
            </span>
          </template>
          <template #actions>
            <button
              type="button"
              class="btn btn-ghost btn-xs h-6 min-h-6 w-6 px-0"
              :title="changesViewMode === 'tree' ? t('gitPanel.listView') : t('gitPanel.treeView')"
              :disabled="busy"
              @click="toggleChangesViewMode"
            >
              <ListTree v-if="changesViewMode === 'tree'" class="h-3.5 w-3.5" />
              <Rows3 v-else class="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-xs h-6 min-h-6 w-6 px-0"
              :title="t('gitPanel.refresh')"
              :disabled="busy"
              @click="refreshChanges"
            >
              <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': changesRefreshing }" />
            </button>
          </template>
        </GitSectionBar>
        <div v-if="!changesCollapsed" class="flex min-h-0 flex-1 flex-col">
        <!-- 提交输入框（单行起，随内容增高，外观同 input） -->
        <div class="shrink-0 border-b border-base-300 bg-base-200/35 p-2">
          <textarea
            ref="commitInputRef"
            v-model="commitMessage"
            class="textarea textarea-sm w-full git-panel-commit-input text-xs"
            :placeholder="t('gitPanel.commitMessagePlaceholder')"
            rows="1"
            @input="autoGrowCommitInput"
          ></textarea>
          <div class="join mt-1.5 w-full">
            <button
              type="button"
              class="btn btn-primary btn-xs join-item w-[80%]"
              :disabled="busy || !commitMessage.trim() || stagedEntries.length === 0"
              @click="runCommit(false)"
            >
              {{ t('gitPanel.commit') }}
            </button>
            <button
              type="button"
              class="btn btn-xs join-item w-[20%] border-base-300 bg-base-100"
              :disabled="busy || !commitMessage.trim() || stagedEntries.length === 0"
              :title="t('gitPanel.amend')"
              @click="runCommit(true)"
            >
              修正
            </button>
          </div>
        </div>

        <!-- 暂存更改 / 更改 双树 -->
        <div ref="changesScroller" class="git-panel-scroller min-h-0 flex-1 overflow-y-auto py-1">
          <GitChangesGroup
            :title="t('gitPanel.stagedChanges')"
            :entries="stagedEntries"
            :busy="busy"
            :mode="changesViewMode"
            :total-count="stagedTotal"
            action-kind="unstage"
            :action-title="t('gitPanel.unstage')"
            :discard-title="t('gitPanel.discard')"
            :expand-title="t('gitPanel.expandDirectory')"
            :collapse-title="t('gitPanel.collapseDirectory')"
            :collapse-all-title="t('gitPanel.collapseAll')"
            :highlight-path="lastClickedDiffPath"
            @open-diff="openDiff"
            @action="unstagePaths"
            @discard="discardPaths"
          />
          <GitChangesGroup
            :title="t('gitPanel.changes')"
            :entries="unstagedEntries"
            :busy="busy"
            :mode="changesViewMode"
            :total-count="unstagedTotal"
            action-kind="stage"
            :action-title="t('gitPanel.stage')"
            :discard-title="t('gitPanel.discard')"
            :expand-title="t('gitPanel.expandDirectory')"
            :collapse-title="t('gitPanel.collapseDirectory')"
            :collapse-all-title="t('gitPanel.collapseAll')"
            :highlight-path="lastClickedDiffPath"
            @open-diff="openDiff"
            @action="stagePaths"
            @discard="discardPaths"
          />
          <div v-if="!busy && totalChanges === 0" class="px-3 py-6 text-center text-xs text-base-content/50">
            {{ repoRoot ? t('gitPanel.noChanges') : t('gitPanel.selectRepoHint') }}
          </div>
        </div>
        </div>
      </div>

      <!-- 分界线：仅两栏都展开时显示，独立于折叠条 -->
      <GitResizeHandle v-if="!changesCollapsed && !historyCollapsed" @resize-start="onHistoryResizeStart" @resize="onHistoryResize" />

      <!-- 下栏：折叠条 + tab 内容 + 底部标签页 -->
      <div
        ref="historySectionRef"
        class="relative flex min-h-0 flex-col"
        :class="{ 'flex-1': !historyCollapsed && (historyHeight === null || changesCollapsed) }"
        :style="!historyCollapsed && !changesCollapsed && historyHeight !== null ? { height: `${historyHeight}px` } : undefined"
      >
        <!-- 下栏折叠条：折叠按钮 + 三选名称（折叠时提示） + 分支名 + 刷新/同步/拉/推 -->
        <GitSectionBar v-model="historyCollapsed">
          <template #default>
            <span v-if="historyCollapsed" class="flex items-center gap-1.5 text-xs font-medium opacity-70">
              <component :is="activeHistoryTab.icon" class="h-3.5 w-3.5 shrink-0 opacity-60" />
              <span class="truncate">{{ activeHistoryTab.label }}</span>
            </span>
          </template>
          <template #actions>
            <button
              v-if="activeGitTab === 'commits'"
              type="button"
              class="flex h-6 min-w-0 items-center gap-1 rounded px-1 text-xs font-medium hover:bg-base-300/40"
              :disabled="busy"
              @click="toggleBranchPicker"
            >
              <GitBranch class="h-3.5 w-3.5 shrink-0 opacity-70" />
              <span class="min-w-0 max-w-28 truncate">{{ currentBranch || t('gitPanel.detachedHead') }}</span>
              <ChevronUp class="h-3 w-3 shrink-0 opacity-50" :class="{ 'rotate-180': !branchPickerOpen }" />
            </button>
            <!-- 分支切换下拉（absolute 相对折叠条）：分组头为树根 + 分支子节点 -->
            <div v-if="branchPickerOpen" class="absolute left-0 right-0 top-full z-20 max-h-64 overflow-y-auto border border-base-300 bg-base-100 p-1 shadow-lg">
              <div v-if="branchPickerLoading" class="px-2 py-2 text-xs opacity-50">{{ t('gitPanel.loading') }}</div>
              <GitTree v-else :nodes="branchPickerTreeNodes" default-expanded :default-collapsed-keys="branchCollapsedKeys" @row-click="onBranchPickerRowClick">
                <template #row="{ row }">
                  <!-- 分组头（树根：本地分支/远程分支） -->
                  <template v-if="row.node.data.kind === 'header'">
                    <span class="min-w-0 truncate font-medium opacity-60">{{ row.node.data.text }}</span>
                  </template>
                  <!-- 目录（名字里按 / 折叠出的分组） -->
                  <template v-else-if="row.node.data.kind === 'folder'">
                    <Folder class="h-3.5 w-3.5 shrink-0 opacity-60" />
                    <span class="min-w-0 flex-1 truncate font-medium opacity-70">{{ row.node.data.name }}</span>
                  </template>
                  <!-- 本地分支 -->
                  <template v-else-if="row.node.data.kind === 'branch'">
                    <GitBranch class="h-3.5 w-3.5 shrink-0" :class="row.node.data.branch.isCurrent ? 'text-primary' : 'opacity-60'" />
                    <span class="min-w-0 flex-1 truncate">{{ row.node.data.branch.name }}</span>
                    <span v-if="row.node.data.branch.isCurrent" class="shrink-0 opacity-50">{{ t('gitPanel.current') }}</span>
                  </template>
                  <!-- 远程分支 -->
                  <template v-else-if="row.node.data.kind === 'remote-branch'">
                    <Cloud class="h-3 w-3 shrink-0 opacity-60" />
                    <span class="min-w-0 flex-1 truncate">{{ row.node.data.branch.name }}</span>
                  </template>
                </template>
              </GitTree>
            </div>
            <button v-if="activeGitTab === 'commits'" class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0" type="button" :title="t('gitPanel.refresh')" :disabled="busy" @click="refreshHistory">
              <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': busy }" />
            </button>
            <button v-if="activeGitTab === 'commits'" class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0" type="button" :title="t('gitPanel.sync')" :disabled="busy" @click="runSync">
              <CloudSync class="h-3.5 w-3.5" :class="{ 'animate-spin': busy }" />
            </button>
            <button v-if="activeGitTab === 'commits'" class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0" type="button" :title="t('gitPanel.pull')" :disabled="busy" @click="runPull">
              <ArrowDownToLine class="h-3.5 w-3.5" />
            </button>
            <button v-if="activeGitTab === 'commits'" class="btn btn-ghost btn-xs h-6 min-h-6 w-6 shrink-0 px-0" type="button" :title="t('gitPanel.push')" :disabled="busy" @click="runPush">
              <ArrowUpFromLine class="h-3.5 w-3.5" />
            </button>
          </template>
        </GitSectionBar>
        <div v-if="!historyCollapsed" class="flex min-h-0 flex-1 flex-col">
          <div class="min-h-0 flex-1 overflow-hidden">
          <!-- 提交 tab：commit 表（工具条已上移到折叠条） -->
          <div v-if="activeGitTab === 'commits'" class="flex h-full min-h-0 flex-col">
            <div ref="historyScroller" class="git-panel-scroller min-h-0 flex-1 overflow-y-auto py-1">
              <div v-if="logEntries.length === 0 && !busy" class="px-3 py-6 text-center text-xs text-base-content/50">
                {{ t('gitPanel.noCommits') }}
              </div>
              <GitTree
                :nodes="commitTreeNodes"
                :indent="0"
                @expand="onCommitExpand"
                @row-click="onCommitRowClick"
              >
                <template #row="{ row, expanded, toggle }">
                  <!-- 父行：提交（整行点击展开懒加载 diff 文件列表） -->
                  <template v-if="row.node.data.kind === 'commit'">
                    <!-- 时间线列：VS Code 泳道图（SVG 由算法生成，全部为内部计算值） -->
                    <span class="contents" v-html="row.node.data.graphSvg" />
                    <span class="min-w-0 flex-1 truncate" :title="`${row.node.data.entry.message} · ${row.node.data.entry.author}`">
                      {{ row.node.data.entry.message }}
                      <span class="text-caption opacity-50"> {{ row.node.data.entry.author }}</span>
                    </span>
                    <!-- 分支终点标签：颜色与图中该 ref 的线色一致 -->
                    <span
                      v-for="ref in row.node.data.refs"
                      :key="ref.name"
                      class="max-w-24 min-w-0 truncate rounded px-1 text-caption font-medium"
                      :style="{ color: graphColor(ref.colorIndex), backgroundColor: graphColor(ref.colorIndex) + '1A' }"
                      :title="ref.name"
                    >{{ ref.name }}</span>
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs h-5 min-h-5 w-5 shrink-0 px-0"
                      :title="t('gitPanel.moreActions')"
                      @click.stop="openCommitMenu(row.node.data.entry, $event)"
                    >
                      <MoreHorizontal class="h-3.5 w-3.5" />
                    </button>
                  </template>
                  <!-- 子节点：查看全部更改 -->
                  <template v-else-if="row.node.data.kind === 'all'">
                    <CommitGraphLine :x="row.node.data.lineX" :color="row.node.data.lineColor" :width="row.node.data.graphWidth" />
                    <button
                      type="button"
                      class="flex shrink-0 cursor-pointer items-center gap-1 font-medium text-primary"
                      :disabled="busy"
                      @click.stop="openCommitAllDiff(row.node.data.hash)"
                    >
                      <Files class="h-3 w-3 shrink-0" />
                      <span class="min-w-0 truncate">{{ t('gitPanel.viewCommitChanges') }}</span>
                    </button>
                  </template>
                  <!-- 子节点：加载中 -->
                  <template v-else-if="row.node.data.kind === 'loading'">
                    <CommitGraphLine :x="row.node.data.lineX" :color="row.node.data.lineColor" :width="row.node.data.graphWidth" />
                    <span class="opacity-50">{{ t('gitPanel.loading') }}</span>
                  </template>
                  <!-- 子节点：无文件 -->
                  <template v-else-if="row.node.data.kind === 'empty'">
                    <CommitGraphLine :x="row.node.data.lineX" :color="row.node.data.lineColor" :width="row.node.data.graphWidth" />
                    <span class="opacity-50">{{ t('gitPanel.noCommitFiles') }}</span>
                  </template>
                  <!-- 子节点：diff 文件（整行点击打开 diff） -->
                  <template v-else>
                    <CommitGraphLine :x="row.node.data.lineX" :color="row.node.data.lineColor" :width="row.node.data.graphWidth" />
                    <span class="shrink-0 font-mono text-caption font-bold" :class="commitFileStatusClass(row.node.data.file.status)">{{ commitFileStatusLabel(row.node.data.file.status) }}</span>
                    <span class="min-w-0 truncate">{{ row.node.data.file.path }}</span>
                  </template>
                </template>
              </GitTree>
              <!-- 滚动到底自动加载更多 -->
              <div v-if="logHasMore" ref="logSentinel" class="flex justify-center px-2 py-2">
                <span v-if="logLoadingMore" class="loading loading-spinner loading-xs opacity-60" />
                <span v-else class="text-xs opacity-40">{{ t('gitPanel.loadMore') }}</span>
              </div>
            </div>
          </div>

          <!-- 储藏 tab -->
          <div v-else-if="activeGitTab === 'stashes'" class="flex h-full min-h-0 flex-col">
            <div class="shrink-0 border-b border-base-300 p-2">
              <div class="flex items-center gap-1.5">
                <input
                  v-model="stashMessage"
                  class="input input-sm input-bordered min-w-0 flex-1 bg-base-100 text-xs"
                  type="text"
                  :placeholder="t('gitPanel.stashMessagePlaceholder')"
                  @keydown.enter="runStashCreate(false)"
                />
                <button type="button" class="btn btn-sm shrink-0" :disabled="busy || !stashMessage.trim()" @click="runStashCreate(false)">
                  {{ t('gitPanel.stashChanges') }}
                </button>
                <button type="button" class="btn btn-sm shrink-0" :disabled="busy || !stashMessage.trim()" @click="runStashCreate(true)">
                  {{ t('gitPanel.stashStaged') }}
                </button>
              </div>
            </div>
            <div ref="stashScroller" class="git-panel-scroller min-h-0 flex-1 overflow-y-auto py-1">
              <div v-if="stashList.length === 0" class="px-3 py-6 text-center text-xs text-base-content/50">
                {{ t('gitPanel.noStashes') }}
              </div>
              <GitTree
                :nodes="stashTreeNodes"
                @expand="onStashExpand"
                @row-click="onStashRowClick"
              >
                <template #row="{ row }">
                  <!-- 父行：储藏（整行点击展开懒加载 diff 文件列表） -->
                  <template v-if="row.node.data.kind === 'stash'">
                    <span class="shrink-0 font-mono opacity-60">{{ row.node.data.index }}</span>
                    <span class="min-w-0 flex-1 truncate opacity-80">{{ row.node.data.stash.message }}</span>
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs h-5 min-h-5 w-5 shrink-0 px-0"
                      :title="t('gitPanel.moreActions')"
                      @click.stop="openStashMenu(row.node.data.stash, $event)"
                    >
                      <MoreHorizontal class="h-3.5 w-3.5" />
                    </button>
                  </template>
                  <!-- 子节点：加载中 -->
                  <template v-else-if="row.node.data.kind === 'loading'">
                    <span class="opacity-50">{{ t('gitPanel.loading') }}</span>
                  </template>
                  <!-- 子节点：无文件 -->
                  <template v-else-if="row.node.data.kind === 'empty'">
                    <span class="opacity-50">{{ t('gitPanel.noCommitFiles') }}</span>
                  </template>
                  <!-- 子节点：diff 文件（整行点击打开 diff） -->
                  <template v-else>
                    <span class="shrink-0 font-mono text-caption font-bold" :class="commitFileStatusClass(row.node.data.file.status)">{{ commitFileStatusLabel(row.node.data.file.status) }}</span>
                    <span class="min-w-0 truncate">{{ row.node.data.file.path }}</span>
                  </template>
                </template>
              </GitTree>
              <!-- 滚动到底自动加载更多 -->
              <div v-if="stashHasMore" ref="stashSentinel" class="flex justify-center px-2 py-2">
                <span class="text-xs opacity-40">{{ t('gitPanel.loadMore') }}</span>
              </div>
            </div>
          </div>

          <!-- 分支 tab -->
          <div v-else class="flex h-full min-h-0 flex-col gap-2 p-2">
            <div class="flex items-center gap-1.5">
              <input
                v-model="newBranchName"
                class="input input-sm input-bordered min-w-0 flex-1 bg-base-100 text-xs"
                type="text"
                :placeholder="t('gitPanel.newBranchPlaceholder')"
                @keydown.enter="runBranchCreate"
              />
              <button type="button" class="btn btn-sm" :disabled="busy || !newBranchName.trim()" @click="runBranchCreate">
                <Plus class="h-3.5 w-3.5" />
              </button>
            </div>
            <div ref="branchesScroller" class="git-panel-scroller min-h-0 flex-1 overflow-y-auto">
              <GitTree :nodes="branchTreeNodes" default-expanded :default-collapsed-keys="branchCollapsedKeys" @row-click="onBranchRowClick">
                <template #row="{ row }">
                  <!-- 分组头（树根：本地分支/远程分支/远程） -->
                  <template v-if="row.node.data.kind === 'header'">
                    <span class="min-w-0 truncate font-medium opacity-60">{{ row.node.data.text }}</span>
                  </template>
                  <!-- 目录（名字里按 / 折叠出的分组） -->
                  <template v-else-if="row.node.data.kind === 'folder'">
                    <Folder class="h-3.5 w-3.5 shrink-0 opacity-60" />
                    <span class="min-w-0 flex-1 truncate font-medium opacity-70">{{ row.node.data.name }}</span>
                  </template>
                  <!-- 本地分支 -->
                  <template v-else-if="row.node.data.kind === 'branch'">
                    <GitBranch class="h-3.5 w-3.5 shrink-0" :class="row.node.data.branch.isCurrent ? 'text-primary' : 'opacity-60'" />
                    <span class="min-w-0 truncate">{{ row.node.data.label }}</span>
                    <span class="shrink-0 text-caption opacity-45">{{ formatRecentRelativeTime(row.node.data.branch.committerDate, relativeNowTick, t) }}</span>
                    <button v-if="!row.node.data.branch.isCurrent" type="button" class="btn btn-ghost btn-xs ml-auto h-4 min-h-4 w-4 shrink-0 px-0 opacity-70 hover:opacity-100" :title="t('gitPanel.checkoutBranch')" :disabled="busy" @click.stop="runCheckoutBranch(row.node.data.branch.name)">
                      <ArrowRightLeft class="h-3 w-3" />
                    </button>
                  </template>
                  <!-- 远程分支 -->
                  <template v-else-if="row.node.data.kind === 'remote-branch'">
                    <Cloud class="h-3 w-3 shrink-0 opacity-60" />
                    <span class="min-w-0 truncate">{{ row.node.data.label }}</span>
                    <span class="shrink-0 text-caption opacity-45">{{ formatRecentRelativeTime(row.node.data.branch.committerDate, relativeNowTick, t) }}</span>
                    <button type="button" class="btn btn-ghost btn-xs ml-auto h-4 min-h-4 w-4 shrink-0 px-0 opacity-70 hover:opacity-100" :title="t('gitPanel.checkoutBranch')" :disabled="busy" @click.stop="runCheckoutBranch(row.node.data.branch.name)">
                      <ArrowRightLeft class="h-3 w-3" />
                    </button>
                  </template>
                  <!-- 远程 URL -->
                  <template v-else>
                    <Cloud class="h-3 w-3 shrink-0 opacity-60" />
                    <span class="shrink-0 font-medium">{{ row.node.data.remote.name }}</span>
                    <span class="min-w-0 truncate font-mono opacity-75">{{ row.node.data.remote.url }}</span>
                  </template>
                </template>
              </GitTree>
              <!-- 滚动到底自动加载更多 -->
              <div v-if="branchHasMore" ref="branchesSentinel" class="flex justify-center px-2 py-2">
                <span class="text-xs opacity-40">{{ t('gitPanel.loadMore') }}</span>
              </div>
            </div>
          </div>
        </div>

          <!-- 底部标签页：提交 / 储藏 / 分支 -->
          <div class="flex h-8 shrink-0 items-center gap-1 border-t border-base-300 bg-base-200/35 px-2">
          <button
            v-for="item in gitPanelTabs"
            :key="item.key"
            type="button"
            class="btn btn-ghost btn-xs h-6 min-h-6 flex-1 justify-center gap-1 px-1 font-medium"
            :class="activeGitTab === item.key ? 'bg-base-100 text-primary shadow-sm' : 'text-base-content/60 hover:bg-base-300/40'"
            @click="selectGitTab(item.key)"
          >
              <component :is="item.icon" class="h-3.5 w-3.5" />
              <span class="truncate">{{ item.label }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- commit 操作菜单卡（行尾更多按钮打开） -->
      <div
        v-if="commitCard.entry"
        ref="commitCardRef"
        class="fixed z-50 w-72 overflow-hidden rounded-lg border border-base-300 bg-base-100 shadow-xl"
        :style="{ left: `${commitCard.x}px`, top: `${commitCard.y}px` }"
        @click.stop
        @contextmenu.prevent
      >
        <div class="border-b border-base-300 bg-base-200/50 px-3 py-2">
          <div class="git-panel-scroller max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-xs font-medium">{{ commitCard.entry.message }}</div>
        </div>
        <div class="px-3 py-2 text-xs opacity-70">
          <div>{{ commitCard.entry.author }}</div>
          <div class="mt-0.5">{{ commitCard.entry.date }}</div>
        </div>
        <div class="flex flex-wrap gap-1 border-t border-base-300 px-2 py-2">
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 gap-1 font-mono" @click="copyCommitHash">
            <Copy class="h-3 w-3" />
            {{ commitCard.entry.shortHash }}
          </button>
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 gap-1" @click="copyCommitMessage">
            <Copy class="h-3 w-3" />
            {{ t('gitPanel.copyMessage') }}
          </button>
        </div>
        <div class="flex flex-col gap-1 border-t border-base-300 px-2 py-2">
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 justify-start gap-1.5 px-2" @click="createBranchFromCommit">
            <GitBranch class="h-3 w-3" />
            <span class="truncate">{{ t('gitPanel.createBranchFromCommit') }}</span>
          </button>
          <button
            v-if="isLatestCommit"
            type="button"
            class="btn btn-ghost btn-xs h-6 min-h-6 justify-start gap-1.5 px-2"
            @click="resetSoftCommit"
          >
            <Undo2 class="h-3 w-3" />
            <span class="truncate">{{ t('gitPanel.resetSoftCommit') }}</span>
          </button>
        </div>
      </div>

      <!-- stash 操作菜单卡（行尾更多按钮打开） -->
      <div
        v-if="stashMenu.entry"
        ref="stashMenuRef"
        class="fixed z-50 w-56 overflow-hidden rounded-lg border border-base-300 bg-base-100 shadow-xl"
        :style="{ left: `${stashMenu.x}px`, top: `${stashMenu.y}px` }"
        @click.stop
        @contextmenu.prevent
      >
        <div class="flex flex-col gap-1 border-b border-base-300 px-2 py-2">
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 justify-start gap-1.5 px-2" :disabled="busy" @click="stashApplyFromMenu">
            <Upload class="h-3 w-3" />
            <span class="truncate">{{ t('gitPanel.stashApply') }}</span>
          </button>
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 justify-start gap-1.5 px-2" :disabled="busy" @click="stashPopFromMenu">
            <Upload class="h-3 w-3" />
            <span class="truncate">{{ t('gitPanel.stashPop') }}</span>
          </button>
        </div>
        <div class="flex flex-col gap-1 px-2 py-2">
          <button type="button" class="btn btn-ghost btn-xs h-6 min-h-6 justify-start gap-1.5 px-2 text-error" :disabled="busy" @click="stashDropFromMenu">
            <Trash2 class="h-3 w-3" />
            <span class="truncate">{{ t('gitPanel.stashDrop') }}</span>
          </button>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  ArrowDownToLine,
  ArrowRightLeft,
  ArrowUpFromLine,
  ChevronUp,
  Cloud,
  CloudSync,
  Copy,
  Files,
  Folder,
  GitBranch,
  GitCommitHorizontal,
  History,
  ListTree,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Rows3,
  SquareTerminal,
  Trash2,
  Undo2,
  Upload,
} from "@lucide/vue";
import { useI18n } from "vue-i18n";
import {
  appendTransportProbeLog,
  gitPanelBranchCreate,
  gitPanelBranchList,
  gitPanelCheckout,
  gitPanelCheckoutCheck,
  gitPanelCommit,
  gitPanelCommitFiles,
  gitPanelDiscover,
  gitPanelDiscard,
  gitPanelLog,
  gitPanelPull,
  gitPanelPush,
  gitPanelRemoteList,
  gitPanelResetSoft,
  gitPanelStage,
  gitPanelStashCreate,
  gitPanelStashApply,
  gitPanelStashDrop,
  gitPanelStashFiles,
  gitPanelStashList,
  gitPanelStashPop,
  gitPanelSync,
  gitPanelUnstage,
  type GitPanelBranchEntry,
  type GitPanelCommitFileEntry,
  type GitPanelLogEntry,
  type GitPanelRemoteEntry,
  type GitPanelRepoEntry,
  type GitPanelRunOutput,
  type GitPanelStashEntry,
  type GitPanelWatchEventPayload,
} from "../../../services/tauri-api";
import { decideGitPanelRefreshTargets } from "../git-panel-watch-refresh";
import { formatRecentRelativeTime } from "../../shared/utils/relative-time";
import { useWorkspaceGitStatus } from "../composables/use-workspace-git-status";
import GitChangesGroup from "./GitChangesGroup.vue";
import CommitGraphLine from "./CommitGraphLine.vue";
import GitResizeHandle from "./GitResizeHandle.vue";
import GitSectionBar from "./GitSectionBar.vue";
import GitTree, { type GitTreeFlatRow, type GitTreeNode } from "./GitTree.vue";
import {
  computeCommitGraph,
  graphColor,
  laneLine,
  renderGraphRowSVG,
  type CommitGraphRef,
} from "../git-commit-graph";
import {
  buildBranchTree,
  sortBranchesForDisplay,
  type BranchTreeNode,
} from "../git-branch-order";

const props = withDefaults(defineProps<{
  workspacePath: string;
  markdownIsDark?: boolean;
  sessionKey?: string;
}>(), {
  markdownIsDark: false,
  sessionKey: "",
});

const emit = defineEmits<{
  (e: "openDiff", payload: { workspacePath: string; path: string; staged: boolean; hash?: string; untracked?: boolean }): void;
}>();

const { t } = useI18n();

const gitPanelTabs = computed(() => [
  { key: "commits", label: t("gitPanel.commitsTab"), icon: History },
  { key: "stashes", label: t("gitPanel.stashesTab"), icon: GitCommitHorizontal },
  { key: "branches", label: t("gitPanel.branchesTab"), icon: GitBranch },
]);

const activeGitTab = ref("commits");
const activeHistoryTab = computed(
  () => gitPanelTabs.value.find((tab) => tab.key === activeGitTab.value) ?? gitPanelTabs.value[0],
);
const busy = ref(false);

// ==================== 折叠状态 ====================
const changesCollapsed = ref(false);
const historyCollapsed = ref(false);
const changesRefreshing = ref(false);

// 更改列表展示模式：tree 树状分组 / list 平铺（VSCode 风格切换），默认平铺
const changesViewMode = ref<"tree" | "list">("list");

// 切换更改列表视图模式，并持久化到会话
function toggleChangesViewMode() {
  changesViewMode.value = changesViewMode.value === "tree" ? "list" : "tree";
  persistChangesViewMode();
}

// 仓库栏：折叠/展开 + 列表（懒加载，展开首次才扫描）— 上栏默认折叠
const repoCollapsed = ref(true);
const repos = ref<GitPanelRepoEntry[]>([]);
const reposLoading = ref(false);
const reposLoaded = ref(false);

// ==================== 分栏高度（分界线拖拽） ====================
const historyHeight = ref<number | null>(null);
const historySectionRef = ref<HTMLElement | null>(null);
let historyResizeStart = 0;
let historyContainerHeight = 0;

// 上栏最小高度：保留折叠条高度，拖拽时不允许覆盖
const CHANGES_MIN_HEIGHT = 40;
// 下栏最小高度：折叠条 + 底部 tab 栏
const HISTORY_MIN_HEIGHT = 96;

function onHistoryResizeStart() {
  historyResizeStart = historySectionRef.value?.offsetHeight ?? 300;
  historyContainerHeight = historySectionRef.value?.parentElement?.offsetHeight ?? 0;
}
function onHistoryResize(dy: number) {
  // 分界线向上拖（dy 为负）→ 下栏变大；向下拖 → 下栏变小
  // 上限：容器高度 - 上栏最小高度，保证上栏折叠条不被覆盖
  const maxHistory = Math.max(HISTORY_MIN_HEIGHT, historyContainerHeight - CHANGES_MIN_HEIGHT);
  historyHeight.value = Math.min(Math.max(HISTORY_MIN_HEIGHT, historyResizeStart - dy), maxHistory);
}
const toast = ref<{ kind: "success" | "error" | "info"; message: string } | null>(null);
let toastTimer: number | undefined;

// ==================== 探测状态 ====================
const gitAvailable = ref(false);
const detectChecked = ref(false);
const detectError = ref("");
// 仓库根与更改状态来自共享状态源（与右栏卡片墙的 Git 卡同一份）：面板负责选定仓库，其他消费方跟随
const {
  repoRoot,
  currentBranch,
  statusEntries,
  statusTruncated,
  stagedTotal,
  unstagedTotal,
  statusLoaded,
  statusError,
  setRepoRoot,
  loadStatus,
  onExternalChange,
  acquirePanel,
  releasePanel,
  rememberRepoRoot,
  readRememberedRepoRoot,
} = useWorkspaceGitStatus();

// 当前仓库名（仓库栏折叠条标题）：repoRoot 最后一段
const currentRepoName = computed(() => {
  if (!repoRoot.value) return t("gitPanel.repoBar");
  const segments = repoRoot.value.replace(/\\/g, "/").split("/").filter(Boolean);
  return segments[segments.length - 1] || repoRoot.value;
});

// ==================== 数据 ====================
const branches = ref<GitPanelBranchEntry[]>([]);
const remotes = ref<GitPanelRemoteEntry[]>([]);
const stashList = ref<GitPanelStashEntry[]>([]);
const logEntries = ref<GitPanelLogEntry[]>([]);
const logHasMore = ref(false);
const logLoadingMore = ref(false);
const logPageSize = 50;
const logSentinel = ref<HTMLElement | null>(null);
let logObserver: IntersectionObserver | undefined;

// 分支/存储渲染分页（git 命令不支持分页，前端分批渲染，滚动到底自动加载更多）
const branchPageSize = 50;
const branchVisibleCount = ref(50);
const stashVisibleCount = ref(50);
const branchesSentinel = ref<HTMLElement | null>(null);
const stashSentinel = ref<HTMLElement | null>(null);
let branchesObserver: IntersectionObserver | undefined;
let stashObserver: IntersectionObserver | undefined;
const commitFilesMap = ref<Record<string, GitPanelCommitFileEntry[]>>({});
const commitFilesLoading = ref<Record<string, boolean>>({});
const stashFilesMap = ref<Record<string, GitPanelCommitFileEntry[]>>({});
const stashFilesLoading = ref<Record<string, boolean>>({});
const branchPickerOpen = ref(false);
const branchPickerLoading = ref(false);
// 相对时间基准：低频刷新，让「多久之前」跟随当前时刻（提交时间是历史值，60 秒粒度足够）
const relativeNowTick = ref(Date.now());
let relativeNowTimer = 0;

// ==================== 提交区 ====================
const commitMessage = ref("");
const stashMessage = ref("");
const newBranchName = ref("");
const selectedBranch = ref("");

// 滚动容器
const changesScroller = ref<HTMLElement | null>(null);
const branchesScroller = ref<HTMLElement | null>(null);
const historyScroller = ref<HTMLElement | null>(null);
const stashScroller = ref<HTMLElement | null>(null);

// ==================== 派生状态 ====================
const stagedEntries = computed(() => {
  return statusEntries.value.filter((entry) => {
    const staged = entry.stagedStatus.trim();
    const unstaged = entry.unstagedStatus.trim();
    // ?? 未跟踪属于未暂存；已暂存的是 X 列非空且非 ?
    return staged !== "" && staged !== "?" && !(unstaged === "?" && staged === "?");
  });
});

const unstagedEntries = computed(() => {
  return statusEntries.value.filter((entry) => {
    const staged = entry.stagedStatus.trim();
    const unstaged = entry.unstagedStatus.trim();
    if (staged === "?" && unstaged === "?") return true; // 未跟踪
    return unstaged !== "" && !(staged !== "" && staged !== "?" && unstaged === "");
  });
});

const totalChanges = computed(() => statusEntries.value.length);

const localBranches = computed(() => branches.value.filter((branch) => !branch.isRemote));
const remoteBranches = computed(() => branches.value.filter((branch) => branch.isRemote));
// 渲染分页：分支/存储统一拍平为行序列，滚动到底自动增加可见数
type BranchRow =
  | { kind: "header"; key: string; text: string }
  | { kind: "folder"; key: string; name: string; path: string }
  | { kind: "branch"; key: string; branch: GitPanelBranchEntry; label: string }
  | { kind: "remote-branch"; key: string; branch: GitPanelBranchEntry; label: string }
  | { kind: "remote"; key: string; remote: GitPanelRemoteEntry };

// 排序：当前分支置顶 → 末次提交时间倒序 → 名称升序；分组：排序后按 / 折叠（见 git-branch-order.ts）
const sortedLocalBranches = computed(() => sortBranchesForDisplay(localBranches.value));
const sortedRemoteBranches = computed(() => sortBranchesForDisplay(remoteBranches.value));
// 分页按「分支」切片，再建树——否则一棵文件夹会被切片边界切掉一半
const pagedLocalBranches = computed(() => sortedLocalBranches.value.slice(0, branchVisibleCount.value));
const pagedRemoteBranches = computed(() => sortedRemoteBranches.value.slice(0, branchVisibleCount.value));

/** 纯函数树节点 → GitTree 节点；scope 决定本地/远程行类型与 key 命名空间 */
function toBranchGitTreeNodes(
  nodes: BranchTreeNode<GitPanelBranchEntry>[],
  scope: "local" | "remote",
): GitTreeNode<BranchRow>[] {
  return nodes.map((node) => {
    if (node.kind === "branch") {
      const key = `${scope}:${node.branch.name}`;
      return {
        key,
        data:
          scope === "local"
            ? { kind: "branch", key, branch: node.branch, label: node.label }
            : { kind: "remote-branch", key, branch: node.branch, label: node.label },
        // 折进文件夹后只显示末段，悬停给完整分支名
        title: node.branch.name,
        // 当前分支行高亮（数据声明的行级样式）
        rowClass:
          scope === "local" && node.branch.isCurrent ? "bg-primary/10 text-primary" : undefined,
      };
    }
    const key = `folder:${scope}:${node.path}`;
    return {
      key,
      data: { kind: "folder", key, name: node.name, path: node.path },
      children: toBranchGitTreeNodes(node.children, scope),
    };
  });
}

const localBranchTreeNodes = computed(() =>
  toBranchGitTreeNodes(buildBranchTree(pagedLocalBranches.value), "local"),
);
const remoteBranchTreeNodes = computed(() =>
  toBranchGitTreeNodes(buildBranchTree(pagedRemoteBranches.value), "remote"),
);

/** 分支 tab 树：分组头为树根（本地分支/远程分支/远程），分支树与远程 URL 为子节点 */
const branchTreeNodes = computed<GitTreeNode<BranchRow>[]>(() => {
  const roots: GitTreeNode<BranchRow>[] = [];
  if (sortedLocalBranches.value.length > 0) {
    roots.push({
      key: "header:local",
      data: { kind: "header", key: "header:local", text: t("gitPanel.localBranches") },
      children: localBranchTreeNodes.value,
    });
  }
  if (sortedRemoteBranches.value.length > 0) {
    roots.push({
      key: "header:remote",
      data: { kind: "header", key: "header:remote", text: t("gitPanel.remoteBranches") },
      children: remoteBranchTreeNodes.value,
    });
  }
  if (remotes.value.length > 0) {
    roots.push({
      key: "header:remotes",
      data: { kind: "header", key: "header:remotes", text: t("gitPanel.remotes") },
      children: remotes.value.map((remote) => ({
        key: `remotes:${remote.name}`,
        data: { kind: "remote" as const, key: `remotes:${remote.name}`, remote },
        // 远程 URL 展示行不可交互（无 hover/点击）
        interactive: false,
      })),
    });
  }
  return roots;
});

/** 远程整体默认折叠（本地一眼看全），展开状态交由用户操作 */
const branchCollapsedKeys = ["header:remote", "header:remotes", "picker:remote", "picker:remotes"];

/** 分支行点击：选中查看（不切换分支） */
function onBranchRowClick(row: GitTreeFlatRow<BranchRow>) {
  const data = row.node.data;
  if (data.kind === "branch" || data.kind === "remote-branch") {
    selectBranch(data.branch.name);
  }
}

/** 分支切换下拉树：与分支 tab 同一套排序与分组，点击分支直接切换 */
const branchPickerTreeNodes = computed<GitTreeNode<BranchRow>[]>(() => {
  const roots: GitTreeNode<BranchRow>[] = [];
  if (sortedLocalBranches.value.length > 0) {
    roots.push({
      key: "picker:local",
      data: { kind: "header", key: "picker:local", text: t("gitPanel.localBranches") },
      children: toBranchGitTreeNodes(buildBranchTree(sortedLocalBranches.value), "local"),
    });
  }
  if (sortedRemoteBranches.value.length > 0) {
    roots.push({
      key: "picker:remote",
      data: { kind: "header", key: "picker:remote", text: t("gitPanel.remoteBranches") },
      children: toBranchGitTreeNodes(buildBranchTree(sortedRemoteBranches.value), "remote"),
    });
  }
  return roots;
});

/** 分支切换下拉行点击：切换分支（当前分支忽略） */
function onBranchPickerRowClick(row: GitTreeFlatRow<BranchRow>) {
  const data = row.node.data;
  if (data.kind === "branch" && data.branch.isCurrent) return;
  if (data.kind === "branch" || data.kind === "remote-branch") {
    void runCheckoutBranch(data.branch.name);
  }
}
const branchHasMore = computed(
  () =>
    sortedLocalBranches.value.length > branchVisibleCount.value ||
    sortedRemoteBranches.value.length > branchVisibleCount.value,
);
const visibleStashList = computed(() => stashList.value.slice(0, stashVisibleCount.value));
const stashHasMore = computed(() => stashList.value.length > stashVisibleCount.value);

// ==================== 操作提示 ====================
function showToast(kind: "success" | "error" | "info", message: string) {
  toast.value = { kind, message };
  if (toastTimer) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = null;
  }, 5000);
}

/** 成功提示：中文动作描述，不展示 git 原始输出 */
function showSuccessToast(message: string) {
  showToast("success", message);
}

/** 失败提示：git stderr 或可读错误信息，保留排障细节 */
function showErrorToast(message: string) {
  showToast("error", message);
}

// ==================== 输出记录 ====================
function appendOutput(command: string, result: GitPanelRunOutput | null, error: unknown = null) {
  // 失败：展示 stderr / 退出码 / 异常信息；成功时不弹 git 原文（由调用方按动作弹中文成功提示）
  const body: string[] = [];
  if (result?.stderr?.trim()) body.push(result.stderr.trim());
  if (error) body.push(error instanceof Error ? error.message : String(error));
  const message = body.join("\n").trim();
  if (message) {
    showErrorToast(message.split("\n").slice(0, 3).join("\n"));
  } else if (result && result.exitCode !== 0) {
    showErrorToast(`git ${command} 失败（退出码 ${result.exitCode}）`);
  }
}

// 共享状态源的取数失败，沿用面板原有的报错出口
watch(statusError, (message) => {
  if (message) appendOutput("status", null, new Error(message));
});

// ==================== 数据加载 ====================
// 按需懒加载：上栏展开才拉更改/暂存，下栏切到对应 tab 才拉历史/存储/分支；
// 各数据首次加载成功置标记，折叠/切走不重复拉取（更改/暂存的标记在共享状态源里）
const historyLoaded = ref(false);
const stashesLoaded = ref(false);
const branchesLoaded = ref(false);

// 按当前可见状态加载缺失数据：上栏展开 → 更改/暂存；下栏展开 → 当前 tab 对应数据
function ensureVisibleData() {
  if (!statusLoaded.value && !changesCollapsed.value) {
    void loadStatus();
  }
  if (historyCollapsed.value) return;
  if (activeGitTab.value === "commits" && !historyLoaded.value) {
    void loadHistory();
  } else if (activeGitTab.value === "stashes" && !stashesLoaded.value) {
    void loadStashes();
  } else if (activeGitTab.value === "branches" && !branchesLoaded.value) {
    void Promise.all([loadBranches(), loadRemotes()]);
  }
}

// 折叠/切 tab 变化时重新评估可见数据；immediate 保证初始即展开时也加载
watch([changesCollapsed, activeGitTab, historyCollapsed], () => ensureVisibleData(), {
  immediate: true,
});

// 收起下栏时提交视图卸载，关闭预览卡
watch(historyCollapsed, (collapsed) => {
  if (collapsed) {
    closeCommitCard();
  }
});

// 仓库列表：单次探查（向上探测 + 向下扫描 + 默认仓库推荐），后端一次返回；
// force=true 强制重扫（绕过缓存）
async function loadDiscover(force = false) {
  if (reposLoading.value) return;
  reposLoading.value = true;
  try {
    const result = await gitPanelDiscover(props.workspacePath, force);
    gitAvailable.value = !!result.gitAvailable;
    detectChecked.value = !!result.checked;
    repos.value = result.repos || [];
    reposLoaded.value = true;
    // 仓库根交给共享状态源，面板自身跟随：优先恢复这个会话上次选中的仓库
    // （探测出的默认仓库是当前工作区自身所在的仓库，直接采用会把用户的选择顶掉）
    const rememberedRoot = readRememberedRepoRoot(
      String(props.sessionKey || "").trim(),
      (result.repos || []).map((repo) => repo.path),
    );
    setRepoRoot(rememberedRoot || String(result.defaultRepoRoot || ""));
    detectError.value =
      result.error ||
      (!result.gitAvailable
        ? t("gitPanel.gitNotInstalled")
        : !result.currentRepoRoot && repos.value.length === 0
          ? t("gitPanel.notRepository")
          : "");
    // 探查完成后统一触发可见数据加载：无论本次探查由 watch immediate、
    // onMounted 还是手动刷新发起，都能补上首次数据，避免 onMounted 被
    // reposLoading 防重入挡住时触发链断裂导致面板全空。
    ensureVisibleData();
  } catch (error) {
    gitAvailable.value = false;
    detectChecked.value = true;
    detectError.value = error instanceof Error ? error.message : String(error);
  } finally {
    reposLoading.value = false;
  }
}

function refreshRepos() {
  void loadDiscover(true);
}

function isCurrentRepo(path: string): boolean {
  if (!repoRoot.value || !path) return false;
  const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase();
  return norm(path) === norm(repoRoot.value);
}

// 切换仓库：把共享状态源切到新仓库，重置各数据加载标记后按当前可见区域重载
function switchRepo(path: string) {
  if (!path || isCurrentRepo(path) || busy.value) return;
  branchPickerOpen.value = false;
  commitCard.value = { entry: null, x: 0, y: 0 };
  lastClickedDiffPath.value = "";
  commitFilesMap.value = {};
  historyLoaded.value = false;
  stashesLoaded.value = false;
  branchesLoaded.value = false;
  // 重置加载冷却时间戳：否则新仓库的首次加载会被 1 秒冷却拦截，面板下方无数据
  lastHistoryLoad.value = 0;
  lastStashesLoad.value = 0;
  lastBranchesLoad.value = 0;
  // 仓库根切换、更改数据清空与 status 冷却重置都在共享状态源里统一处理
  setRepoRoot(path);
  // 记住这次选择：面板重挂、手动刷新仓库栏、切会话回来都按它恢复
  rememberRepoRoot(String(props.sessionKey || "").trim(), path);
  ensureVisibleData();
}

// 展开仓库栏才首次探查（懒加载）；之后只读后端缓存。
// immediate：初始即展开时也要触发加载，否则列表一直空到手动刷新。
watch(
  repoCollapsed,
  (collapsed) => {
    if (!collapsed && !reposLoaded.value) {
      void loadDiscover(false);
    }
  },
  { immediate: true },
);

/** 其余数据的加载冷却：自动触发 1 秒内不重复请求；写操作后的刷新与用户主动刷新可穿透 */
const REFRESH_CD_MS = 1000;
const lastBranchesLoad = ref(0);
const lastStashesLoad = ref(0);
const lastHistoryLoad = ref(0);

async function refreshChanges() {
  changesRefreshing.value = true;
  try {
    await loadStatus(true);
  } finally {
    changesRefreshing.value = false;
  }
}

async function loadBranches(force = false) {
  if (!repoRoot.value) return;
  const now = Date.now();
  if (!force && now - lastBranchesLoad.value < REFRESH_CD_MS) return;
  lastBranchesLoad.value = now;
  try {
    branches.value = await gitPanelBranchList(repoRoot.value);
    branchesLoaded.value = true;
  } catch (error) {
    appendOutput("for-each-ref", null, error);
  }
}

async function loadRemotes() {
  if (!repoRoot.value) return;
  try {
    remotes.value = await gitPanelRemoteList(repoRoot.value);
  } catch (error) {
    appendOutput("remote -v", null, error);
  }
}

async function loadStashes(force = false) {
  if (!repoRoot.value) return;
  const now = Date.now();
  if (!force && now - lastStashesLoad.value < REFRESH_CD_MS) return;
  lastStashesLoad.value = now;
  try {
    stashList.value = await gitPanelStashList(repoRoot.value);
    stashesLoaded.value = true;
  } catch (error) {
    appendOutput("stash list", null, error);
  }
}

async function loadHistory(force = false) {
  if (!repoRoot.value) return;
  const now = Date.now();
  if (!force && now - lastHistoryLoad.value < REFRESH_CD_MS) return;
  lastHistoryLoad.value = now;
  try {
    const result = await gitPanelLog(repoRoot.value, logPageSize, 0);
    logEntries.value = result.entries || [];
    logHasMore.value = (result.entries || []).length >= logPageSize;
    historyLoaded.value = true;
  } catch (error) {
    appendOutput("log", null, error);
  }
}

async function loadMoreHistory() {
  if (!repoRoot.value || logLoadingMore.value || busy.value || !logHasMore.value) return;
  logLoadingMore.value = true;
  try {
    const result = await gitPanelLog(repoRoot.value, logPageSize, logEntries.value.length);
    const next = result.entries || [];
    logEntries.value = logEntries.value.concat(next);
    logHasMore.value = next.length >= logPageSize;
  } catch (error) {
    appendOutput("log", null, error);
  } finally {
    logLoadingMore.value = false;
  }
}

function observeLogSentinel() {
  logObserver?.disconnect();
  if (!logSentinel.value) return;
  logObserver = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) void loadMoreHistory();
    },
    { root: historyScroller.value, rootMargin: "120px" },
  );
  logObserver.observe(logSentinel.value);
}

watch(logHasMore, async (hasMore) => {
  if (hasMore) {
    await nextTick();
    observeLogSentinel();
  } else {
    logObserver?.disconnect();
    logObserver = undefined;
  }
});

function observeBranchesSentinel() {
  branchesObserver?.disconnect();
  if (!branchesSentinel.value) return;
  branchesObserver = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) {
        branchVisibleCount.value += branchPageSize;
      }
    },
    { root: branchesScroller.value, rootMargin: "120px" },
  );
  branchesObserver.observe(branchesSentinel.value);
}

function observeStashSentinel() {
  stashObserver?.disconnect();
  if (!stashSentinel.value) return;
  stashObserver = new IntersectionObserver(
    (entries) => {
      if (entries[0]?.isIntersecting) {
        stashVisibleCount.value += 50;
      }
    },
    { root: stashScroller.value, rootMargin: "120px" },
  );
  stashObserver.observe(stashSentinel.value);
}

watch([branchHasMore, activeGitTab], async ([hasMore, tab]) => {
  if (hasMore && tab === "branches") {
    await nextTick();
    observeBranchesSentinel();
  } else {
    branchesObserver?.disconnect();
    branchesObserver = undefined;
  }
});

watch([stashHasMore, activeGitTab], async ([hasMore, tab]) => {
  if (hasMore && tab === "stashes") {
    await nextTick();
    observeStashSentinel();
  } else {
    stashObserver?.disconnect();
    stashObserver = undefined;
  }
});

async function refreshHistory() {
  if (busy.value) return;
  busy.value = true;
  try {
    await loadHistory(true);
  } finally {
    busy.value = false;
  }
}

function selectGitTab(key: string) {
  activeGitTab.value = key;
  persistGitTab();
  branchPickerOpen.value = false;
  closeCommitCard();
}

// 分支下拉展开时才加载分支/远程数据（提交页底部的分支按钮也能用）
function toggleBranchPicker() {
  branchPickerOpen.value = !branchPickerOpen.value;
  if (branchPickerOpen.value && !branchesLoaded.value) {
    void Promise.all([loadBranches(), loadRemotes()]);
  }
}

// ==================== git 标签页持久化 ====================
// 与文件树目录展开状态共用 sessionKey，记住上次打开的 git 标签
function gitTabStorageKey() {
  const key = String(props.sessionKey || "").trim();
  return key ? `${key}:git-panel-tab` : "";
}

function restoreGitTab() {
  const storageKey = gitTabStorageKey();
  if (!storageKey || typeof window === "undefined") return;
  try {
    const saved = window.localStorage.getItem(storageKey);
    if (saved && gitPanelTabs.value.some((tab) => tab.key === saved)) {
      activeGitTab.value = saved;
    }
  } catch {
    // 读取失败忽略，保持默认
  }
}

function persistGitTab() {
  const storageKey = gitTabStorageKey();
  if (!storageKey || typeof window === "undefined") return;
  try {
    window.localStorage.setItem(storageKey, activeGitTab.value);
  } catch {
    // 写入失败忽略
  }
}

// ==================== 更改列表视图模式持久化 ====================
// 视图模式是全局偏好，不随会话变化，所有会话共享同一设置
function changesViewModeStorageKey() {
  return "git-panel-view-mode";
}

function restoreChangesViewMode() {
  const storageKey = changesViewModeStorageKey();
  if (!storageKey || typeof window === "undefined") return;
  try {
    const saved = window.localStorage.getItem(storageKey);
    if (saved === "tree" || saved === "list") {
      changesViewMode.value = saved;
    }
  } catch {
    // 读取失败忽略，保持默认
  }
}

function persistChangesViewMode() {
  const storageKey = changesViewModeStorageKey();
  if (!storageKey || typeof window === "undefined") return;
  try {
    window.localStorage.setItem(storageKey, changesViewMode.value);
  } catch {
    // 写入失败忽略
  }
}

// ==================== 更改操作 ====================
async function runGitAction(
  command: string,
  action: () => Promise<GitPanelRunOutput>,
  successText?: string,
): Promise<boolean> {
  if (busy.value) return false;
  busy.value = true;
  // 操作超过 300ms 未完成时显示进行中提示，避免快速操作闪烁
  let processingTimer: number | undefined;
  if (successText) {
    processingTimer = window.setTimeout(() => showToast("info", "正在执行 git 操作…"), 300);
  }
  try {
    const result = await action();
    if (processingTimer) window.clearTimeout(processingTimer);
    const succeeded = result.exitCode === 0;
    if (!succeeded) {
      appendOutput(command, result);
    }
    // 无论成败都刷新状态：stash apply/pop 冲突时退出码非零，
    // 但工作树已写入冲突标记，必须让用户能在面板中看到冲突状态
    if (succeeded && successText) showSuccessToast(successText);
    await loadStatus(true);
    await loadBranches(true);
    await loadStashes(true);
    return succeeded;
  } catch (error) {
    if (processingTimer) window.clearTimeout(processingTimer);
    appendOutput(command, null, error);
    return false;
  } finally {
    busy.value = false;
  }
}

function stagePaths(paths: string[]) {
  if (paths.length === 0) return;
  void runGitAction(`add ${paths.join(" ")}`, () => gitPanelStage(repoRoot.value, paths), `已暂存 ${paths.length} 个文件`);
}

function unstagePaths(paths: string[]) {
  if (paths.length === 0) return;
  void runGitAction(`restore --staged ${paths.join(" ")}`, () => gitPanelUnstage(repoRoot.value, paths), `已取消暂存 ${paths.length} 个文件`);
}

function discardPaths(paths: string[]) {
  if (paths.length === 0) return;
  if (!window.confirm(t("gitPanel.discardConfirm", { paths: paths.join(", ") }))) return;
  void runGitAction(`restore --staged --worktree ${paths.join(" ")}`, () => gitPanelDiscard(repoRoot.value, paths), `已撤回 ${paths.length} 个文件`);
}

async function runCommit(amend = false) {
  const message = commitMessage.value.trim();
  if (!message || stagedEntries.value.length === 0 || busy.value) return;
  busy.value = true;
  try {
    const result = await gitPanelCommit(repoRoot.value, message, amend);
    appendOutput(`commit${amend ? " --amend" : ""}`, result);
    if (result.exitCode === 0) showSuccessToast(amend ? "已修改提交" : "已提交");
    commitMessage.value = "";
    resetCommitInputHeight();
    await loadStatus(true);
    await loadHistory(true);
  } catch (error) {
    appendOutput("commit", null, error);
  } finally {
    busy.value = false;
  }
}

// ==================== 储藏操作 ====================
async function runStashCreate(staged = false) {
  const message = stashMessage.value.trim();
  if (!message || busy.value) return;
  const ok = await runGitAction(
    staged ? "stash push --staged" : "stash push",
    () => gitPanelStashCreate(repoRoot.value, message, staged),
    "已创建储藏",
  );
  if (ok) stashMessage.value = "";
}

async function runStashPop(stashRef: string) {
  void runGitAction(`stash pop ${stashRef}`, () => gitPanelStashPop(repoRoot.value, stashRef), "已恢复储藏");
}

async function runStashApply(stashRef: string) {
  void runGitAction(`stash apply ${stashRef}`, () => gitPanelStashApply(repoRoot.value, stashRef), "已应用储藏");
}

async function runStashDrop(stashRef: string) {
  if (!window.confirm(t("gitPanel.stashDropConfirm", { reference: stashRef }))) return;
  void runGitAction(`stash drop ${stashRef}`, () => gitPanelStashDrop(repoRoot.value, stashRef), "已删除储藏");
}

// ==================== 同步操作 ====================
function runSync() {
  void runGitAction("sync (fetch + pull)", () => gitPanelSync(repoRoot.value), "已同步");
}

function runPush() {
  void runGitAction("push", () => gitPanelPush(repoRoot.value), "已推送");
}

function runPull() {
  void runGitAction("pull", () => gitPanelPull(repoRoot.value), "已拉取");
}

// ==================== 分支操作 ====================
async function runBranchCreate() {
  const name = newBranchName.value.trim();
  if (!name || busy.value) return;
  busy.value = true;
  try {
    const result = await gitPanelBranchCreate(repoRoot.value, name);
    appendOutput(`branch ${name}`, result);
    if (result.exitCode === 0) showSuccessToast("已创建分支");
    newBranchName.value = "";
    await loadBranches(true);
  } catch (error) {
    appendOutput(`branch ${name}`, null, error);
  } finally {
    busy.value = false;
  }
}

async function selectBranch(name: string) {
  // 仅选中查看，不切换分支；切换走显式按钮 + 确认
  selectedBranch.value = name;
}

async function runCheckoutBranch(name: string) {
  if (busy.value) return;
  // 预检：工作区未提交文件与目标分支改动有交集则禁止切换
  try {
    const check = await gitPanelCheckoutCheck(repoRoot.value, name);
    if (check.conflictingPaths.length > 0) {
      window.alert(
        t("gitPanel.checkoutBlocked", {
          name,
          paths: check.conflictingPaths.join("\n"),
        }),
      );
      return;
    }
  } catch {
    // 预检失败不阻塞，走默认确认文案
  }
  if (!window.confirm(t("gitPanel.checkoutConfirm", { name }))) return;
  branchPickerOpen.value = false;
  const ok = await runGitAction(`checkout ${name}`, () => gitPanelCheckout(repoRoot.value, name), `已切换分支 ${name}`);
  if (ok) {
    await loadHistory(true);
  }
}

// ==================== commit 右键预览卡 ====================
const commitCard = ref<{ entry: GitPanelLogEntry | null; x: number; y: number }>({ entry: null, x: 0, y: 0 });
const commitCardRef = ref<HTMLElement | null>(null);

function openCommitMenu(entry: GitPanelLogEntry, event: MouseEvent) {
  const anchor = (event.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  const cardWidth = 288; // w-72
  const gap = 4;
  // 默认锚定按钮下方；拿不到锚点时回退到鼠标位置
  let x = anchor ? anchor.left : event.clientX;
  let y = anchor ? anchor.bottom + gap : event.clientY;
  // 卡片右侧溢出视口时左移
  x = Math.min(x, window.innerWidth - cardWidth - 8);
  commitCard.value = { entry, x: Math.max(8, x), y: Math.max(8, y) };
  window.addEventListener("pointerdown", handleGlobalPointerDownForCommitCard, true);
  window.addEventListener("keydown", handleCommitCardKeydown);
  // 卡片渲染后按实际高度校正：底部超出视口则上移，避免溢出屏幕
  void nextTick(() => {
    const el = commitCardRef.value;
    if (!el) return;
    const maxY = window.innerHeight - el.offsetHeight - 8;
    if (commitCard.value.y > maxY) {
      commitCard.value = { ...commitCard.value, y: Math.max(8, maxY) };
    }
  });
}

function closeCommitCard() {
  commitCard.value = { entry: null, x: 0, y: 0 };
  window.removeEventListener("pointerdown", handleGlobalPointerDownForCommitCard, true);
  window.removeEventListener("keydown", handleCommitCardKeydown);
}

/** 点击卡片外部任意位置关闭 */
function handleGlobalPointerDownForCommitCard(event: PointerEvent) {
  if (!commitCard.value.entry) return;
  const target = event.target as Node;
  if (commitCardRef.value?.contains(target)) return;
  closeCommitCard();
}

/** Escape 关闭 */
function handleCommitCardKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  closeCommitCard();
}

// ==================== stash 操作菜单 ====================
const stashMenu = ref<{ entry: GitPanelStashEntry | null; x: number; y: number }>({ entry: null, x: 0, y: 0 });
const stashMenuRef = ref<HTMLElement | null>(null);

function openStashMenu(stash: GitPanelStashEntry, event: MouseEvent) {
  const anchor = (event.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  const cardWidth = 224; // w-56
  const gap = 4;
  // 默认锚定按钮下方；拿不到锚点时回退到鼠标位置
  let x = anchor ? anchor.left : event.clientX;
  let y = anchor ? anchor.bottom + gap : event.clientY;
  // 卡片右侧溢出视口时左移
  x = Math.min(x, window.innerWidth - cardWidth - 8);
  stashMenu.value = { entry: stash, x: Math.max(8, x), y: Math.max(8, y) };
  window.addEventListener("pointerdown", handleGlobalPointerDownForStashMenu, true);
  window.addEventListener("keydown", handleStashMenuKeydown);
  // 卡片渲染后按实际高度校正：底部超出视口则上移，避免溢出屏幕
  void nextTick(() => {
    const el = stashMenuRef.value;
    if (!el) return;
    const maxY = window.innerHeight - el.offsetHeight - 8;
    if (stashMenu.value.y > maxY) {
      stashMenu.value = { ...stashMenu.value, y: Math.max(8, maxY) };
    }
  });
}

function closeStashMenu() {
  stashMenu.value = { entry: null, x: 0, y: 0 };
  window.removeEventListener("pointerdown", handleGlobalPointerDownForStashMenu, true);
  window.removeEventListener("keydown", handleStashMenuKeydown);
}

/** 点击菜单外部任意位置关闭 */
function handleGlobalPointerDownForStashMenu(event: PointerEvent) {
  if (!stashMenu.value.entry) return;
  const target = event.target as Node;
  if (stashMenuRef.value?.contains(target)) return;
  closeStashMenu();
}

/** Escape 关闭 */
function handleStashMenuKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  closeStashMenu();
}

function stashApplyFromMenu() {
  const stash = stashMenu.value.entry;
  closeStashMenu();
  if (!stash || busy.value) return;
  void runStashApply(stash.reference);
}

function stashPopFromMenu() {
  const stash = stashMenu.value.entry;
  closeStashMenu();
  if (!stash || busy.value) return;
  void runStashPop(stash.reference);
}

function stashDropFromMenu() {
  const stash = stashMenu.value.entry;
  closeStashMenu();
  if (!stash || busy.value) return;
  void runStashDrop(stash.reference);
}

/** 当前预览卡是否是最新提交（soft 撤销仅对 HEAD 生效） */
const isLatestCommit = computed(() => {
  const entry = commitCard.value.entry;
  return !!entry && logEntries.value[0]?.hash === entry.hash;
});

async function copyCommitHash() {
  const hash = commitCard.value.entry?.hash || "";
  if (!hash) return;
  try {
    await navigator.clipboard.writeText(hash);
    closeCommitCard();
  } catch {
    appendOutput("copy hash", null, new Error("复制哈希失败"));
  }
}

async function copyCommitMessage() {
  const message = commitCard.value.entry?.message || "";
  if (!message) return;
  try {
    await navigator.clipboard.writeText(message);
    closeCommitCard();
  } catch {
    appendOutput("copy message", null, new Error("复制提交消息失败"));
  }
}

/** 基于该提交新建分支：弹窗输入分支名后创建 */
async function createBranchFromCommit() {
  const entry = commitCard.value.entry;
  closeCommitCard();
  if (!entry || busy.value) return;
  const name = window.prompt(t("gitPanel.createBranchPrompt"), "");
  if (!name?.trim()) return;
  busy.value = true;
  try {
    const result = await gitPanelBranchCreate(repoRoot.value, name.trim(), entry.hash);
    appendOutput(`branch ${name} ${entry.shortHash}`, result);
    if (result.exitCode === 0) showSuccessToast(t("gitPanel.branchCreated"));
    await loadBranches(true);
  } catch (error) {
    appendOutput(`branch ${name}`, null, error);
  } finally {
    busy.value = false;
  }
}

/** 撤销最近一次提交（soft，保留改动）：已推送时拒绝并提示 */
async function resetSoftCommit() {
  closeCommitCard();
  if (busy.value) return;
  busy.value = true;
  try {
    const result = await gitPanelResetSoft(repoRoot.value);
    appendOutput("reset --soft HEAD~1", result);
    if (result.exitCode === 0) {
      showSuccessToast(t("gitPanel.resetSoftSuccess"));
      await loadHistory(true);
      await loadStatus(true);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const pushed = message.includes("已经在远端");
    if (pushed) showErrorToast(t("gitPanel.resetSoftPushed"));
    else appendOutput("reset --soft HEAD~1", null, error);
  } finally {
    busy.value = false;
  }
}

// ==================== commit 展开（GitTree 懒加载） ====================
type CommitNode =
  | { kind: "commit"; entry: GitPanelLogEntry; graphSvg: string; refs: CommitGraphRef[] }
  | { kind: "all"; hash: string; lineX: number; lineColor: string; graphWidth: number }
  | { kind: "loading"; hash: string; lineX: number; lineColor: string; graphWidth: number }
  | { kind: "empty"; hash: string; lineX: number; lineColor: string; graphWidth: number }
  | { kind: "file"; hash: string; file: GitPanelCommitFileEntry; lineX: number; lineColor: string; graphWidth: number };

/** 提交 tab 树：父节点=提交（懒加载 diff 文件子节点） */
const commitTreeNodes = computed<GitTreeNode<CommitNode>[]>(() => {
  const graph = computeCommitGraph(logEntries.value);
  return logEntries.value.map((entry, i) => {
    const row = graph.rows[i];
    const graphWidth = graph.widthByRow[i];
    // 子节点延续竖线：位置与颜色取自父提交行的节点泳道
    const line = laneLine(row.circleIndex, row.circleColorIndex);
    const node: GitTreeNode<CommitNode> = {
      key: entry.hash,
      data: {
        kind: "commit",
        entry,
        graphSvg: renderGraphRowSVG(row),
        refs: row.refs,
      },
      expandable: true,
      title: `${entry.hash}\n${entry.author} ${entry.date}`,
    };
    const children = commitChildren(entry.hash, line.x, line.color, graphWidth);
    if (children) node.children = children;
    return node;
  });
});

function commitChildren(hash: string, lineX: number, lineColor: string, graphWidth: number): GitTreeNode<CommitNode>[] | undefined {
  if (commitFilesLoading.value[hash]) {
    return [
      { key: `${hash}:all`, data: { kind: "all", hash, lineX, lineColor, graphWidth } },
      { key: `${hash}:loading`, data: { kind: "loading", hash, lineX, lineColor, graphWidth }, interactive: false },
    ];
  }
  const files = commitFilesMap.value[hash];
  if (!files) return undefined;
  const children: GitTreeNode<CommitNode>[] = [
    { key: `${hash}:all`, data: { kind: "all", hash, lineX, lineColor, graphWidth } },
  ];
  if (files.length === 0) {
    children.push({ key: `${hash}:empty`, data: { kind: "empty", hash, lineX, lineColor, graphWidth }, interactive: false });
  } else {
    for (const file of files) {
      children.push({ key: `${hash}:${file.path}`, data: { kind: "file", hash, file, lineX, lineColor, graphWidth } });
    }
  }
  return children;
}

/** 提交子节点点击：diff 文件行打开文件 diff */
function onCommitRowClick(row: GitTreeFlatRow<CommitNode>) {
  if (row.node.data.kind === "file") {
    openCommitFileDiff(row.node.data.hash, row.node.data.file);
  }
}

/** 提交展开懒加载：拉取该提交的 diff 文件列表 */
async function onCommitExpand(hash: string) {
  if (commitFilesMap.value[hash] || commitFilesLoading.value[hash]) return;
  branchPickerOpen.value = false;
  commitFilesLoading.value = { ...commitFilesLoading.value, [hash]: true };
  try {
    const result = await gitPanelCommitFiles(repoRoot.value, hash);
    commitFilesMap.value = { ...commitFilesMap.value, [hash]: result.entries || [] };
  } catch (error) {
    appendOutput(`show --name-status ${hash}`, null, error);
    commitFilesMap.value = { ...commitFilesMap.value, [hash]: [] };
  } finally {
    commitFilesLoading.value = { ...commitFilesLoading.value, [hash]: false };
  }
}

function openCommitFileDiff(hash: string, file: GitPanelCommitFileEntry) {
  emit("openDiff", {
    workspacePath: repoRoot.value,
    path: file.path,
    staged: false,
    hash,
  });
}

// 一次性聚合查看整个提交的全部文件更改（类似 VS Code 悬停提交时的"查看提交更改"）
function openCommitAllDiff(hash: string) {
  emit("openDiff", {
    workspacePath: repoRoot.value,
    path: hash,
    staged: false,
    hash,
  });
}

// ==================== stash 展开（GitTree 懒加载） ====================
type StashNode =
  | { kind: "stash"; stash: GitPanelStashEntry; index: number }
  | { kind: "loading"; reference: string }
  | { kind: "empty"; reference: string }
  | { kind: "file"; reference: string; file: GitPanelCommitFileEntry };

/** 储存 tab 树：父节点=储藏（懒加载 diff 文件子节点） */
const stashTreeNodes = computed<GitTreeNode<StashNode>[]>(() =>
  visibleStashList.value.map((stash, index) => {
    const node: GitTreeNode<StashNode> = {
      key: stash.reference,
      data: { kind: "stash", stash, index },
      expandable: true,
      title: stash.message,
    };
    const children = stashChildren(stash.reference);
    if (children) node.children = children;
    return node;
  }),
);

function stashChildren(reference: string): GitTreeNode<StashNode>[] | undefined {
  if (stashFilesLoading.value[reference]) {
    return [{ key: `${reference}:loading`, data: { kind: "loading", reference }, interactive: false }];
  }
  const files = stashFilesMap.value[reference];
  if (!files) return undefined;
  if (files.length === 0) {
    return [{ key: `${reference}:empty`, data: { kind: "empty", reference }, interactive: false }];
  }
  return files.map((file) => ({
    key: `${reference}:${file.path}`,
    data: { kind: "file", reference, file },
  }));
}

/** 储藏子节点点击：diff 文件行打开文件 diff */
function onStashRowClick(row: GitTreeFlatRow<StashNode>) {
  if (row.node.data.kind === "file") {
    openStashFileDiff(row.node.data.reference, row.node.data.file);
  }
}

/** 储藏展开懒加载：拉取该储藏的 diff 文件列表 */
async function onStashExpand(reference: string) {
  if (stashFilesMap.value[reference] || stashFilesLoading.value[reference]) return;
  branchPickerOpen.value = false;
  stashFilesLoading.value = { ...stashFilesLoading.value, [reference]: true };
  try {
    const result = await gitPanelStashFiles(repoRoot.value, reference);
    stashFilesMap.value = { ...stashFilesMap.value, [reference]: result.entries || [] };
  } catch (error) {
    appendOutput(`stash show --name-status ${reference}`, null, error);
    stashFilesMap.value = { ...stashFilesMap.value, [reference]: [] };
  } finally {
    stashFilesLoading.value = { ...stashFilesLoading.value, [reference]: false };
  }
}

function openStashFileDiff(reference: string, file: GitPanelCommitFileEntry) {
  emit("openDiff", {
    workspacePath: repoRoot.value,
    path: file.path,
    staged: false,
    hash: reference,
  });
}

function commitFileStatusLabel(status: string) {
  switch (status) {
    case "A": return "A";
    case "D": return "D";
    case "R": return "R";
    case "C": return "C";
    case "M": return "M";
    default: return "?";
  }
}

function commitFileStatusClass(status: string) {
  switch (status) {
    case "A": return "text-success";
    case "D": return "text-error";
    case "R": return "text-warning";
    case "C": return "text-info";
    default: return "text-warning";
  }
}

// ==================== diff 打开 ====================
// 最后点击打开的 diff 文件路径（用于树行高亮）
const lastClickedDiffPath = ref("");

function openDiff(payload: { path: string; staged: boolean; untracked?: boolean }) {
  lastClickedDiffPath.value = payload.path;
  emit("openDiff", {
    workspacePath: repoRoot.value,
    path: payload.path,
    staged: payload.staged,
    untracked: payload.untracked,
  });
}

// ==================== 提交框自适应高度 ====================
// ==================== 外部变化自动刷新 ====================
// 仓库监听的启停与 gitPanel.watchChanged 订阅都在共享状态源里（与卡片墙共用一份）；
// 面板只订阅「当前仓库有变化」这一层，用来补刷 head/refs 相关的区域。
// 刷新粒度：workdir 只刷更改/暂存区（共享状态源负责）；head/refs 额外刷可见的提交历史/分支/储藏。
let unlistenExternalChange: (() => void) | null = null;

function refreshRefsDependentVisibleArea() {
  if (!repoRoot.value || historyCollapsed.value) return;
  if (activeGitTab.value === "commits") {
    void loadHistory(true);
  } else if (activeGitTab.value === "stashes") {
    void loadStashes(true);
  } else if (activeGitTab.value === "branches") {
    void Promise.all([loadBranches(true), loadRemotes()]);
  }
}

// 共享状态源已过滤出属于当前仓库的外部变化，并已刷新 status；
// 这里只负责 head/refs 信号下的提交历史 / 储藏 / 分支（含 remotes）刷新
function handleExternalChange(payload: GitPanelWatchEventPayload) {
  const targets = decideGitPanelRefreshTargets({
    hasRepoRoot: !!repoRoot.value,
    isCurrentRepo: true,
    historyCollapsed: historyCollapsed.value,
    activeGitTab: activeGitTab.value,
    workdirChanged: !!payload?.workdirChanged,
    headChanged: !!payload?.headChanged,
    refsChanged: !!payload?.refsChanged,
  });
  for (const target of targets) {
    if (target === "history") {
      void loadHistory(true);
    } else if (target === "stashes") {
      void loadStashes(true);
    } else if (target === "branches") {
      void Promise.all([loadBranches(true), loadRemotes()]);
    }
  }
}

function handleWindowFocusRefresh() {
  appendTransportProbeLog("[Git监视诊断]", "窗口获得焦点", {
    page: window.location?.pathname || "",
    visibility: document.visibilityState || "",
    focused: typeof document.hasFocus === "function" ? document.hasFocus() : false,
    root: repoRoot.value,
  }, "debug");
  if (!repoRoot.value) return;
  void loadStatus(true);
  refreshRefsDependentVisibleArea();
}

// ==================== 生命周期 ====================
const commitInputRef = ref<HTMLTextAreaElement | null>(null);

function autoGrowCommitInput() {
  const el = commitInputRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 96)}px`;
}

function resetCommitInputHeight() {
  const el = commitInputRef.value;
  if (!el) return;
  el.style.height = "";
}

onMounted(() => {
  restoreGitTab();
  restoreChangesViewMode();
  // 相对时间基准低频自增：面板可见期间让「多久之前」跟随当前时刻
  relativeNowTimer = window.setInterval(() => {
    relativeNowTick.value = Date.now();
  }, 60_000);
  // 声明面板占用：仓库根由面板控制，并让共享状态源开启仓库监听
  acquirePanel();
  unlistenExternalChange = onExternalChange(handleExternalChange);
  window.addEventListener("focus", handleWindowFocusRefresh);
  void loadDiscover().then(() => {
    if (repoRoot.value) {
      ensureVisibleData();
    }
  });
});

// 会话切换时（sessionKey 变化）重新恢复该会话记住的 git 标签与视图模式
watch(
  () => props.sessionKey,
  () => {
    restoreGitTab();
    restoreChangesViewMode();
  },
);

onBeforeUnmount(() => {
  unlistenExternalChange?.();
  unlistenExternalChange = null;
  window.removeEventListener("focus", handleWindowFocusRefresh);
  if (relativeNowTimer) {
    window.clearInterval(relativeNowTimer);
    relativeNowTimer = 0;
  }
  // 释放面板占用：共享状态源按引用计数决定是否停止仓库监听，其他消费方（卡片墙）仍在时继续
  releasePanel();
  logObserver?.disconnect();
  logObserver = undefined;
  branchesObserver?.disconnect();
  branchesObserver = undefined;
  stashObserver?.disconnect();
  stashObserver = undefined;
  window.removeEventListener("pointerdown", handleGlobalPointerDownForCommitCard, true);
  window.removeEventListener("keydown", handleCommitCardKeydown);
  window.removeEventListener("pointerdown", handleGlobalPointerDownForStashMenu, true);
  window.removeEventListener("keydown", handleStashMenuKeydown);
});
</script>

<style scoped>
.git-panel-scroller {
  scrollbar-width: thin;
  /* 抵消全局 overflow 容器的 both-edges gutter，避免左右留白 */
  scrollbar-gutter: auto;
}

.git-panel-graph {
  scrollbar-width: thin;
}

/* 提交输入框：textarea 默认 min-height 5rem 太大，去掉后初始即单行；隐藏滚动条避免占位不对称 */
.git-panel-commit-input {
  min-height: 0;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.git-panel-commit-input::-webkit-scrollbar {
  display: none;
}
</style>
