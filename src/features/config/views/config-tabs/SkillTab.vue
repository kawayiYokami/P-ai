<template>
  <SettingsPageShell :breadcrumb="skillBreadcrumb" header-class="">
    <template #left>
      <div v-if="!selectedSkill" class="relative w-full min-w-0 sm:w-60 sm:min-w-60 sm:flex-none">
        <input
          v-model="searchQuery"
          type="text"
          class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
          :placeholder="t('config.skill.searchPlaceholder')"
        />
        <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
        <button
          v-if="searchQuery"
          type="button"
          class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
          :title="t('config.skill.clearSearch')"
          @click="searchQuery = ''"
        >
          ✕
        </button>
      </div>
    </template>

    <template #actions>
      <template v-if="selectedSkill">
        <div class="flex flex-wrap items-center gap-2">
            <!-- 自定义技能支持放弃修改与保存 -->
            <template v-if="!selectedSkill.isBuiltin">
              <button
                v-if="isDirty"
                class="btn btn-sm min-h-[2.25rem] btn-ghost gap-1.5 px-3"
                type="button"
                :disabled="savingSkill"
                :title="t('config.skill.discardTitle')"
                @click="discardChanges"
              >
                <RotateCcw class="h-4 w-4" />
                <span>{{ t("config.skill.discard") }}</span>
              </button>
              <button
                class="btn btn-sm min-h-[2.25rem] gap-1.5 px-3.5"
                :class="isDirty ? 'btn-primary' : 'bg-base-100'"
                type="button"
                :disabled="!isDirty || savingSkill"
                :title="t('config.skill.ctrlSSaveHint')"
                @click="saveCurrentSkill"
              >
                <span v-if="savingSkill" class="loading loading-spinner loading-xs"></span>
                <Save v-else class="h-4 w-4" />
                <span>{{ savingSkill ? t("common.saving") : t("common.save") }}</span>
              </button>
            </template>
            <button
              v-if="localFileSystemAvailable"
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              @click="openCurrentSkillDir"
              :disabled="loading"
              :title="t('config.skill.openFolderHint')"
            >
              <FolderOpen class="h-4 w-4" />
              <span>{{ t("config.skill.openFolder") }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              @click="reload"
              :disabled="loading"
              :title="t('common.refresh')"
            >
              <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
              <span>{{ t("common.refresh") }}</span>
            </button>
            <button
              v-if="!selectedSkill.isBuiltin"
              class="btn btn-sm min-h-[2.25rem] btn-ghost text-error gap-1.5 px-3"
              type="button"
              :disabled="loading"
              @click="confirmRemoveSkill(selectedSkill)"
            >
              <Trash2 class="h-4 w-4" />
              <span>{{ t("config.skill.delete") }}</span>
            </button>
          </div>
      </template>

      <template v-else>
        <div class="flex flex-wrap items-center gap-2">
            <button
              v-if="localFileSystemAvailable"
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              @click="openSkillsDir"
              :disabled="loading"
            >
              <FolderOpen class="h-4 w-4" />
              <span>{{ t("config.skill.openWorkspace") }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              @click="reload"
              :disabled="loading"
            >
              <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': loading }" />
              <span>{{ t("common.refresh") }}</span>
            </button>
          </div>
      </template>
    </template>

    <!-- 主体区域切换：一级卡片列表 ↔ 二级详情页 -->
    <Transition name="ecall-config-content" mode="out-in">
      <!-- 二级菜单：技能详情视图 -->
      <div v-if="selectedSkill" :key="'detail-body-' + selectedSkill.path" class="grid gap-3 pb-8">
        <!-- 技能信息卡片 -->
        <div class="card bg-base-100 border border-base-300 card-sm">
          <div class="card-header border-b border-base-300/60 px-4 py-2.5 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.skill.metadata") }}</span>
              <span v-if="selectedSkill.isBuiltin" class="badge badge-sm badge-neutral opacity-70 flex items-center gap-1">
                <Lock class="h-3 w-3" />
                {{ t("config.skill.builtinReadonly") }}
              </span>
              <span v-else-if="isMetaDirty" class="badge badge-sm badge-warning">
                {{ t("config.skill.modified") }}
              </span>
            </div>
            <button
              v-if="localFileSystemAvailable"
              class="btn btn-sm h-8 min-h-[2rem] btn-ghost gap-1.5 px-2.5 text-xs text-base-content/70 hover:text-base-content"
              type="button"
              :title="t('config.skill.openFolderHint')"
              @click="openCurrentSkillDir"
            >
              <FolderOpen class="h-3.5 w-3.5" />
              <span>{{ t("config.skill.openFolder") }}</span>
            </button>
          </div>

          <div class="card-body p-4 gap-4">
            <!-- 技能名称 -->
            <div class="flex flex-col gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-caption font-semibold opacity-60 uppercase tracking-wider">{{ t("config.skill.name") }}</span>
                <span v-if="!selectedSkill.isBuiltin && editingName !== selectedSkill.name" class="text-caption text-warning font-medium">{{ t("config.skill.modified") }}</span>
              </div>
              <!-- 非编辑状态：文本展示，点击就地进入编辑 -->
              <div
                v-if="!isEditingName"
                class="group flex items-center justify-between rounded-field border border-transparent px-3 py-2 -mx-3 transition-colors select-none"
                :class="selectedSkill.isBuiltin ? 'cursor-default' : 'cursor-pointer hover:border-base-300 hover:bg-base-200/60'"
                :title="selectedSkill.isBuiltin ? t('config.skill.builtinReadonly') : t('common.edit')"
                @click="startEditName"
              >
                <div class="text-base font-bold text-base-content break-all">
                  {{ editingName || t("config.skill.unnamed") }}
                </div>
                <div v-if="!selectedSkill.isBuiltin" class="flex items-center gap-1 opacity-0 group-hover:opacity-70 transition-opacity ml-2 shrink-0">
                  <Edit3 class="h-3.5 w-3.5 text-primary" />
                  <span class="text-caption text-primary">{{ t("common.edit") }}</span>
                </div>
              </div>
              <!-- 编辑状态：输入框 -->
              <div v-else class="flex items-center gap-2">
                <input
                  ref="nameInputRef"
                  v-model="editingName"
                  type="text"
                  class="input input-bordered input-sm h-9 w-full text-sm font-bold bg-base-100"
                  :placeholder="t('config.skill.namePlaceholder')"
                  @keydown.enter="finishEditName"
                  @keydown.esc="cancelEditName"
                />
                <button
                  class="btn btn-sm h-9 min-h-[2.25rem] px-3.5 btn-ghost shrink-0"
                  type="button"
                  :title="t('common.done')"
                  @click="finishEditName"
                >
                  {{ t("common.done") }}
                </button>
              </div>
            </div>

            <!-- 技能描述 -->
            <div class="flex flex-col gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-caption font-semibold opacity-60 uppercase tracking-wider">{{ t("config.skill.description") }}</span>
                <div class="flex items-center gap-2 font-mono">
                  <span v-if="!selectedSkill.isBuiltin && editingDescription !== selectedSkill.description" class="text-caption text-warning font-medium">{{ t("config.skill.modified") }}</span>
                  <span class="text-caption opacity-60">
                    {{ t("config.skill.charCount", { count: (editingDescription || '').length.toLocaleString() }) }}
                  </span>
                </div>
              </div>
              <!-- 非编辑状态：文本展示，点击就地进入编辑 -->
              <div
                v-if="!isEditingDesc"
                class="group flex items-start justify-between rounded-field border border-transparent p-3 -mx-3 transition-colors"
                :class="selectedSkill.isBuiltin ? 'cursor-default' : 'cursor-pointer hover:border-base-300 hover:bg-base-200/60'"
                :title="selectedSkill.isBuiltin ? t('config.skill.builtinReadonly') : t('common.edit')"
                @click="startEditDesc"
              >
                <div
                  class="text-xs leading-relaxed break-words flex-1"
                  :class="editingDescription ? 'text-base-content/80' : 'italic opacity-40'"
                >
                  {{ editingDescription || t("config.skill.noDescriptionPlaceholder") }}
                </div>
                <div v-if="!selectedSkill.isBuiltin" class="flex items-center gap-1 opacity-0 group-hover:opacity-70 transition-opacity ml-2 mt-0.5 shrink-0">
                  <Edit3 class="h-3.5 w-3.5 text-primary" />
                  <span class="text-caption text-primary">{{ t("common.edit") }}</span>
                </div>
              </div>
              <!-- 编辑状态：多行文本框 -->
              <div v-else class="flex flex-col gap-2">
                <textarea
                  ref="descTextareaRef"
                  v-model="editingDescription"
                  class="textarea textarea-bordered text-xs leading-relaxed w-full min-h-[5.5rem] p-3 resize-y bg-base-100"
                  :placeholder="t('config.skill.descPlaceholder')"
                  @keydown.esc="cancelEditDesc"
                ></textarea>
                <div class="flex items-center justify-between text-caption opacity-60">
                  <span>{{ t("config.skill.escOrDoneHint") }}</span>
                  <button
                    class="btn btn-sm h-8 min-h-[2rem] px-4 btn-ghost text-xs"
                    type="button"
                    @click="finishEditDesc"
                  >
                    {{ t("common.done") }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 附加文件列表卡片（仅在存在附加文件时显示） -->
        <div v-if="(selectedSkill.additionalFiles || []).length > 0" class="card bg-base-100 border border-base-300 card-sm">
          <div class="card-header border-b border-base-300/60 px-4 py-2.5 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.skill.additionalFiles") }}</span>
              <span class="badge badge-sm badge-neutral">
                {{ (selectedSkill.additionalFiles || []).length }}
              </span>
            </div>
          </div>
          <div class="card-body p-3">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
              <div
                v-for="file in selectedSkill.additionalFiles"
                :key="file.relativePath"
                class="flex items-center justify-between gap-2.5 rounded border border-base-200 bg-base-200/30 p-2.5 hover:bg-base-200/60 transition-colors"
              >
                <div class="flex items-center gap-2 min-w-0 flex-1">
                  <component :is="getFileIcon(file.name)" class="h-4 w-4 shrink-0 text-primary opacity-80" />
                  <div class="min-w-0 flex-1">
                    <div class="text-xs font-medium truncate" :title="file.relativePath">
                      {{ file.name }}
                    </div>
                    <div class="text-caption opacity-50 truncate font-mono" :title="file.relativePath">
                      {{ file.relativePath }} · {{ formatFileSize(file.sizeBytes) }}
                    </div>
                  </div>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <button
                    class="btn btn-sm h-8 min-h-[2rem] btn-ghost gap-1 px-2.5"
                    type="button"
                    :title="t('config.skill.viewFile')"
                    @click="previewAdditionalFile(file)"
                  >
                    <Eye class="h-3.5 w-3.5" />
                    <span class="text-xs">{{ t("config.skill.viewFile") }}</span>
                  </button>
                  <button
                    class="btn btn-sm h-8 w-8 min-h-[2rem] btn-ghost btn-circle"
                    type="button"
                    :title="t('common.copy')"
                    @click="copySkillPath(file.relativePath)"
                  >
                    <Copy class="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 正文展示与编辑卡片 -->
        <div class="card bg-base-100 border border-base-300 card-sm overflow-hidden">
          <div class="card-header border-b border-base-300/60 px-4 py-2.5 flex flex-wrap items-center justify-between gap-2.5">
            <div class="flex items-center gap-2">
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.skill.content") }}</span>
              <span class="text-caption opacity-60 font-mono">
                {{ t("config.skill.charCount", { count: (editingContent || '').length.toLocaleString() }) }}
              </span>
              <span v-if="selectedSkill.isBuiltin" class="badge badge-sm badge-neutral opacity-70">{{ t("config.skill.readonly") }}</span>
              <span v-else-if="isContentDirty" class="badge badge-sm badge-warning">{{ t("config.skill.modified") }}</span>
            </div>

            <div class="flex items-center gap-2">
              <!-- 仅自定义技能支持切换为纯文本编辑模式 -->
              <div v-if="!selectedSkill.isBuiltin" class="join">
                <button
                  class="btn btn-sm h-8 min-h-[2rem] join-item px-3"
                  :class="contentMode === 'preview' ? 'btn-primary' : 'bg-base-100'"
                  type="button"
                  @click="contentMode = 'preview'"
                >
                  <Eye class="h-3.5 w-3.5 mr-1" />
                  <span class="text-xs">{{ t("config.skill.previewMarkdown") }}</span>
                </button>
                <button
                  class="btn btn-sm h-8 min-h-[2rem] join-item px-3"
                  :class="contentMode === 'edit' ? 'btn-primary' : 'bg-base-100'"
                  type="button"
                  @click="contentMode = 'edit'"
                >
                  <Edit3 class="h-3.5 w-3.5 mr-1" />
                  <span class="text-xs">{{ t("config.skill.editText") }}</span>
                </button>
              </div>

              <!-- 复制正文 -->
              <button
                class="btn btn-sm h-8 min-h-[2rem] btn-ghost gap-1.5 px-3"
                type="button"
                @click="copySkillContent"
              >
                <Check v-if="copiedContent" class="h-3.5 w-3.5 text-success" />
                <Copy v-else class="h-3.5 w-3.5" />
                <span class="text-xs">{{ copiedContent ? t("config.skill.copySuccess") : t("config.skill.copyAll") }}</span>
              </button>
            </div>
          </div>

          <!-- 模式一：Markdown 渲染预览 -->
          <OverlayScrollArea v-if="contentMode === 'preview'" scroller-class="p-4 bg-base-100 max-h-[65vh]">
            <AppMarkdownRenderer
              v-if="editingContent.trim()"
              :text="editingContent"
              variant="document"
            />
            <div v-else class="py-12 text-center text-xs opacity-50">
              {{ t("config.skill.emptyContentHint") }}
            </div>
          </OverlayScrollArea>

          <!-- 模式二：纯文本编辑 -->
          <div v-else class="flex flex-col bg-base-200/30 p-3 gap-2">
            <textarea
              ref="textareaRef"
              v-model="editingContent"
              class="textarea textarea-bordered font-mono text-xs w-full min-h-[24rem] max-h-[65vh] leading-relaxed p-3 bg-base-100 select-text resize-y"
              :placeholder="t('config.skill.contentPlaceholder')"
              @keydown="handleTextareaKeydown"
            ></textarea>
            <div class="flex items-center justify-between px-1 text-caption opacity-60 font-mono">
              <span>{{ t("config.skill.ctrlSSaveHint") }}</span>
              <span>{{ t("config.skill.lineCount", { count: editingContent.split('\n').length.toLocaleString() }) }}</span>
            </div>
          </div>
        </div>

        <div v-if="statusText" class="text-xs" :class="statusError ? 'text-error' : 'opacity-60'">
          {{ statusText }}
        </div>
      </div>

      <!-- 一级概览：卡片矩阵视图 -->
      <div v-else key="overview-grid" class="grid gap-5 pb-8">
        <div v-if="loading && skills.length === 0" class="flex items-center justify-center py-16 text-sm opacity-60">
          <RefreshCw class="mr-2 h-4 w-4 animate-spin" />
          <span>{{ t("config.skill.loading") }}</span>
        </div>

        <div v-else-if="filteredSkills.length === 0" class="rounded-box border border-dashed border-base-300 p-8 text-center">
          <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-base-200 text-base-content/50">
            <Code class="h-6 w-6" />
          </div>
          <div class="mt-3 text-sm font-medium">
            {{ searchQuery ? t("config.skill.noSearchMatch") : t("config.skill.noSkills") }}
          </div>
          <div class="mt-1 text-xs opacity-60">
            {{ searchQuery ? t("config.skill.tryAnotherSearch") : t("config.skill.addSkillGuide") }}
          </div>
          <div class="mt-4 flex justify-center gap-2">
            <button v-if="searchQuery" class="btn btn-sm btn-ghost" type="button" @click="searchQuery = ''">
              {{ t("config.skill.clearSearch") }}
            </button>
            <button v-if="localFileSystemAvailable" class="btn btn-sm bg-base-200" type="button" @click="openSkillsDir">
              <FolderOpen class="h-3.5 w-3.5 mr-1" />
              {{ t("config.skill.openWorkspace") }}
            </button>
          </div>
        </div>

        <template v-else>
          <!-- 区域一：用户自定义技能 -->
          <div class="space-y-2.5">
            <div class="flex items-center justify-between px-0.5">
              <div class="flex items-center gap-2">
                <Code class="h-4 w-4 text-primary" />
                <span class="text-sm font-semibold">{{ t("config.skill.customSkills") }}</span>
                <span class="badge badge-sm badge-neutral">{{ userSkills.length }}</span>
              </div>
              <span class="text-caption opacity-50">{{ t("config.skill.customSubtitle") }}</span>
            </div>

            <div v-if="userSkills.length === 0" class="rounded-box border border-dashed border-base-300 p-6 text-center">
              <p class="text-xs opacity-60">
                {{ searchQuery ? t("config.skill.noSearchMatch") : t("config.skill.noCustomSkills") }}
              </p>
            </div>

            <div v-else class="config-grid-auto-sm">
              <div
                v-for="item in userSkills"
                :key="item.path"
                role="button"
                tabindex="0"
                class="rounded-box border border-base-200 bg-base-100 p-4 hover:border-primary/50 transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] group"
                @click="selectSkill(item.path)"
                @keydown.enter.prevent="selectSkill(item.path)"
                @keydown.space.prevent="selectSkill(item.path)"
              >
                <!-- 头部：技能名称 -->
                <div class="card-title-bar card-title-bar--accent text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors flex-1 min-w-0">
                  {{ item.name }}
                </div>

                <!-- 中部：描述预览（无描述时呈现优雅占位，高度固定 2.5rem 保持整齐） -->
                <p class="text-xs line-clamp-2 leading-relaxed min-h-[2.5rem] break-words" :class="item.description ? 'text-base-content/70' : 'text-base-content/40 italic'">
                  {{ item.description || t("config.skill.noDescription") }}
                </p>

                <!-- 底栏：内容规模 + 启用开关 + 进入指示 -->
                <div class="flex items-center justify-between border-t border-base-200 pt-2.5 text-caption opacity-60 font-mono">
                  <span
                    class="hover:text-primary transition-colors cursor-help"
                    :title="t('config.skill.contentWordsTooltip', { count: (item.content || '').length.toLocaleString() })"
                  >
                    {{ t("config.skill.contentWords", { count: (item.content || '').length.toLocaleString() }) }}
                  </span>
                  <div class="flex items-center gap-2">
                    <input
                      type="checkbox"
                      class="toggle toggle-xs toggle-primary"
                      :checked="item.enabled ?? true"
                      :disabled="togglingSkillName === item.name"
                      :title="(item.enabled ?? true) ? t('config.skill.toggleEnabledOn') : t('config.skill.toggleEnabledOff')"
                      @click.stop
                      @keydown.enter.stop
                      @keydown.space.stop
                      @change="toggleSkillEnabled(item)"
                    />
                    <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
                  </div>
                </div>
              </div>

              <!-- TODO: 新增技能卡位置预留——技能需从 zip 导入、交互尚未设计，暂不提供新增入口 -->
            </div>
          </div>

          <!-- 区域二：系统内置技能（默认折叠） -->
          <div v-if="builtinSkills.length > 0" class="rounded-box border border-base-300 bg-base-100/70 overflow-hidden">
            <!-- 折叠栏头部 -->
            <div
              role="button"
              tabindex="0"
              class="flex cursor-pointer items-center justify-between px-4 py-3 select-none hover:bg-base-200/50 transition-colors"
              @click="builtinCollapsed = !builtinCollapsed"
              @keydown.enter.prevent="builtinCollapsed = !builtinCollapsed"
              @keydown.space.prevent="builtinCollapsed = !builtinCollapsed"
            >
              <div class="flex items-center gap-2.5">
                <ChevronRight
                  class="h-4 w-4 text-base-content/60 transition-transform duration-200"
                  :class="{ 'rotate-90': !builtinCollapsed }"
                />
                <ShieldCheck class="h-4 w-4 text-primary opacity-80" />
                <span class="text-sm font-semibold">{{ t("config.skill.builtinSkills") }}</span>
                <span class="badge badge-sm badge-neutral">{{ builtinSkills.length }}</span>
              </div>
              <div class="text-caption opacity-50 flex items-center gap-1.5">
                <span>{{ t("config.skill.builtinSubtitle") }}</span>
                <span class="text-xs">({{ builtinCollapsed ? t("common.expand") : t("common.collapse") }})</span>
              </div>
            </div>

            <!-- 折叠卡片内容区 -->
            <div v-show="!builtinCollapsed" class="p-3 border-t border-base-300 bg-base-200/20">
              <div class="config-grid-auto-sm">
                <div
                  v-for="item in builtinSkills"
                  :key="item.path"
                  role="button"
                  tabindex="0"
                  class="rounded-box border border-base-200 bg-base-100 p-4 hover:border-primary/50 transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] group"
                  @click="selectSkill(item.path)"
                  @keydown.enter.prevent="selectSkill(item.path)"
                  @keydown.space.prevent="selectSkill(item.path)"
                >
                  <!-- 头部：技能名称 + 内置标签 -->
                  <div class="flex items-center justify-between gap-2 min-w-0">
                    <div class="card-title-bar card-title-bar--accent text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors flex-1 min-w-0">
                      {{ item.name }}
                    </div>
                    <span class="badge badge-neutral badge-xs opacity-70 shrink-0">{{ t("config.skill.builtinTag") }}</span>
                  </div>

                  <!-- 中部：描述预览（无描述时呈现优雅占位，高度固定 2.5rem 保持整齐） -->
                  <p class="text-xs line-clamp-2 leading-relaxed min-h-[2.5rem] break-words" :class="item.description ? 'text-base-content/70' : 'text-base-content/40 italic'">
                    {{ item.description || t("config.skill.noDescription") }}
                  </p>

                  <!-- 底栏：内容规模 + 启用开关 + 进入指示 -->
                  <div class="flex items-center justify-between border-t border-base-200 pt-2.5 text-caption opacity-60 font-mono">
                    <span
                      class="hover:text-primary transition-colors cursor-help"
                      :title="t('config.skill.contentWordsTooltip', { count: (item.content || '').length.toLocaleString() })"
                    >
                      {{ t("config.skill.contentWords", { count: (item.content || '').length.toLocaleString() }) }}
                    </span>
                    <div class="flex items-center gap-2">
                      <input
                        type="checkbox"
                        class="toggle toggle-xs toggle-primary"
                        :checked="item.enabled ?? true"
                        :disabled="togglingSkillName === item.name"
                        :title="(item.enabled ?? true) ? t('config.skill.toggleEnabledOn') : t('config.skill.toggleEnabledOff')"
                        @click.stop
                        @keydown.enter.stop
                        @keydown.space.stop
                        @change="toggleSkillEnabled(item)"
                      />
                      <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </template>

        <div v-if="statusText" class="text-xs" :class="statusError ? 'text-error' : 'opacity-60'">
          {{ statusText }}
        </div>
      </div>
    </Transition>

    <!-- 附加文件预览模态弹窗 -->
    <dialog ref="filePreviewModalRef" class="modal">
      <div class="modal-box max-w-2xl p-4">
        <div class="flex items-center justify-between border-b border-base-300 pb-3">
          <div class="flex items-center gap-2 min-w-0">
            <component :is="getFileIcon(previewFileName)" class="h-5 w-5 text-primary shrink-0" />
            <div class="min-w-0">
              <h3 class="text-sm font-bold truncate">{{ previewFileName }}</h3>
              <p class="text-caption opacity-60 font-mono truncate">{{ previewFileRelPath }}</p>
            </div>
          </div>
          <button class="btn btn-ghost btn-circle h-9 w-9 min-h-[2.25rem] text-base-content/70 hover:text-base-content" type="button" @click="closeFilePreview">
            ✕
          </button>
        </div>

        <div class="py-3">
          <div v-if="previewFileLoading" class="py-12 text-center text-xs opacity-60">
            <span class="loading loading-spinner loading-sm mr-2"></span>
            {{ t("config.skill.readingFile") }}
          </div>
          <div v-else-if="previewFileError" class="py-8 text-center text-xs text-error">
            {{ previewFileError }}
          </div>
          <OverlayScrollArea v-else scroller-class="rounded-box bg-base-200/50 p-3 max-h-[55vh]">
            <pre class="font-mono text-xs whitespace-pre-wrap break-words leading-relaxed select-text">{{ previewFileContent }}</pre>
          </OverlayScrollArea>
        </div>

        <div class="modal-action mt-2 flex justify-between items-center">
          <div class="text-caption opacity-50 font-mono">
            {{ previewFileContent ? t("config.skill.charCount", { count: previewFileContent.length.toLocaleString() }) : '' }}
          </div>
          <div class="flex gap-2">
            <button
              class="btn btn-sm min-h-[2.25rem] h-9 px-4 bg-base-200 gap-1.5"
              type="button"
              :disabled="!previewFileContent"
              @click="copyFilePreviewContent"
            >
              <Check v-if="previewFileCopied" class="h-4 w-4 text-success" />
              <Copy v-else class="h-4 w-4" />
              <span>{{ previewFileCopied ? t("config.skill.copySuccess") : t("config.skill.copyContent") }}</span>
            </button>
            <button class="btn btn-sm min-h-[2.25rem] h-9 px-5" type="button" @click="closeFilePreview">
              {{ t("common.close") }}
            </button>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click="closeFilePreview">close</button>
      </form>
    </dialog>
  </SettingsPageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import {
  Check,
  ChevronRight,
  Code,
  Copy,
  Edit3,
  Eye,
  FileCode,
  FileText,
  Files,
  FolderOpen,
  Lock,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  ShieldCheck,
  Trash2,
} from "@lucide/vue";
import {
  getTransportCapabilities,
  invokeTauri,
  openTransportSkillDirectory,
  openTransportSkillWorkspaceDirectory,
  readTransportSkillFile,
  removeTransportSkill,
  saveTransportSkill,
  setTransportSkillEnabled,
} from "../../../../services/tauri-api";
import type { SkillFileItem, SkillListResult, SkillSummaryItem } from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbItem } from "../../components/SettingsBreadcrumb.vue";
import AppMarkdownRenderer from "../../../chat/markdown/AppMarkdownRenderer.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";

const { t } = useI18n();

const loading = ref(false);
const savingSkill = ref(false);
const statusText = ref("");
const statusError = ref(false);
const skills = ref<SkillSummaryItem[]>([]);
const selectedSkillPath = ref<string | null>(null);
const searchQuery = ref("");
const copiedContent = ref(false);
const copiedPath = ref(false);

// 元数据编辑状态
const isEditingName = ref(false);
const isEditingDesc = ref(false);
const editingName = ref("");
const editingDescription = ref("");
const nameInputRef = ref<HTMLInputElement | null>(null);
const descTextareaRef = ref<HTMLTextAreaElement | null>(null);

// 正文模式：预览 Markdown (preview) 或纯文本编辑 (edit)
const contentMode = ref<"preview" | "edit">("preview");
const editingContent = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null);

// 附加文件弹窗状态
const filePreviewModalRef = ref<HTMLDialogElement | null>(null);
const previewFileName = ref("");
const previewFileRelPath = ref("");
const previewFileContent = ref("");
const previewFileLoading = ref(false);
const previewFileError = ref("");
const previewFileCopied = ref(false);

const localFileSystemAvailable = getTransportCapabilities().localFileSystem;

const builtinCollapsed = ref(true);

const selectedSkill = computed(() => {
  if (!selectedSkillPath.value) return null;
  return skills.value.find((v) => v.path === selectedSkillPath.value) ?? null;
});

// 元数据是否修改
const isMetaDirty = computed(() => {
  if (!selectedSkill.value || selectedSkill.value.isBuiltin) return false;
  return (
    editingName.value.trim() !== (selectedSkill.value.name || "").trim() ||
    editingDescription.value.trim() !== (selectedSkill.value.description || "").trim()
  );
});

// 正文是否修改
const isContentDirty = computed(() => {
  if (!selectedSkill.value || selectedSkill.value.isBuiltin) return false;
  return editingContent.value !== (selectedSkill.value.content || "");
});

// 是否存在任何未保存的修改（内置技能不可编辑，故永为 false）
const isDirty = computed(() => isMetaDirty.value || isContentDirty.value);

const skillBreadcrumb = computed<SettingsBreadcrumbItem[]>(() => {
  const skill = selectedSkill.value;
  if (!skill) return [{ label: t("config.tabs.skill") }];
  return [
    { label: t("config.tabs.skill"), title: t("config.skill.backToList"), onClick: backToList },
    {
      label: editingName.value || skill.name,
      badge: skill.isBuiltin
        ? t("config.skill.builtin")
        : isDirty.value
          ? t("config.skill.unsaved")
          : undefined,
      badgeClass: skill.isBuiltin ? "badge-neutral" : "badge-warning",
    },
  ];
});

// 当切换选中的技能时，同步初始化编辑区内容并默认切回预览模式
watch(
  () => selectedSkill.value,
  (newSkill) => {
    if (newSkill) {
      editingContent.value = newSkill.content || "";
      editingName.value = newSkill.name || "";
      editingDescription.value = newSkill.description || "";
      contentMode.value = "preview";
      isEditingName.value = false;
      isEditingDesc.value = false;
    } else {
      editingContent.value = "";
      editingName.value = "";
      editingDescription.value = "";
    }
  },
  { immediate: true }
);

const filteredSkills = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return skills.value;
  return skills.value.filter(
    (item) =>
      item.name.toLowerCase().includes(q) ||
      (item.description && item.description.toLowerCase().includes(q)) ||
      item.path.toLowerCase().includes(q)
  );
});

const userSkills = computed(() => filteredSkills.value.filter((item) => !item.isBuiltin));
const builtinSkills = computed(() => filteredSkills.value.filter((item) => !!item.isBuiltin));

// 搜索时若有匹配关键词，自动展开内置技能折叠区
watch(
  () => searchQuery.value,
  (q) => {
    if (q.trim() && builtinSkills.value.length > 0) {
      builtinCollapsed.value = false;
    }
  }
);

function selectSkill(path: string) {
  selectedSkillPath.value = path;
}

const togglingSkillName = ref<string | null>(null);

async function toggleSkillEnabled(item: SkillSummaryItem) {
  if (togglingSkillName.value) return;
  const target = !(item.enabled ?? true);
  togglingSkillName.value = item.name;
  try {
    const result = await setTransportSkillEnabled(item.name, target);
    skills.value = result?.skills || [];
    setStatus(t("config.skill.toggleSuccess", { name: item.name }));
  } catch (error) {
    setStatus(`${t("config.skill.toggleFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    togglingSkillName.value = null;
  }
}

function startEditName() {
  if (!selectedSkill.value || selectedSkill.value.isBuiltin) return;
  isEditingName.value = true;
  nextTick(() => {
    nameInputRef.value?.focus();
    nameInputRef.value?.select();
  });
}

function finishEditName() {
  isEditingName.value = false;
}

function cancelEditName() {
  if (selectedSkill.value) {
    editingName.value = selectedSkill.value.name || "";
  }
  isEditingName.value = false;
}

function startEditDesc() {
  if (!selectedSkill.value || selectedSkill.value.isBuiltin) return;
  isEditingDesc.value = true;
  nextTick(() => {
    descTextareaRef.value?.focus();
  });
}

function finishEditDesc() {
  isEditingDesc.value = false;
}

function cancelEditDesc() {
  if (selectedSkill.value) {
    editingDescription.value = selectedSkill.value.description || "";
  }
  isEditingDesc.value = false;
}

function confirmRemoveSkill(skill: SkillSummaryItem) {
  if (window.confirm(t("config.skill.deleteConfirm", { name: skill.name }))) {
    void removeSkill(skill);
  }
}

async function removeSkill(skill: SkillSummaryItem) {
  try {
    const result = await removeTransportSkill(skill.path);
    skills.value = result?.skills || [];
    selectedSkillPath.value = null;
    setStatus(t("config.skill.deleted", { name: skill.name }));
  } catch (error) {
    setStatus(`${t("config.skill.deleteFailed")}: ${toErrorMessage(error)}`, true);
  }
}

function backToList() {
  if (isDirty.value) {
    const confirmLeave = window.confirm(t("config.skill.confirmLeaveUnsaved"));
    if (!confirmLeave) return;
  }
  selectedSkillPath.value = null;
}

function discardChanges() {
  if (!selectedSkill.value) return;
  editingContent.value = selectedSkill.value.content || "";
  editingName.value = selectedSkill.value.name || "";
  editingDescription.value = selectedSkill.value.description || "";
  isEditingName.value = false;
  isEditingDesc.value = false;
  setStatus(t("config.skill.discardedHint"));
}

async function saveCurrentSkill() {
  if (!selectedSkill.value || savingSkill.value || selectedSkill.value.isBuiltin) return;
  savingSkill.value = true;
  try {
    const updated = await saveTransportSkill(
      selectedSkill.value.path,
      editingContent.value,
      editingName.value.trim() || selectedSkill.value.name,
      editingDescription.value.trim()
    );
    // 更新本地 skills 列表中的对应项
    const idx = skills.value.findIndex((s) => s.path === updated.path);
    if (idx !== -1) {
      skills.value[idx] = updated;
    }
    editingContent.value = updated.content;
    editingName.value = updated.name;
    editingDescription.value = updated.description;
    isEditingName.value = false;
    isEditingDesc.value = false;
    setStatus(t("config.skill.saveSuccessWithName", { name: updated.name }));
  } catch (error) {
    setStatus(`${t("config.skill.saveFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    savingSkill.value = false;
  }
}

function handleTextareaKeydown(event: KeyboardEvent) {
  // 支持 Tab 键缩进
  if (event.key === "Tab") {
    event.preventDefault();
    const textarea = textareaRef.value;
    if (!textarea) return;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const val = editingContent.value;
    editingContent.value = val.substring(0, start) + "  " + val.substring(end);
    nextTick(() => {
      textarea.selectionStart = textarea.selectionEnd = start + 2;
    });
    return;
  }
  // Ctrl+S / Cmd+S 快捷保存
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    if (isDirty.value) {
      void saveCurrentSkill();
    }
  }
}

function getFileIcon(fileName: string): Component {
  const lower = fileName.toLowerCase();
  if (lower.endsWith(".py") || lower.endsWith(".js") || lower.endsWith(".ts") || lower.endsWith(".sh") || lower.endsWith(".bat")) {
    return FileCode;
  }
  if (lower.endsWith(".md") || lower.endsWith(".txt") || lower.endsWith(".json") || lower.endsWith(".yaml") || lower.endsWith(".yml")) {
    return FileText;
  }
  return Files;
}

function formatFileSize(bytes: number): string {
  if (!bytes || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getSkillDirPath(skillMdPath: string): string {
  const normalized = skillMdPath.replace(/\\/g, "/");
  const idx = normalized.lastIndexOf("/");
  if (idx !== -1) {
    return normalized.substring(0, idx);
  }
  return normalized;
}

async function previewAdditionalFile(file: SkillFileItem) {
  if (!selectedSkill.value) return;
  const skillDir = getSkillDirPath(selectedSkill.value.path);
  const fullPath = `${skillDir}/${file.relativePath}`;
  previewFileName.value = file.name;
  previewFileRelPath.value = file.relativePath;
  previewFileContent.value = "";
  previewFileError.value = "";
  previewFileCopied.value = false;
  previewFileLoading.value = true;
  filePreviewModalRef.value?.showModal();

  try {
    const content = await readTransportSkillFile(fullPath);
    previewFileContent.value = content;
  } catch (error) {
    previewFileError.value = `读取文件失败: ${toErrorMessage(error)}`;
  } finally {
    previewFileLoading.value = false;
  }
}

function closeFilePreview() {
  filePreviewModalRef.value?.close();
}

async function copyFilePreviewContent() {
  if (!previewFileContent.value) return;
  try {
    await navigator.clipboard.writeText(previewFileContent.value);
    previewFileCopied.value = true;
    setTimeout(() => {
      previewFileCopied.value = false;
    }, 2000);
  } catch (error) {
    setStatus(`复制失败: ${toErrorMessage(error)}`, true);
  }
}


function formatSkillStats(content: string): string {
  if (!content) return "0 字";
  return `${content.length.toLocaleString()} 字`;
}

async function copySkillContent() {
  const text = editingContent.value || selectedSkill.value?.content || "";
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    copiedContent.value = true;
    setTimeout(() => {
      copiedContent.value = false;
    }, 2000);
  } catch (error) {
    setStatus(`复制失败: ${toErrorMessage(error)}`, true);
  }
}

async function copySkillPath(path: string) {
  if (!path) return;
  try {
    await navigator.clipboard.writeText(path);
    copiedPath.value = true;
    setTimeout(() => {
      copiedPath.value = false;
    }, 2000);
  } catch (error) {
    setStatus(`复制失败: ${toErrorMessage(error)}`, true);
  }
}

function setStatus(text: string, isError = false) {
  statusText.value = text;
  statusError.value = isError;
}

async function reload() {
  loading.value = true;
  try {
    const result = await invokeTauri<SkillListResult>("mcp_list_skills");
    skills.value = result?.skills || [];
    if (selectedSkillPath.value && !skills.value.some((v) => v.path === selectedSkillPath.value)) {
      selectedSkillPath.value = null;
    }
    if ((result?.errors?.length || 0) > 0) {
      setStatus(`已加载 ${skills.value.length} 个 SKILL，${result.errors.length} 个目录读取失败`, true);
    } else {
      setStatus(`已加载 ${skills.value.length} 个 SKILL`);
    }
  } catch (error) {
    setStatus(`刷新失败: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function openSkillsDir() {
  if (!localFileSystemAvailable || loading.value) return;
  loading.value = true;
  try {
    const opened = await openTransportSkillWorkspaceDirectory();
    setStatus(`已打开目录: ${opened}`);
  } catch (error) {
    setStatus(`打开目录失败: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function openCurrentSkillDir() {
  if (!localFileSystemAvailable || !selectedSkill.value || loading.value) return;
  loading.value = true;
  try {
    const opened = await openTransportSkillDirectory(selectedSkill.value.path);
    setStatus(`已打开文件夹: ${opened}`);
  } catch (error) {
    setStatus(`打开文件夹失败: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

function handleGlobalKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (filePreviewModalRef.value?.open) {
      closeFilePreview();
      return;
    }
    if (isEditingName.value) {
      cancelEditName();
      return;
    }
    if (isEditingDesc.value) {
      cancelEditDesc();
      return;
    }
    if (selectedSkill.value) {
      backToList();
      return;
    }
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    if (selectedSkill.value && isDirty.value) {
      event.preventDefault();
      void saveCurrentSkill();
    }
  }
}

onMounted(() => {
  void reload();
  window.addEventListener("keydown", handleGlobalKeydown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
});
</script>
