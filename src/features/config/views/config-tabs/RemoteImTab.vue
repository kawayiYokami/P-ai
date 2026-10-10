<template>
  <SettingsPageShell
    :breadcrumb="remoteImBreadcrumb"
    :no-scroll="inDetailMode"
    :content-class="inDetailMode ? 'mx-auto max-w-5xl h-full min-h-0' : undefined"
    header-class=""
  >
    <template #left>
      <ChannelBehaviorSettingsModal
        v-if="inDetailMode && selectedChannel"
        :channel="selectedChannel"
        :save-config-action="props.saveConfigAction"
        :set-status-action="props.setStatusAction"
      />
      <div v-else class="relative w-full min-w-0 sm:w-60 sm:min-w-60 sm:flex-none">
        <input
          v-model="channelSearchQuery"
          type="text"
          class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
          :placeholder="t('config.remoteIm.searchPlaceholder')"
        />
        <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
        <button
          v-if="channelSearchQuery"
          type="button"
          class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
          :title="t('common.clear')"
          @click="channelSearchQuery = ''"
        >
          ✕
        </button>
      </div>
    </template>

    <template #actions>
      <div v-if="inDetailMode && selectedChannel" class="flex flex-wrap items-center gap-2">
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              :title="t('config.remoteIm.viewLogs')"
              @click="openChannelLogsModalForChannel(selectedChannel.id)"
            >
              <ScrollText class="h-4 w-4" />
              <span>{{ t("config.remoteIm.viewLogs") }}</span>
            </button>
            <button
              class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
              type="button"
              :title="t('common.refresh')"
              :disabled="contactsLoading"
              @click="refreshContacts"
            >
              <RefreshCw class="h-4 w-4" :class="{ 'animate-spin': contactsLoading }" />
              <span>{{ t("common.refresh") }}</span>
            </button>
            <button
              v-if="channelDirty"
              class="btn btn-sm min-h-[2.25rem] btn-primary gap-1.5 px-3.5"
              type="button"
              :disabled="saving || isChannelOperationBusy(selectedChannel.id)"
              :title="t('common.save')"
              @click="saveChannels"
            >
              <span v-if="saving" class="loading loading-spinner loading-xs"></span>
              <Save v-else class="h-4 w-4" />
              <span>{{ t("common.save") }}</span>
            </button>
          </div>
      </template>

    <!-- 主体区域切换：一级卡片列表 ↔ 二级详情页 -->
    <Transition name="ecall-config-content" mode="out-in">
      <!-- 二级菜单：渠道详情与联系人视图 -->
      <div v-if="inDetailMode && selectedChannel" :key="'detail-body-' + selectedChannel.id" class="h-full min-h-0 flex flex-col gap-3 overflow-hidden flex-1">
        <OverlayScrollArea class="flex-1 min-h-0 h-full overflow-hidden" scroller-class="h-full space-y-3 p-4">
        <!-- 区块一：渠道配置折叠卡（改过一次后常驻折叠，不占核心视野） -->
        <details class="collapse collapse-arrow border border-base-300 bg-base-100 rounded-box shrink-0">
          <summary class="collapse-title min-h-0 py-2.5 px-4 text-xs font-semibold flex items-center justify-between cursor-pointer select-none">
            <div class="flex items-center gap-2">
              <Settings class="h-4 w-4 opacity-70" />
              <span>{{ t("config.remoteIm.channelSettings") }}</span>
              <span class="text-caption opacity-50 font-normal">({{ selectedChannel.name || platformLabelText(selectedChannel.platform) }})</span>
              <span v-if="channelDirty" class="badge badge-sm badge-warning">{{ t("config.skill.modified") }}</span>
            </div>
          </summary>
          <div class="collapse-content pt-3 pb-4 px-4 space-y-4 border-t border-base-200">
            <!-- 渠道名称与平台类型 -->
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div class="flex flex-col gap-1.5">
                <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.remoteIm.channelName") }}</label>
                <input v-model="selectedChannel.name" class="input input-bordered input-sm h-9 w-full text-xs" :placeholder="t('config.remoteIm.channelName')" />
              </div>
              <div class="flex flex-col gap-1.5">
                <label class="text-caption font-semibold opacity-60 uppercase">{{ t("config.remoteIm.platform") }}</label>
                <select v-model="selectedChannel.platform" class="select select-bordered select-sm h-9 w-full text-xs">
                  <option value="onebot_v11">{{ t("config.remoteIm.platformOptions.onebotV11") }}</option>
                  <option value="feishu">{{ t("config.remoteIm.platformOptions.feishu") }}</option>
                  <option value="dingtalk">{{ t("config.remoteIm.platformOptions.dingtalk") }}</option>
                  <option value="weixin_oc">{{ t('config.remoteIm.platformOptions.weixinOc') }}</option>
                </select>
              </div>
            </div>

            <!-- OneBot 凭证配置 -->
            <template v-if="selectedChannel.platform === 'onebot_v11'">
              <div class="rounded-field border border-base-300 bg-base-200/20 p-3 space-y-3">
                <div class="text-xs font-bold">{{ t("config.remoteIm.napcatConfig") }}</div>
                <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5">
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.wsHost") }}</label>
                    <input v-model="napcatCredentials.wsHost" class="input input-bordered input-sm h-9 w-full text-xs" placeholder="0.0.0.0" />
                  </div>
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.wsPort") }}</label>
                    <input v-model.number="napcatCredentials.wsPort" type="number" class="input input-bordered input-sm h-9 w-full text-xs" placeholder="6199" />
                  </div>
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.wsToken") }}</label>
                    <input v-model="napcatCredentials.wsToken" class="input input-bordered input-sm h-9 w-full text-xs" :placeholder="t('config.remoteIm.wsTokenPlaceholder')" />
                  </div>
                </div>
              </div>
            </template>

            <!-- 钉钉凭证 -->
            <template v-else-if="selectedChannel.platform === 'dingtalk'">
              <div class="rounded-field border border-base-300 bg-base-200/20 p-3 space-y-3">
                <div class="text-xs font-bold">{{ t("config.remoteIm.dingtalkCredentials") }}</div>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.dingtalkClientId") }}</label>
                    <input v-model="dingtalkCredentials.clientId" class="input input-bordered input-sm h-9 w-full text-xs" placeholder="dingxxxxxxxxxxxx" />
                  </div>
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.dingtalkClientSecret") }}</label>
                    <div class="flex items-center gap-1.5">
                      <input
                        v-model="dingtalkCredentials.clientSecret"
                        :type="showDingtalkSecret ? 'text' : 'password'"
                        class="input input-bordered input-sm h-9 w-full text-xs"
                        placeholder="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                      />
                      <button
                        class="btn btn-sm min-h-[2.25rem] h-9 px-2.5 btn-ghost shrink-0 text-xs"
                        type="button"
                        @click="showDingtalkSecret = !showDingtalkSecret"
                      >
                        {{ showDingtalkSecret ? t('config.remoteIm.hide') : t('config.remoteIm.show') }}
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </template>

            <!-- 微信扫码登录 -->
            <template v-else-if="selectedChannel.platform === 'weixin_oc'">
              <div class="rounded-field border border-base-300 bg-base-200/20 p-3 space-y-3">
                <div class="text-xs font-bold">{{ t('config.remoteIm.weixinScanLogin') }}</div>
                <div class="flex flex-wrap items-center justify-between gap-3">
                  <div class="flex flex-col gap-1 min-w-0">
                    <span class="text-xs font-medium">{{ weixinStatusText }}</span>
                    <span v-if="weixinStatusMessage" class="text-caption opacity-60 break-all">{{ weixinStatusMessage }}</span>
                  </div>
                  <button class="btn btn-sm min-h-[2.25rem] btn-primary" :disabled="weixinLoginBusy" @click="onWeixinLoginButtonClick">
                    {{ weixinLoginBusy ? t('config.remoteIm.processing') : (isWeixinLoggedIn ? t('config.remoteIm.logoutAndRescan') : t('config.remoteIm.scanLogin')) }}
                  </button>
                </div>
                <div v-if="!isWeixinLoggedIn && weixinLoginState.qrcodeImgContent" class="flex flex-col items-center gap-2 pt-2">
                  <img :src="weixinQrImageSrc" alt="weixin login qr" class="w-48 h-48 rounded-box border border-base-300 object-contain bg-white p-2" />
                  <span class="text-caption opacity-60">{{ t('config.remoteIm.scanQrCode') }}</span>
                </div>
              </div>
            </template>

            <!-- 飞书凭证 JSON -->
            <template v-else>
              <div class="rounded-field border border-base-300 bg-base-200/20 p-3 space-y-2">
                <div class="text-xs font-bold">{{ t("config.remoteIm.credentialsJson") }}</div>
                <textarea
                  v-model="credentialDrafts[selectedChannel.id]"
                  class="textarea textarea-bordered w-full min-h-20 font-mono text-xs"
                  spellcheck="false"
                  @blur="syncCredentialJson(selectedChannel)"
                />
              </div>
            </template>

            <!-- 折叠卡底部：左侧删除渠道，右侧保存渠道 -->
            <div class="flex items-center justify-between pt-3 border-t border-base-200">
              <button
                class="btn btn-sm btn-ghost text-error gap-1.5 px-3 hover:bg-error/10"
                type="button"
                :disabled="saving || isChannelOperationBusy(selectedChannel.id)"
                @click="deleteSelectedChannel"
              >
                <Trash2 class="h-3.5 w-3.5" />
                <span>{{ t("config.remoteIm.deleteChannel") || t("common.delete") }}</span>
              </button>

              <button
                v-if="channelDirty"
                class="btn btn-sm btn-primary gap-1.5 px-4"
                type="button"
                :disabled="saving || isChannelOperationBusy(selectedChannel.id)"
                @click="saveChannels"
              >
                <span v-if="saving" class="loading loading-spinner loading-xs"></span>
                <Save v-else class="h-3.5 w-3.5" />
                <span>{{ t("common.save") }}</span>
              </button>
            </div>
          </div>
        </details>

        <!-- 区块二：联系人列表（按自定义分组分段罗列，卡片样式） -->
        <div class="space-y-3 pt-2">
          <div class="flex flex-col gap-3 px-1 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between">
            <div class="flex shrink-0 items-center gap-2">
              <Users class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.remoteIm.contactsTitle") }}</span>
              <span class="badge badge-sm badge-neutral">{{ currentChannelContacts.length }}</span>
            </div>

            <div class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
              <button
                type="button"
                class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
                :title="t('config.remoteIm.contactGroupCreateTitle')"
                @click="startContactGroupCreate"
              >
                <Plus class="h-4 w-4" />
                <span>{{ t("config.remoteIm.contactGroupCreateTitle") }}</span>
              </button>

              <!-- 联系人过滤搜索框 -->
              <div class="relative w-full min-w-0 sm:w-60 sm:min-w-60 sm:flex-none">
                <input
                  v-model="contactSearchQuery"
                  type="text"
                  class="input input-bordered input-sm h-9 w-full pl-8 pr-8 text-xs"
                  :placeholder="t('config.remoteIm.contactsSearchPlaceholder')"
                />
                <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
                <button
                  v-if="contactSearchQuery"
                  type="button"
                  class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1 h-7 w-7 min-h-[1.75rem] opacity-60 hover:opacity-100"
                  @click="contactSearchQuery = ''"
                >
                  ✕
                </button>
              </div>
            </div>
          </div>

          <!-- 分组创建编辑行（重命名改为在分组名处就地编辑） -->
          <div v-if="contactGroupEditMode === 'create'" class="flex items-center gap-1 px-1">
            <input
              v-model="contactGroupNameDraft"
              type="text"
              class="input input-bordered input-sm h-8 min-w-0 flex-1 text-xs"
              :placeholder="t('config.remoteIm.contactGroupNamePlaceholder')"
              :disabled="contactGroupBusy"
              @keydown.enter.prevent="submitContactGroupEdit"
              @keydown.esc.prevent="cancelContactGroupEdit"
            />
            <button
              type="button"
              class="btn btn-primary btn-sm h-8 min-h-0 px-3"
              :disabled="contactGroupBusy"
              @click="submitContactGroupEdit"
            >
              <span v-if="contactGroupBusy" class="loading loading-spinner loading-xs"></span>
              <Check v-else class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-sm h-8 min-h-0 px-3"
              :disabled="contactGroupBusy"
              @click="cancelContactGroupEdit"
            >
              <X class="h-4 w-4" />
            </button>
          </div>
          <p
            v-if="contactGroupEditMode !== 'rename' && contactGroupError"
            class="px-1 text-caption text-error break-all whitespace-pre-wrap"
            style="overflow-wrap:anywhere"
          >
            {{ contactGroupError }}
          </p>

          <div v-if="contactsError" class="rounded-box px-4 py-3 text-xs text-error bg-error/10 border border-error/20 break-all whitespace-pre-wrap" style="overflow-wrap:anywhere">
            {{ contactsError }}
          </div>
          <div v-else-if="currentChannelContacts.length === 0" class="rounded-box border border-dashed border-base-300 py-12 text-center text-xs opacity-60 italic">
            {{ t("config.remoteIm.contactsEmpty") }}
          </div>
          <div v-else-if="contactListSections.length === 0" class="rounded-box border border-dashed border-base-300 py-12 text-center text-xs opacity-60 italic">
            {{ t("config.remoteIm.contactsNoMatch") }}
          </div>
          <div v-else class="space-y-4">
            <div v-for="section in contactListSections" :key="section.key" class="space-y-2">
              <!-- 分组头：折叠按钮 + 分组名 + 组内批量设置（重命名/删除走右键菜单） -->
              <div
                class="flex items-center justify-between gap-2 px-1"
                @contextmenu.prevent="openContactGroupMenu($event, section.group)"
              >
                <div class="flex min-w-0 items-center gap-1.5">
                  <button
                    type="button"
                    class="btn btn-ghost btn-xs btn-circle h-5 w-5 min-h-0 p-0"
                    :title="isContactSectionCollapsed(section) ? t('config.remoteIm.contactSectionExpand') : t('config.remoteIm.contactSectionCollapse')"
                    @click="toggleContactSectionCollapse(section)"
                  >
                    <ChevronDown v-if="!isContactSectionCollapsed(section)" class="h-3.5 w-3.5 opacity-60" />
                    <ChevronRight v-else class="h-3.5 w-3.5 opacity-60" />
                  </button>
                  <template v-if="isContactGroupRenaming(section)">
                    <input
                      :ref="focusContactGroupRenameInput"
                      v-model="contactGroupNameDraft"
                      type="text"
                      class="input input-bordered input-xs h-7 min-w-0 max-w-56 flex-1 text-xs"
                      :placeholder="t('config.remoteIm.contactGroupNamePlaceholder')"
                      :disabled="contactGroupBusy"
                      @contextmenu.stop.prevent
                      @click.stop
                      @keydown.enter.prevent="submitContactGroupEdit"
                      @keydown.esc.prevent="cancelContactGroupEdit"
                    />
                    <button
                      type="button"
                      class="btn btn-primary btn-xs h-7 min-h-0 px-2"
                      :disabled="contactGroupBusy"
                      @click.stop="submitContactGroupEdit"
                    >
                      <span v-if="contactGroupBusy" class="loading loading-spinner loading-xs"></span>
                      <Check v-else class="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs h-7 min-h-0 px-2"
                      :disabled="contactGroupBusy"
                      @click.stop="cancelContactGroupEdit"
                    >
                      <X class="h-3.5 w-3.5" />
                    </button>
                  </template>
                  <span v-else class="truncate text-xs font-bold text-base-content/70">{{ section.name }}</span>
                </div>
                <div class="flex shrink-0 items-center gap-1">
                  <button
                    type="button"
                    class="btn btn-ghost btn-xs gap-1 px-2"
                    :title="t('config.remoteIm.batchSettingsTitle')"
                    @click="openBatchSettingsWizard(section.key)"
                  >
                    <ListChecks class="h-3.5 w-3.5" />
                    <span>{{ t("config.remoteIm.batchSettings") }}</span>
                  </button>
                </div>
              </div>

              <p
                v-if="isContactGroupRenaming(section) && contactGroupError"
                class="px-1 text-caption text-error break-all whitespace-pre-wrap"
                style="overflow-wrap:anywhere"
              >
                {{ contactGroupError }}
              </p>

              <div v-show="!isContactSectionCollapsed(section)" class="space-y-2">
                <div
                  v-for="item in section.items"
                  :key="item.id"
                  class="rounded-box border border-base-300 bg-base-100 overflow-hidden"
                >
                  <div class="p-3.5 flex items-start gap-3.5">
                  <div class="avatar placeholder shrink-0">
                    <div class="flex h-10 w-10 items-center justify-center overflow-hidden rounded-full border border-base-300 bg-base-200 text-xs font-semibold leading-none text-base-content/70">
                      <img v-if="contactAvatarUrl(item)" :src="contactAvatarUrl(item)" :alt="contactSafeDisplayName(item)" class="block h-full w-full object-cover" />
                      <Users v-else-if="item.remoteContactType === 'group'" class="h-4 w-4 text-secondary" />
                      <User v-else class="h-4 w-4 text-primary" />
                    </div>
                  </div>

                  <div class="flex-1 min-w-0">
                    <div class="flex items-center justify-between gap-2">
                      <div class="min-w-0 flex-1 truncate font-semibold text-sm">
                        <span v-if="item.remoteContactType !== 'group'" class="font-normal opacity-60 text-xs">[{{ contactAgentLabel(item) }}]{{ " " }}</span>
                        {{ contactSafeDisplayName(item) }}
                        <span class="text-xs font-normal opacity-50">（{{ contactSecondaryText(item) }}）</span>
                      </div>

                      <!-- 权限与通信操作栏 -->
                      <div class="flex shrink-0 items-center gap-1.5">
                        <input
                          type="checkbox"
                          class="toggle toggle-sm"
                          :class="contactCommunicationToggleClass(item)"
                          :checked="contactCommunicationToggleEnabled(item)"
                          :title="`${t('config.remoteIm.allowReceive')} / ${t('config.remoteIm.allowSend')}`"
                          @click.stop
                          @change="toggleContactCommunication(item, ($event.target as HTMLInputElement).checked)"
                        />
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('config.remoteIm.viewLogs')"
                          @click.stop="openContactLogsModal(item.id)"
                        >
                          <ScrollText class="h-4 w-4" />
                        </button>
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('config.remoteIm.contactSettingsTitle', { name: contactSafeDisplayName(item) })"
                          @click.stop="toggleContactSettings(item.id)"
                        >
                          <ChevronDown v-if="isContactExpanded(item.id)" class="h-4 w-4" />
                          <ChevronRight v-else class="h-4 w-4" />
                        </button>
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('common.copy')"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="copyContactSettings(item)"
                        >
                          <Copy class="h-4 w-4" />
                        </button>
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('common.paste')"
                          :disabled="isContactOperationBusy(item.id) || !contactSettingsClipboard"
                          @click.stop="pasteContactSettings(item)"
                        >
                          <ClipboardPaste class="h-4 w-4" />
                        </button>
                      </div>
                    </div>

                    <!-- 标签徽章栏 -->
                    <div class="mt-2 flex flex-wrap gap-1.5 overflow-visible whitespace-nowrap text-xs">
                      <span class="badge badge-sm shrink-0" :class="item.remoteContactType === 'group' ? 'badge-secondary' : 'badge-primary'">
                        {{ item.remoteContactType === "group" ? t("config.remoteIm.group") : t("config.remoteIm.private") }}
                      </span>
                      <div>
                        <button
                          type="button"
                          class="badge badge-sm shrink-0 gap-1.5 transition-colors"
                          :class="contactActivationBadgeClass(item)"
                          :title="contactActivationHintText(item)"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'activation')"
                        >
                          {{ contactActivationModeLabel(item) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                      <span
                        v-if="contactKeywordModeMissingKeywords(item)"
                        class="badge badge-sm badge-warning shrink-0 gap-1.5"
                        :title="t('config.remoteIm.keywordMissingHint')"
                      >
                        <AlertTriangle class="h-3.5 w-3.5" />
                        {{ t('config.remoteIm.keywordEmpty') }}
                      </span>
                      <div>
                        <button
                          type="button"
                          class="badge badge-sm shrink-0 gap-1.5 transition-colors"
                          :class="contactProcessingModeBadgeClass(item)"
                          :title="processingModeHintText(item)"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'processing')"
                        >
                          {{ contactProcessingModeLabel(item) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                      <div v-if="!isPrivateContact(item)">
                        <button
                          type="button"
                          class="badge badge-sm shrink-0 gap-1.5"
                          :class="contactResponseStrategy(item) === 'smart_judge' ? 'badge-accent' : 'badge-ghost'"
                          :title="contactResponseStrategyHintText(item)"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'response')"
                        >
                          {{ contactResponseStrategyLabel(item) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                      <div>
                        <button
                          type="button"
                          class="badge badge-sm shrink-0 gap-1.5"
                          :class="item.allowSendFiles ? 'badge-warning' : 'badge-ghost'"
                          :title="t('config.remoteIm.allowSendFiles')"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'files')"
                        >
                          {{ contactSendFilesLabel(item) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                      <div>
                        <button
                          type="button"
                          class="badge badge-sm shrink-0 gap-1.5"
                          :class="contactGroupIdOf(item) ? 'badge-info' : 'badge-ghost'"
                          :title="t('config.remoteIm.moveToGroup')"
                          :disabled="isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'group')"
                        >
                          {{ contactGroupLabelById(contactGroupIdOf(item)) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                    </div>
                  </div>
                  </div>

                  <!-- 联系人复杂设置：展开后列在卡片下方，简单设置保留在卡片上 -->
                  <div
                    v-if="isContactExpanded(item.id)"
                    class="border-t border-base-200 bg-base-200/20 px-3.5 py-3 space-y-3"
                  >
                    <template v-if="selectedContact && contactDraft">
                      <ul class="list gap-2.5">
                        <!-- 处理人格 -->
                        <li class="list-row flex items-start justify-between gap-3 p-2 rounded-lg">
                          <div class="font-medium text-xs pt-1">{{ t("config.remoteIm.processingAgent") }}</div>
                          <div class="w-56 max-w-full shrink-0">
                            <AgentPersonaSelect
                              v-model:agent-id="contactDraft.boundAgentId"
                              :personas="personas"
                              :persona-avatar-url-map="personaAvatarUrlMap"
                              :api-configs="config.apiConfigs"
                              :expert-api-config-id="config.expertApiConfigId"
                              :tool-review-api-config-id="config.toolReviewApiConfigId"
                              :placeholder="t('config.remoteIm.processingAgentPlaceholder')"
                              :show-model="false"
                              size="sm"
                            />
                          </div>
                        </li>

                        <!-- 首选模型（挂在联系人的会话上） -->
                        <li class="list-row flex items-start justify-between gap-3 p-2 rounded-lg">
                          <div class="font-medium text-xs pt-1">{{ t("config.remoteIm.preferredModel") }}</div>
                          <div class="w-56 max-w-full shrink-0">
                            <ApiConfigPicker
                              size="sm"
                              :model-value="contactConversationModel.preferredApiConfigId"
                              :api-configs="config.apiConfigs"
                              :placeholder="t('config.remoteIm.preferredModelUnset')"
                              @update:model-value="onContactConversationModelChange"
                            />
                          </div>
                        </li>

                        <!-- 点名关键词：入场时机由卡片上的入场徽章切换，此处只填关键词 -->
                        <li
                          v-if="!isPrivateContact(selectedContact) && normalizeActivationMode(selectedContact.activationMode || 'never') === 'keyword'"
                          class="list-row flex items-center justify-between gap-3 p-2 rounded-lg"
                        >
                          <div class="font-medium text-xs">{{ t("config.remoteIm.activateKeywords") }}</div>
                          <div class="w-56 max-w-full shrink-0">
                            <input
                              type="text"
                              class="input input-bordered input-sm w-full text-xs"
                              :placeholder="t('config.remoteIm.activateKeywordsPlaceholder')"
                              v-model="contactDraft.activationKeywordsText"
                            />
                          </div>
                        </li>
                      </ul>

                      <!-- 工作目录配置 -->
                      <div class="pt-3 border-t border-base-200 space-y-2.5">
                        <div class="flex items-center justify-between px-1">
                          <div class="font-medium text-xs">{{ t("config.remoteIm.workspace") }}</div>
                          <span class="text-caption opacity-50">{{ t("config.remoteIm.systemWorkspaceReadonly") }}</span>
                        </div>
                        <div class="rounded-xl border border-base-content/10 bg-base-200/20 p-3">
                          <WorkspaceConfigCard
                            :main-path="contactMainPath"
                            :secondary-paths="contactSecondaryPaths"
                            :access="contactUnifiedAccess"
                            :available-workspaces="contactAvailableWorkspaces"
                            @update:main-path="onContactMainPathUpdate"
                            @update:access="onContactAccessUpdate"
                            @add-secondary="onContactAddSecondary"
                            @remove-secondary="onContactRemoveSecondary"
                          />
                        </div>
                      </div>

                      <!-- 操作栏：左侧删除与收信开关，右侧还原与保存 -->
                      <div class="flex flex-wrap items-center justify-between gap-3 border-t border-base-300 pt-3">
                        <div class="flex items-center gap-2 min-w-0">
                          <button
                            class="btn btn-sm btn-ghost gap-1.5 text-error"
                            :disabled="contactSaving || contactDeleting"
                            @click="deleteContact(selectedContact)"
                          >
                            <Trash2 class="h-3.5 w-3.5" />
                            <span>{{ t("common.delete") }}</span>
                          </button>
                          <span v-if="contactDraftDirty" class="text-warning text-xs font-medium shrink-0 ml-1">● {{ t("config.skill.modified") }}</span>
                        </div>
                        <div class="flex items-center gap-2 shrink-0">
                          <button
                            class="btn btn-sm btn-ghost gap-1.5"
                            :disabled="!contactDraftDirty || contactSaving"
                            @click="resetContactDraft"
                          >
                            <RotateCcw class="h-3.5 w-3.5" />
                            <span>{{ t("common.reset") }}</span>
                          </button>
                          <button
                            class="btn btn-sm btn-primary gap-1.5 px-4"
                            :disabled="!contactDraftDirty || contactSaving || contactDeleting"
                            @click="saveContactDraft"
                          >
                            <span v-if="contactSaving" class="loading loading-spinner loading-xs"></span>
                            <Save v-else class="h-3.5 w-3.5" />
                            <span>{{ t("common.save") }}</span>
                          </button>
                        </div>
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        </OverlayScrollArea>

      </div>

      <!-- 一级概览：渠道 2 列卡片矩阵 -->
      <div v-else key="overview-grid" class="grid gap-4 pb-8">
        <div class="config-grid-auto-sm">
          <div
            v-for="ch in filteredChannels"
            :key="ch.id"
            role="button"
            tabindex="0"
            class="rounded-box border border-base-300 bg-base-100 p-4 hover:border-primary/50 transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] group"
            @click="enterChannel(ch.id)"
            @keydown.enter.prevent="enterChannel(ch.id)"
            @keydown.space.prevent="enterChannel(ch.id)"
          >
            <!-- 头部：平台图标 + 渠道名称 + 平台标识 + 启停开关 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="flex items-center gap-2.5 min-w-0 flex-1">
                <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-field border font-bold text-xs" :class="getPlatformIconColor(ch.platform)">
                  {{ platformBadgeText(ch.platform) }}
                </div>
                <div class="min-w-0 flex-1">
                  <div class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ ch.name || platformLabelText(ch.platform) }}
                  </div>
                  <div class="text-caption opacity-50 truncate mt-0.5">
                    {{ platformLabelText(ch.platform) }}
                  </div>
                </div>
              </div>

              <!-- 启用开关 -->
              <input
                type="checkbox"
                class="toggle toggle-primary toggle-sm shrink-0"
                :checked="ch.enabled"
                :disabled="saving || isChannelOperationBusy(ch.id)"
                @click.stop
                @change.stop="(e) => toggleChannelEnabled(ch, (e.target as HTMLInputElement).checked)"
              />
            </div>

            <!-- 底栏：在线状态 + 联系人计数 + 进入箭头 -->
            <div class="flex items-center justify-between border-t border-base-300 pt-2.5 text-caption">
              <div class="flex items-center gap-1.5">
                <span class="size-2 rounded-full shrink-0" :class="getChannelStatusInfo(ch).dot"></span>
                <span class="opacity-70">{{ getChannelStatusInfo(ch).text }}</span>
              </div>
              <div class="flex items-center gap-2">
                <span class="font-mono opacity-60">{{ t("config.remoteIm.contactsCount", { count: getChannelContactCount(ch.id).toLocaleString() }) }}</span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>

          <!-- 新增渠道卡：网格末位入口 -->
          <button
            type="button"
            class="flex min-h-[7.5rem] flex-col items-center justify-center gap-1.5 rounded-box border border-dashed border-base-300 bg-base-100 p-3.5 text-base-content/50 transition-all hover:border-primary/50 hover:text-primary sm:p-4"
            @click="openAddChannelModal"
          >
            <Plus class="h-5 w-5" />
            <span class="text-sm font-medium">{{ t("config.remoteIm.addChannel") }}</span>
          </button>
        </div>
      </div>
    </Transition>

    <Teleport to="body">
      <div
        v-if="contactPillMenu"
        class="fixed inset-0 z-9999"
        @click="closeContactPillMenu"
        @wheel.passive="closeContactPillMenu"
      >
        <ul
          class="menu menu-sm fixed rounded-box border border-base-300 bg-base-100 p-1 text-sm shadow-xl"
          :class="contactPillMenu.widthClass"
          :style="{ left: `${contactPillMenu.left}px`, top: `${contactPillMenu.top}px` }"
          @click.stop
        >
          <li v-for="option in contactPillMenu.options" :key="option.key">
            <button
              type="button"
              class="leading-5"
              :class="{ active: option.active }"
              @click="selectContactPillMenuOption(option)"
            >
              {{ option.label }}
            </button>
          </li>
        </ul>
      </div>
    </Teleport>

    <!-- 分组右键菜单：重命名 / 删除 -->
    <Teleport to="body">
      <div
        v-if="contactGroupMenu"
        class="fixed inset-0 z-9999"
        @click="closeContactGroupMenu"
        @wheel.passive="closeContactGroupMenu"
        @contextmenu.prevent="closeContactGroupMenu"
      >
        <ul
          class="menu menu-sm fixed w-36 rounded-box border border-base-300 bg-base-100 p-1 text-sm shadow-xl"
          :style="{ left: `${contactGroupMenu.left}px`, top: `${contactGroupMenu.top}px` }"
          @click.stop
        >
          <li>
            <button type="button" class="leading-5" @click="renameContactGroupFromMenu">
              {{ t("config.remoteIm.contactGroupRenameTitle") }}
            </button>
          </li>
          <li>
            <button type="button" class="leading-5 text-error" @click="deleteContactGroupFromMenu">
              {{ t("config.remoteIm.contactGroupDeleteTitle") }}
            </button>
          </li>
        </ul>
      </div>
    </Teleport>

    <dialog ref="addChannelDialogRef" class="modal" @close="closeAddChannelModal" @cancel.prevent="closeAddChannelModal">
      <div class="modal-box max-w-md">
        <div class="flex items-center justify-between">
          <div class="font-semibold text-lg">{{ t("config.remoteIm.addChannel") }}</div>
          <button class="btn btn-sm btn-circle btn-ghost" @click="closeAddChannelModal">
            <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="mt-3 text-sm opacity-70">{{ t("config.remoteIm.choosePlatform") }}</div>
        <div class="mt-4 grid grid-cols-1 gap-2">
          <button
            v-for="option in channelPlatformOptions"
            :key="option.platform"
            class="btn btn-outline justify-start h-auto min-h-0 py-3 px-4 normal-case"
            @click="addChannel(option.platform)"
          >
            <span class="font-medium">{{ option.label }}</span>
          </button>
        </div>
        <div class="modal-action">
          <button class="btn btn-ghost" @click="closeAddChannelModal">{{ t("common.cancel") }}</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeAddChannelModal">close</button>
      </form>
    </dialog>

    <dialog ref="channelLogsDialogRef" class="modal" @close="closeChannelLogsModal" @cancel.prevent="closeChannelLogsModal">
      <div class="modal-box max-w-4xl">
        <div class="flex items-center justify-between">
          <div class="font-semibold">
            {{ t("config.remoteIm.channelLogs") }} · {{ selectedChannel?.name || "-" }}
          </div>
          <div class="flex items-center gap-2">
            <button class="btn btn-sm btn-ghost" :title="t('common.refresh')" @click="refreshChannelLogs">
              <RefreshCw class="h-4 w-4" :class="channelLogsLoading ? 'animate-spin' : ''" />
            </button>
            <button class="btn btn-sm" @click="closeChannelLogsModal">{{ t("common.close") }}</button>
          </div>
        </div>
        <div v-if="channelLogsModalOpen" class="mt-3 max-h-[60vh] overflow-y-auto">
          <div v-if="channelLogs.length === 0" class="opacity-60 italic text-xs">{{ t("config.remoteIm.noLogs") }}</div>
          <pre v-else class="bg-base-200 rounded-box p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all m-0"><template v-for="(log, idx) in channelLogs" :key="idx"><span :class="log.level === 'error' ? 'text-error' : log.level === 'warn' ? 'text-warning' : ''"><span class="opacity-50">{{ formatLogTime(log.timestamp) }}</span> {{ log.message }}</span>{{ '\n' }}</template></pre>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeChannelLogsModal">close</button>
      </form>
    </dialog>

    <dialog ref="contactLogsDialogRef" class="modal" @close="closeContactLogsModal" @cancel.prevent="closeContactLogsModal">
      <div class="modal-box max-w-4xl">
        <div class="flex items-center justify-between">
          <div class="font-semibold">
            {{ t('config.remoteIm.contactLogs') }} · {{ contactLogsTitle }}
          </div>
          <div class="flex items-center gap-2">
            <button class="btn btn-sm btn-ghost" :title="t('common.refresh')" @click="refreshContactLogs">
              <RefreshCw class="h-4 w-4" :class="contactLogsLoading ? 'animate-spin' : ''" />
            </button>
            <button class="btn btn-sm" @click="closeContactLogsModal">{{ t("common.close") }}</button>
          </div>
        </div>
        <div v-if="contactLogsModalOpen" class="mt-3 max-h-[60vh] overflow-y-auto">
          <div v-if="contactLogs.length === 0" class="opacity-60 italic text-xs">{{ t('config.remoteIm.noContactLogs') }}</div>
          <pre v-else class="bg-base-200 rounded-box p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all m-0"><template v-for="(line, idx) in contactLogDisplayLines" :key="`${idx}-${line}`"><span>{{ line }}</span>{{ '\n' }}</template></pre>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeContactLogsModal">close</button>
      </form>
    </dialog>

    <!-- 渠道配置模态框 -->
    <dialog ref="channelConfigDialogRef" class="modal" @close="closeChannelConfigModal" @cancel.prevent="closeChannelConfigModal">
      <div class="modal-box max-w-3xl max-h-[80vh] overflow-hidden flex flex-col">
        <div class="flex items-center justify-between shrink-0">
          <div class="font-semibold text-lg">
            {{ t("config.remoteIm.channelDetails") }} · {{ selectedChannel?.name || "-" }}
          </div>
          <button class="btn btn-sm btn-circle btn-ghost" @click="closeChannelConfigModal">
            <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div v-if="selectedChannel" class="flex-1 min-h-0 overflow-hidden flex flex-col mt-4">
          <!-- 头部 -->
          <div class="flex items-center justify-between px-3 py-2 shrink-0">
            <div></div>
          </div>

          <!-- 状态栏 -->
          <div class="px-3 pb-2 shrink-0">
            <div class="rounded-box border border-base-300 bg-base-200/60 px-3 py-2 flex items-center justify-between gap-3">
              <div class="flex items-center gap-2 min-w-0">
                <span
                  class="size-2 rounded-full shrink-0"
                  :class="selectedChannel.platform === 'onebot_v11'
                    ? (channelRuntimeStates[selectedChannel.id]?.connected ? 'bg-success' : (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300'))
                    : (selectedChannel.platform === 'dingtalk'
                      ? (channelRuntimeStates[selectedChannel.id]?.connected ? 'bg-success' : (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300'))
                      : ((selectedChannel.platform === 'feishu')
                        ? (selectedChannel.enabled ? 'bg-warning' : 'bg-base-300')
                        : (selectedChannel.enabled ? 'bg-success' : 'bg-base-300')))"
                ></span>
                <span class="text-xs font-medium">{{ t("config.remoteIm.connectionStatus") }}</span>
                <span class="text-xs opacity-80 truncate">{{ channelStatusPreview(selectedChannel!) }}</span>
              </div>
              <div class="flex items-center gap-2">
                <button class="btn btn-xs btn-ghost" @click="openChannelLogsModal">
                  {{ t("config.remoteIm.viewLogs") }}
                </button>
              </div>
            </div>
          </div>

          <!-- 内容滚动区 -->
          <div class="flex-1 overflow-y-auto px-3 text-xs pb-4">
              <!-- 渠道名称 -->
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.channelName") }}</span>
                <input v-model="selectedChannel.name" class="input input-bordered input-sm w-48" :placeholder="t('config.remoteIm.channelName')" />
              </div>
              <!-- 平台 -->
              <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                <span>{{ t("config.remoteIm.platform") }}</span>
                <select v-model="selectedChannel.platform" class="select select-bordered select-sm w-48">
                  <option value="onebot_v11">{{ t("config.remoteIm.platformOptions.onebotV11") }}</option>
                  <option value="feishu">{{ t("config.remoteIm.platformOptions.feishu") }}</option>
                  <option value="dingtalk">{{ t("config.remoteIm.platformOptions.dingtalk") }}</option>
                  <option value="weixin_oc">{{ t('config.remoteIm.weixinPlatform') }}</option>
                </select>
              </div>
              <div class="border-b-base-content/5 flex items-start justify-between gap-3 border-b border-dashed py-2">
                <div class="flex flex-col gap-1">
                  <span>{{ t("config.remoteIm.filterMarkdown") }}</span>
                  <span class="max-w-80 text-xs opacity-60">{{ t("config.remoteIm.filterMarkdownHint") }}</span>
                </div>
                <input v-model="selectedChannel.filterMarkdown" type="checkbox" class="toggle toggle-primary toggle-sm mt-0.5" />
              </div>

              <!-- OneBot v11 凭证配置 -->
              <template v-if="selectedChannel.platform === 'onebot_v11'">
                <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                  <span class="font-semibold">{{ t("config.remoteIm.napcatConfig") }}</span>
                </div>
                <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                  <span>{{ t("config.remoteIm.wsHost") }}</span>
                  <input v-model="napcatCredentials.wsHost" class="input input-bordered input-sm w-32" placeholder="0.0.0.0" />
                </div>
                <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                  <span>{{ t("config.remoteIm.wsPort") }}</span>
                  <input v-model.number="napcatCredentials.wsPort" type="number" class="input input-bordered input-sm w-32" placeholder="6199" />
                </div>
                <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                  <span>{{ t("config.remoteIm.wsToken") }}</span>
                  <input v-model="napcatCredentials.wsToken" class="input input-bordered input-sm w-32" :placeholder="t('config.remoteIm.wsTokenPlaceholder')" />
                </div>
              </template>

              <!-- 钉钉凭证 -->
              <template v-else-if="selectedChannel.platform === 'dingtalk'">
                <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                  <span class="font-semibold">{{ t("config.remoteIm.dingtalkCredentials") }}</span>
                </div>
                <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                  <span>{{ t("config.remoteIm.dingtalkClientId") }}</span>
                  <input
                    v-model="dingtalkCredentials.clientId"
                    class="input input-bordered input-sm w-72"
                    placeholder="dingxxxxxxxxxxxxxxxx"
                  />
                </div>
                <div class="border-b-base-content/5 flex items-center justify-between gap-2 border-b border-dashed py-2">
                  <span>{{ t("config.remoteIm.dingtalkClientSecret") }}</span>
                  <div class="flex items-center gap-2">
                    <input
                      v-model="dingtalkCredentials.clientSecret"
                      :type="showDingtalkSecret ? 'text' : 'password'"
                      class="input input-bordered input-sm w-72"
                      placeholder="xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                    />
                    <button
                      class="btn btn-xs btn-ghost"
                      type="button"
                      @click="showDingtalkSecret = !showDingtalkSecret"
                    >
                      {{ showDingtalkSecret ? t('config.remoteIm.hide') : t('config.remoteIm.show') }}
                    </button>
                  </div>
                </div>
              </template>

              <template v-else-if="selectedChannel.platform === 'weixin_oc'">
                <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                  <span class="font-semibold">{{ t('config.remoteIm.weixinScanLogin') }}</span>
                </div>
                <div class="border-b-base-content/5 flex items-start justify-between gap-2 border-b border-dashed py-2">
                  <div class="flex flex-col gap-1">
                    <span>{{ t('config.remoteIm.loginStatus') }}</span>
                    <span class="opacity-70 break-all">{{ weixinStatusText }}</span>
                    <span v-if="weixinStatusMessage" class="opacity-60 break-all">{{ weixinStatusMessage }}</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <button class="btn btn-primary" :disabled="weixinLoginBusy" @click="onWeixinLoginButtonClick">
                      {{ weixinLoginBusy ? t('config.remoteIm.processing') : (isWeixinLoggedIn ? t('config.remoteIm.logoutAndRescan') : t('config.remoteIm.scanLogin')) }}
                    </button>
                  </div>
                </div>
                <div v-if="isWeixinLoggedIn" class="border-b-base-content/5 flex items-center gap-2 border-b border-dashed py-2 text-success">
                  <span class="font-semibold">{{ t('config.remoteIm.loggedInReady') }}</span>
                </div>
                <div v-else-if="weixinLoginState.qrcodeImgContent" class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2">
                  <span class="font-semibold">{{ t('config.remoteIm.scanQrCode') }}</span>
                  <img :src="weixinQrImageSrc" alt="weixin login qr" class="w-48 h-48 rounded-box border border-base-300 object-contain bg-white p-2" />
                </div>
              </template>

              <!-- 飞书凭证 JSON -->
              <template v-else>
                <div class="border-b-base-content/5 flex flex-col gap-2 border-b border-dashed py-2 mt-2">
                  <span class="font-semibold">{{ t("config.remoteIm.credentialsJson") }}</span>
                  <textarea
                    v-model="credentialDrafts[selectedChannel.id]"
                    class="textarea textarea-bordered w-full min-h-20 font-mono"
                    spellcheck="false"
                    @blur="syncCredentialJson(selectedChannel)"
                  />
                </div>
              </template>

            <!-- 连接状态区域 (仅 OneBot v11) -->
            <template v-if="selectedChannel.platform === 'onebot_v11'">
              <div class="border-t border-base-300 mt-2 pt-2">
                <div class="flex items-center justify-between">
                  <span class="font-semibold">{{ t("config.remoteIm.connectionStatus") }}</span>
                  <button
                    class="btn btn-square btn-ghost"
                    :title="t('common.refresh')"
                    :disabled="isChannelOperationBusy(selectedChannel.id)"
                    @click="refreshChannelStatus"
                  >
                    <RefreshCw class="h-3.5 w-3.5" />
                  </button>
                </div>
                <div class="mt-2 flex items-center gap-2">
                  <span class="size-2 rounded-full" :class="channelStatus?.connected ? 'bg-success' : 'bg-base-300'"></span>
                  <span class="text-xs">
                    {{ onebotStatusText(channelStatus) }}
                  </span>
                </div>
              </div>

            </template>
          </div>

          <!-- 底部操作区（固定在滚动区外） -->
          <div class="px-3 py-2 shrink-0 border-t border-base-300 flex items-center justify-between">
            <button
              class="btn btn-ghost"
              :title="t('common.delete')"
              :disabled="saving || isChannelOperationBusy(selectedChannel.id)"
              @click="deleteSelectedChannel"
            >
              <Trash2 class="h-3.5 w-3.5" />
              {{ t("common.delete") }}
            </button>
            <div class="flex items-center gap-2">
              <button
                v-if="selectedChannel.platform === 'onebot_v11'"
                class="btn btn-ghost"
                :title="t('common.reset')"
                @click="resetNapcatCredentials"
              >
                <RotateCcw class="h-3.5 w-3.5" />
                {{ t("common.reset") }}
              </button>
              <button
                class="btn"
                :class="channelDirty ? 'btn-primary' : 'btn-ghost'"
                :disabled="!channelDirty || saving || isChannelOperationBusy(selectedChannel.id)"
                @click="saveChannels"
              >
                <Save v-if="!saving && !isChannelOperationBusy(selectedChannel.id)" class="h-3.5 w-3.5" />
                <span v-else class="loading loading-spinner loading-xs"></span>
                {{ t("common.save") }}
              </button>
            </div>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeChannelConfigModal">close</button>
      </form>
    </dialog>

    <!-- 分组批量设置向导 -->
    <dialog
      ref="batchSettingsDialogRef"
      class="modal"
      @close="closeBatchSettingsWizard"
      @cancel.prevent="closeBatchSettingsWizard"
    >
      <div class="modal-box max-w-xl flex flex-col h-[80vh] max-h-[85vh] p-6 overflow-hidden">
        <!-- 顶部 Header（标题与关闭按钮同行展示） -->
        <div class="flex items-center justify-between shrink-0 mb-1">
          <h3 class="font-bold text-base">{{ t("config.remoteIm.batchSettingsTitle") }}</h3>
          <button
            class="btn btn-sm btn-circle btn-ghost -mr-2"
            type="button"
            @click="closeBatchSettingsWizard"
          >
            ✕
          </button>
        </div>

        <!-- 面包屑步骤导航（简洁纯粹，上下无多余分割线） -->
        <div v-if="batchSettingsWizard" class="breadcrumbs text-xs shrink-0 py-2 text-base-content/70">
          <ul>
            <li>
              <a
                v-if="batchSettingsWizard.step > 1 && !batchSettingsWizard.executing"
                class="cursor-pointer opacity-70 hover:opacity-100 hover:text-primary transition-colors"
                @click.prevent="batchSettingsWizard.step = 1"
              >
                {{ t("config.remoteIm.batchStepFields") }}
              </a>
              <span v-else :class="batchSettingsWizard.step === 1 ? 'font-bold text-primary' : 'opacity-40'">
                {{ t("config.remoteIm.batchStepFields") }}
              </span>
            </li>
            <li>
              <a
                v-if="batchSettingsWizard.step > 2 && !batchSettingsWizard.executing"
                class="cursor-pointer opacity-70 hover:opacity-100 hover:text-primary transition-colors"
                @click.prevent="batchSettingsWizard.step = 2"
              >
                {{ t("config.remoteIm.batchStepContacts") }}
              </a>
              <span v-else :class="batchSettingsWizard.step === 2 ? 'font-bold text-primary' : 'opacity-40'">
                {{ t("config.remoteIm.batchStepContacts") }}
              </span>
            </li>
            <li>
              <a
                v-if="batchSettingsWizard.step > 3 && !batchSettingsWizard.executing"
                class="cursor-pointer opacity-70 hover:opacity-100 hover:text-primary transition-colors"
                @click.prevent="batchSettingsWizard.step = 3"
              >
                {{ t("config.remoteIm.batchStepPreview") }}
              </a>
              <span v-else :class="batchSettingsWizard.step === 3 ? 'font-bold text-primary' : 'opacity-40'">
                {{ t("config.remoteIm.batchStepPreview") }}
              </span>
            </li>
            <li>
              <span :class="batchSettingsWizard.step === 4 ? 'font-bold text-primary' : 'opacity-40'">
                {{ t("config.remoteIm.batchStepResult") }}
              </span>
            </li>
          </ul>
        </div>

        <!-- 中间独立滚动区（严格锁定高度，内容溢出时在内部滚动） -->
        <OverlayScrollArea
          v-if="batchSettingsWizard"
          class="flex-1 min-h-0 h-full overflow-hidden"
          scroller-class="h-full pr-1"
        >
          <div
            v-if="batchSettingsWizard.error"
            class="alert alert-error mb-3 py-2 text-xs break-all"
          >
            {{ batchSettingsWizard.error }}
          </div>

          <!-- 第一步：选择要批量设置的项（所有项严格统一结构：左侧复选框，右侧同宽控件，未勾选禁用展示） -->
          <div v-if="batchSettingsWizard.step === 1" class="flex flex-col">
            <!-- 处理人格 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.agent"
                  @change="batchWizardToggleField('agent', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.processingAgent") }}</span>
              </label>
              <div class="w-56 max-w-full shrink-0">
                <AgentPersonaSelect
                  v-model:agent-id="batchSettingsWizard.agentId"
                  :personas="personas"
                  :persona-avatar-url-map="personaAvatarUrlMap"
                  :api-configs="config.apiConfigs"
                  :expert-api-config-id="config.expertApiConfigId"
                  :tool-review-api-config-id="config.toolReviewApiConfigId"
                  :placeholder="t('config.remoteIm.processingAgentPlaceholder')"
                  :show-model="false"
                  :disabled="!batchSettingsWizard.fields.agent"
                  size="sm"
                />
              </div>
            </div>

            <!-- 首选模型（独立项） -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.model"
                  @change="batchWizardToggleField('model', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.preferredModel") }}</span>
              </label>
              <ApiConfigPicker
                class="w-56 max-w-full shrink-0"
                size="sm"
                :model-value="batchSettingsWizard.preferredApiConfigId"
                :api-configs="config.apiConfigs"
                :placeholder="t('config.remoteIm.preferredModelUnset')"
                :disabled="!batchSettingsWizard.fields.model"
                @update:model-value="(val: string) => { if (batchSettingsWizard) batchSettingsWizard.preferredApiConfigId = val; }"
              />
            </div>

            <!-- 处理模式 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.processingMode"
                  @change="batchWizardToggleField('processingMode', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.processingMode") }}</span>
              </label>
              <select
                v-model="batchSettingsWizard.processingMode"
                class="select select-bordered select-sm text-xs w-56 shrink-0"
                :disabled="!batchSettingsWizard.fields.processingMode"
              >
                <option value="continuous">{{ t("config.remoteIm.processingModeContinuous") }}</option>
                <option value="qa">{{ t("config.remoteIm.processingModeQa") }}</option>
              </select>
            </div>

            <!-- 入场时机 -->
            <div class="border-b-base-content/10 flex flex-col border-b border-dashed py-3 last:border-b-0">
              <div class="flex items-center justify-between gap-3">
                <label class="flex cursor-pointer items-center gap-2.5 select-none">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-sm"
                    :checked="batchSettingsWizard.fields.activation"
                    @change="batchWizardToggleField('activation', ($event.target as HTMLInputElement).checked)"
                  />
                  <span class="text-xs font-medium">{{ t("config.remoteIm.activateMode") }}</span>
                </label>
                <select
                  v-model="batchSettingsWizard.activationMode"
                  class="select select-bordered select-sm text-xs w-56 shrink-0"
                  :disabled="!batchSettingsWizard.fields.activation"
                >
                  <option value="always">{{ t("config.remoteIm.activateModeAlways") }}</option>
                  <option value="keyword">{{ t("config.remoteIm.activateModeKeyword") }}</option>
                  <option value="never">{{ t("config.remoteIm.activateModeNever") }}</option>
                </select>
              </div>
              <div
                v-if="batchSettingsWizard.fields.activation && batchSettingsWizard.activationMode === 'keyword'"
                class="mt-2.5 pl-6.5"
              >
                <input
                  v-model="batchSettingsWizard.activationKeywordsText"
                  type="text"
                  class="input input-bordered input-sm text-xs w-full"
                  :placeholder="t('config.remoteIm.activateKeywordsPlaceholder')"
                />
              </div>
            </div>

            <!-- 应答策略 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.responseStrategy"
                  @change="batchWizardToggleField('responseStrategy', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.responseStrategy") }}</span>
              </label>
              <select
                v-model="batchSettingsWizard.responseStrategy"
                class="select select-bordered select-sm text-xs w-56 shrink-0"
                :disabled="!batchSettingsWizard.fields.responseStrategy"
              >
                <option value="always_reply">{{ t("config.remoteIm.responseStrategyAlways") }}</option>
                <option value="smart_judge">{{ t("config.remoteIm.responseStrategySmart") }}</option>
              </select>
            </div>

            <!-- 通信开关 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.communication"
                  @change="batchWizardToggleField('communication', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.batchCommunicationLabel") }}</span>
              </label>
              <div class="flex items-center gap-4 shrink-0">
                <label class="flex cursor-pointer items-center gap-2 select-none">
                  <span class="text-xs opacity-80">{{ t("config.remoteIm.batchCommunicationToggle") }}</span>
                  <input
                    v-model="batchSettingsWizard.allowCommunication"
                    type="checkbox"
                    class="toggle toggle-primary toggle-sm"
                    :disabled="!batchSettingsWizard.fields.communication"
                  />
                </label>
                <label class="flex cursor-pointer items-center gap-2 select-none">
                  <span class="text-xs opacity-80">{{ t("config.remoteIm.allowSendFiles") }}</span>
                  <input
                    v-model="batchSettingsWizard.allowSendFiles"
                    type="checkbox"
                    class="toggle toggle-primary toggle-sm"
                    :disabled="!batchSettingsWizard.fields.communication"
                  />
                </label>
              </div>
            </div>

            <!-- 工作区权限 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none min-w-0">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.workspaceAccess"
                  @change="batchWizardToggleField('workspaceAccess', ($event.target as HTMLInputElement).checked)"
                />
                <div class="min-w-0">
                  <div class="text-xs font-medium">{{ t("config.remoteIm.batchWorkspaceAccess") }}</div>
                  <div class="text-caption opacity-50">{{ t("config.remoteIm.batchWorkspaceAccessHint") }}</div>
                </div>
              </label>
              <select
                v-model="batchSettingsWizard.workspaceAccess"
                class="select select-bordered select-sm text-xs w-56 shrink-0"
                :disabled="!batchSettingsWizard.fields.workspaceAccess"
              >
                <option value="read_only">{{ t("config.tools.workspaceAccessReadOnly") }}</option>
                <option value="approval">{{ t("config.tools.workspaceAccessApproval") }}</option>
                <option value="full_access">{{ t("config.tools.workspaceAccessFullAccess") }}</option>
              </select>
            </div>

            <!-- 移动到分组 -->
            <div class="border-b-base-content/10 flex items-center justify-between gap-3 border-b border-dashed py-3 last:border-b-0">
              <label class="flex cursor-pointer items-center gap-2.5 select-none">
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :checked="batchSettingsWizard.fields.group"
                  @change="batchWizardToggleField('group', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-xs font-medium">{{ t("config.remoteIm.batchMoveToGroup") }}</span>
              </label>
              <select
                v-model="batchSettingsWizard.targetGroupId"
                class="select select-bordered select-sm text-xs w-56 shrink-0"
                :disabled="!batchSettingsWizard.fields.group"
              >
                <option value="">{{ t("config.remoteIm.contactGroupUngrouped") }}</option>
                <option v-for="group in currentChannelGroups" :key="group.id" :value="group.id">
                  {{ group.name }}
                </option>
              </select>
            </div>
          </div>

          <!-- 第二步：选择联系人（按分组罗列，支持折叠与三态勾选） -->
          <template v-else-if="batchSettingsWizard.step === 2">
            <div class="flex items-center justify-between gap-2 mb-3">
              <div class="text-xs font-medium">
                {{ t("config.remoteIm.batchSelectedCount", {
                  selected: batchSettingsWizard.selectedContactIds.length,
                  total: batchWizardScopedContacts.length
                }) }}
              </div>
              <div class="flex items-center gap-2">
                <div class="relative">
                  <input
                    v-model="batchSettingsWizard.contactSearchQuery"
                    type="text"
                    class="input input-bordered input-sm text-xs pl-8 w-44"
                    :placeholder="t('config.remoteIm.contactsSearchPlaceholder')"
                  />
                  <Search class="pointer-events-none absolute left-2.5 top-2.5 h-3.5 w-3.5 opacity-40" />
                </div>
                <button
                  type="button"
                  class="btn btn-ghost btn-xs text-xs"
                  @click="batchWizardToggleAllContacts(true)"
                >
                  {{ t("config.remoteIm.batchSelectAll") }}
                </button>
                <button
                  type="button"
                  class="btn btn-ghost btn-xs text-xs"
                  @click="batchWizardToggleAllContacts(false)"
                >
                  {{ t("config.remoteIm.batchClearAll") }}
                </button>
              </div>
            </div>

            <div
              v-if="batchWizardScopedContacts.length === 0"
              class="py-12 text-center text-xs opacity-50"
            >
              {{ t("config.remoteIm.contactGroupEmpty") }}
            </div>
            <div v-else class="space-y-3">
              <div
                v-for="section in batchWizardContactSections"
                :key="section.key"
                class="rounded-box border border-base-300 bg-base-100 overflow-hidden"
              >
                <!-- 分组头部：点击整行折叠/展开，右边带三态勾选 Checkbox -->
                <div
                  class="flex items-center justify-between px-3 py-2 bg-base-200/50 cursor-pointer select-none"
                  @click="batchWizardToggleGroupExpand(section.key)"
                >
                  <div class="flex items-center gap-2">
                    <ChevronDown
                      v-if="batchWizardIsGroupExpanded(section.key)"
                      class="h-4 w-4 opacity-50"
                    />
                    <ChevronRight
                      v-else
                      class="h-4 w-4 opacity-50"
                    />
                    <span class="text-xs font-bold">{{ section.name }}</span>
                  </div>

                  <!-- 分组全选/全不选切换 -->
                  <button
                    type="button"
                    class="btn btn-ghost btn-xs h-6 px-1.5"
                    @click.stop="batchWizardToggleContactGroup(
                      section.contacts,
                      batchWizardGroupSelectionState(section.contacts) !== 'all',
                    )"
                  >
                    <span
                      class="flex h-3.5 w-3.5 items-center justify-center rounded border transition"
                      :class="batchWizardStateBoxClasses(batchWizardGroupSelectionState(section.contacts))"
                    >
                      <Check v-if="batchWizardGroupSelectionState(section.contacts) === 'all'" class="h-2.5 w-2.5" stroke-width="3" />
                      <Minus v-else-if="batchWizardGroupSelectionState(section.contacts) === 'partial'" class="h-2.5 w-2.5" stroke-width="3" />
                    </span>
                  </button>
                </div>

                <!-- 分组内联系人列表 -->
                <div
                  v-if="batchWizardIsGroupExpanded(section.key)"
                  class="divide-y divide-base-200"
                >
                  <label
                    v-for="contact in section.contacts"
                    :key="contact.id"
                    class="flex items-center gap-3 px-3 py-2 cursor-pointer hover:bg-base-200/40"
                  >
                    <input
                      type="checkbox"
                      class="checkbox checkbox-sm"
                      :checked="batchSettingsWizard.selectedContactIds.includes(contact.id)"
                      @change="batchWizardToggleContact(contact.id, ($event.target as HTMLInputElement).checked)"
                    />
                    <div class="flex-1 min-w-0 flex items-center gap-2 text-xs">
                      <span class="badge badge-ghost badge-sm font-normal">
                        {{ contactAgentLabel(contact) }}
                      </span>
                      <span class="truncate font-medium">
                        {{ contactSafeDisplayName(contact) }}
                      </span>
                    </div>
                    <span
                      v-if="isPrivateContact(contact)"
                      class="badge badge-ghost badge-xs opacity-60"
                    >
                      {{ t("config.remoteIm.private") }}
                    </span>
                  </label>
                </div>
              </div>
            </div>
          </template>

          <!-- 第三步：变更预览 -->
          <template v-else-if="batchSettingsWizard.step === 3">
            <div
              v-if="batchWizardChangedPreviews.length === 0"
              class="rounded-box border border-base-300 p-8 text-center text-xs opacity-50"
            >
              {{ t("config.remoteIm.batchNoChange") }}
            </div>
            <div v-else class="space-y-2">
              <div class="text-xs opacity-70 font-medium px-1">
                {{ t("config.remoteIm.batchPreviewSummary", {
                  changed: batchWizardChangedPreviews.length,
                  total: batchWizardPreviews.length,
                }) }}
              </div>
              <div class="rounded-box border border-base-300 p-2 divide-y divide-base-200">
                <div
                  v-for="preview in batchWizardChangedPreviews"
                  :key="preview.contactId"
                  class="py-2.5 px-2"
                >
                  <div class="mb-1 text-xs font-semibold">{{ preview.name }}</div>
                  <div class="space-y-1">
                    <div
                      v-for="change in preview.changes"
                      :key="change.label"
                      class="flex flex-wrap items-center gap-1.5 text-xs"
                    >
                      <span class="shrink-0 opacity-70">{{ change.label }}：</span>
                      <span class="opacity-50 line-through">{{ change.from }}</span>
                      <span class="opacity-50">→</span>
                      <span class="font-medium text-primary">{{ change.to }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </template>

          <!-- 第四步：执行结果 -->
          <template v-else>
            <div v-if="batchSettingsWizard.result" class="flex flex-col items-center py-6 text-center">
              <div class="flex h-12 w-12 items-center justify-center rounded-full bg-success/15 text-success mb-3">
                <Check class="h-6 w-6 stroke-[2.5]" />
              </div>
              <div class="text-sm font-semibold mb-2">{{ t("common.success") || "设置已完成" }}</div>
              <div class="space-y-1 text-xs opacity-75">
                <div v-if="batchSettingsWizard.result.updatedContactCount > 0">
                  {{ t("config.remoteIm.batchResultContacts", {
                    count: batchSettingsWizard.result.updatedContactCount,
                  }) }}
                </div>
                <div v-if="batchSettingsWizard.result.movedContactCount > 0">
                  {{ t("config.remoteIm.batchResultMoved", {
                    count: batchSettingsWizard.result.movedContactCount,
                    name: contactGroupLabelById(batchSettingsWizard.targetGroupId),
                  }) }}
                </div>
                <div v-if="batchSettingsWizard.result.workspaceChangedContactCount > 0">
                  {{ t("config.remoteIm.batchResultWorkspace", {
                    count: batchSettingsWizard.result.workspaceChangedContactCount,
                  }) }}
                </div>
                <div v-if="batchSettingsWizard.fields.model">
                  {{ t("config.remoteIm.batchResultModel", {
                    updated: batchSettingsWizard.result.modelUpdatedCount,
                    skipped: batchSettingsWizard.result.modelSkippedCount,
                  }) }}
                </div>
              </div>
            </div>
          </template>
        </OverlayScrollArea>

        <!-- DaisyUI 原生 modal-action，shrink-0 确保永远固定在底部，绝不被裁切或遮盖 -->
        <div class="modal-action shrink-0 mt-3 pt-3 border-t border-base-content/10 flex items-center justify-between">
          <!-- 左侧：取消 / 上一步 -->
          <button
            v-if="batchSettingsWizard && batchSettingsWizard.step === 1"
            class="btn btn-sm btn-ghost"
            type="button"
            @click="closeBatchSettingsWizard"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            v-else-if="batchSettingsWizard && batchSettingsWizard.step > 1 && batchSettingsWizard.step < 4"
            class="btn btn-sm btn-ghost"
            type="button"
            :disabled="batchSettingsWizard.executing"
            @click="batchSettingsWizard.step -= 1"
          >
            {{ t("config.remoteIm.batchStepBack") }}
          </button>
          <span v-else></span>

          <!-- 右侧：下一步 / 执行设置 / 完成 -->
          <div v-if="batchSettingsWizard" class="flex items-center gap-2">
            <button
              v-if="batchSettingsWizard.step === 1"
              class="btn btn-sm btn-primary"
              type="button"
              @click="goToBatchWizardContactsStep"
            >
              {{ t("config.remoteIm.batchStepNext") }}
            </button>
            <button
              v-else-if="batchSettingsWizard.step === 2"
              class="btn btn-sm btn-primary"
              type="button"
              :disabled="batchSettingsWizard.preparing"
              @click="goToBatchWizardPreviewStep"
            >
              <span v-if="batchSettingsWizard.preparing" class="loading loading-spinner loading-xs"></span>
              {{ t("config.remoteIm.batchStepNext") }}
            </button>
            <button
              v-else-if="batchSettingsWizard.step === 3"
              class="btn btn-sm btn-primary"
              type="button"
              :disabled="batchSettingsWizard.executing || batchWizardChangedPreviews.length === 0"
              @click="executeBatchSettings"
            >
              <span v-if="batchSettingsWizard.executing" class="loading loading-spinner loading-xs"></span>
              {{ t("config.remoteIm.batchExecute") }}
            </button>
            <button
              v-else
              class="btn btn-sm btn-primary"
              type="button"
              @click="closeBatchSettingsWizard"
            >
              {{ t("common.close") }}
            </button>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeBatchSettingsWizard">close</button>
      </form>
    </dialog>
  </SettingsPageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  ClipboardPaste,
  Copy,
  ListChecks,
  Minus,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  ScrollText,
  Search,
  Settings,
  Trash2,
  User,
  Users,
  X,
} from "@lucide/vue";
import { invokeTauri, openTransportFileDialog } from "../../../../services/tauri-api";
import type { AppConfig, LegacyShellWorkspaceAccess, PersonaProfile, RemoteImChannelConfig, RemoteImContact, RemoteImContactGroup, RemoteImPlatform, ShellWorkspace, ShellWorkspaceAccess } from "../../../../types/app";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbBadge, SettingsBreadcrumbItem } from "../../components/SettingsBreadcrumb.vue";
import AgentPersonaSelect from "../../../shared/components/AgentPersonaSelect.vue";
import ApiConfigPicker from "../../components/ApiConfigPicker.vue";
import WorkspaceConfigCard from "../../../shared/components/WorkspaceConfigCard.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import ChannelBehaviorSettingsModal from "./remote-im/ChannelBehaviorSettingsModal.vue";
import type { ChannelConnectionStatus, ChannelLogEntry, WeixinLoginStatus } from "./remote-im/types";
import { buildContactLogDisplayItem, type ContactLogDisplayItem } from "./remote-im/contact-log-display";
import {
  contactCommunicationToggleClass,
  contactCommunicationToggleEnabled,
  cloneChannelBehaviorSettings,
  formatLogTime,
  normalizeActivationMode,
  normalizeProcessingMode,
  normalizeResponseStrategy,
  parseActivationKeywords,
} from "./remote-im/helpers";

const props = defineProps<{
  config: AppConfig;
  personas: PersonaProfile[];
  personaAvatarUrlMap: Record<string, string>;
  saveConfigAction: () => Promise<boolean> | boolean;
  setStatusAction: (text: string) => void;
}>();

const { t } = useI18n();
const WEIXIN_OC_BOT_TYPE = "3";
const WEIXIN_OC_QR_POLL_INTERVAL = 1;
const WEIXIN_OC_LONG_POLL_TIMEOUT_MS = 35000;
const WEIXIN_OC_API_TIMEOUT_MS = 15000;
type ContactPillMenuKind = "activation" | "processing" | "response" | "files" | "group";
type ContactPillMenuOption = {
  key: string;
  label: string;
  active: boolean;
  value: string | boolean;
};
type ContactPillMenuState = {
  contactId: string;
  kind: ContactPillMenuKind;
  left: number;
  top: number;
  widthClass: string;
  options: ContactPillMenuOption[];
};
type BatchWizardFieldKey =
  | "agent"
  | "model"
  | "processingMode"
  | "activation"
  | "responseStrategy"
  | "communication"
  | "workspaceAccess"
  | "group";
type BatchWizardChange = { label: string; from: string; to: string };
type BatchWizardPreview = { contactId: string; name: string; changes: BatchWizardChange[] };
type BatchWizardContactSection = { key: string; name: string; contacts: RemoteImContact[] };
type BatchSettingsWizardState = {
  step: number;
  fields: Record<BatchWizardFieldKey, boolean>;
  agentId: string;
  setPreferredModel: boolean;
  preferredApiConfigId: string;
  processingMode: "qa" | "continuous";
  activationMode: RemoteImContact["activationMode"];
  activationKeywordsText: string;
  responseStrategy: NonNullable<RemoteImContact["responseStrategy"]>;
  allowCommunication: boolean;
  allowSendFiles: boolean;
  workspaceAccess: LegacyShellWorkspaceAccess;
  /** 移动到分组的目标分组 id，空字符串表示未分组。 */
  targetGroupId: string;
  selectedContactIds: string[];
  contactSearchQuery: string;
  contactModels: Record<string, ContactConversationModel>;
  preparing: boolean;
  executing: boolean;
  error: string;
  result: {
    updatedContactCount: number;
    workspaceChangedContactCount: number;
    modelUpdatedCount: number;
    modelSkippedCount: number;
    movedContactCount: number;
  } | null;
};
type ContactSettingsClipboard = {
  boundAgentId: string;
  /** 会话首选模型：模型属于联系人的会话，复制设置时一并带上。 */
  preferredApiConfigId: string;
  processingMode: "qa" | "continuous";
  activationMode: RemoteImContact["activationMode"];
  activationKeywordsText: string;
  responseStrategy: NonNullable<RemoteImContact["responseStrategy"]>;
  allowReceive: boolean;
  allowSend: boolean;
  allowSendFiles: boolean;
};
const saving = ref(false);
const contactsLoading = ref(false);
const contactsError = ref("");
const contacts = ref<RemoteImContact[]>([]);
/** 联系人自定义分组：后端按渠道存储，前端只缓存当前加载结果。 */
const contactGroups = ref<RemoteImContactGroup[]>([]);
const contactGroupEditMode = ref<"create" | "rename" | null>(null);
const contactGroupEditTargetId = ref("");
const contactGroupNameDraft = ref("");
const contactGroupError = ref("");
const contactGroupBusy = ref(false);
const batchSettingsDialogRef = ref<HTMLDialogElement | null>(null);
const batchSettingsWizard = ref<BatchSettingsWizardState | null>(null);
const channelOperationIds = ref<Record<string, boolean>>({});
const contactOperationIds = ref<Record<string, boolean>>({});
const credentialDrafts = ref<Record<string, string>>({});
const napcatCredentials = ref({ wsHost: "0.0.0.0", wsPort: 6199, wsToken: "" });
const dingtalkCredentials = ref({ clientId: "", clientSecret: "" });
const weixinCredentials = ref({
  baseUrl: "https://ilinkai.weixin.qq.com",
  botType: WEIXIN_OC_BOT_TYPE,
  qrPollInterval: WEIXIN_OC_QR_POLL_INTERVAL,
  longPollTimeoutMs: WEIXIN_OC_LONG_POLL_TIMEOUT_MS,
  apiTimeoutMs: WEIXIN_OC_API_TIMEOUT_MS,
});
const showDingtalkSecret = ref(false);
const suppressCredentialSync = ref(false);
const inDetailMode = ref(false);
const channelSearchQuery = ref("");
const contactSearchQuery = ref("");

const selectedChannelId = ref<string>("");
const channels = computed(() => props.config.remoteImChannels || []);

const filteredChannels = computed(() => {
  const q = channelSearchQuery.value.trim().toLowerCase();
  if (!q) return channels.value;
  return channels.value.filter(
    (ch) =>
      (ch.name || "").toLowerCase().includes(q) ||
      platformLabelText(ch.platform).toLowerCase().includes(q)
  );
});

function enterChannel(channelId: string) {
  selectedChannelId.value = channelId;
  inDetailMode.value = true;
  cancelContactGroupEdit();
  void refreshContactGroups();
}

function backToChannels() {
  if (channelDirty.value) {
    const confirmLeave = window.confirm(t("config.skill.confirmLeaveUnsaved") || "当前渠道有未保存的修改，确认返回列表吗？");
    if (!confirmLeave) return;
  }
  inDetailMode.value = false;
}

function getChannelContactCount(channelId: string): number {
  return contacts.value.filter((c) => c.channelId === channelId).length;
}

function getChannelStatusInfo(channel: RemoteImChannelConfig) {
  if (!channel.enabled) {
    return { dot: "bg-base-300", text: t("config.remoteIm.statusDisabled"), connected: false };
  }
  const runtime = channelRuntimeStates.value[channel.id];
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk") {
    if (runtime?.connected) {
      return { dot: "bg-success", text: t("config.remoteIm.statusConnected"), connected: true };
    }
    return { dot: "bg-warning", text: t("config.remoteIm.statusConnecting"), connected: false };
  }
  if (channel.platform === "weixin_oc") {
    const wLogin = weixinLoginStates.value[channel.id];
    if (runtime?.connected || wLogin?.connected) {
      return { dot: "bg-success", text: t("config.remoteIm.statusConnected"), connected: true };
    }
    return { dot: "bg-warning", text: t("config.remoteIm.waitingScan"), connected: false };
  }
  if (channel.platform === "feishu") {
    return { dot: "bg-info", text: t("config.remoteIm.statusConnected"), connected: true };
  }
  return { dot: "bg-success", text: t("config.remoteIm.statusConnected"), connected: true };
}

function platformBadgeText(platform: RemoteImPlatform): string {
  switch (platform) {
    case "weixin_oc": return "微";
    case "dingtalk": return "钉";
    case "feishu": return "飞";
    case "onebot_v11": return "QQ";
    default: return "IM";
  }
}

function getPlatformIconColor(platform: RemoteImPlatform): string {
  switch (platform) {
    case "weixin_oc":
      return "text-emerald-500 bg-emerald-500/10 border-emerald-500/20";
    case "dingtalk":
      return "text-blue-500 bg-blue-500/10 border-blue-500/20";
    case "feishu":
      return "text-cyan-500 bg-cyan-500/10 border-cyan-500/20";
    case "onebot_v11":
      return "text-amber-500 bg-amber-500/10 border-amber-500/20";
    default:
      return "text-primary bg-primary/10 border-primary/20";
  }
}

const channelStatus = ref<ChannelConnectionStatus | null>(null);
const channelLogs = ref<ChannelLogEntry[]>([]);
const channelLogsDialogRef = ref<HTMLDialogElement | null>(null);
const channelLogsModalOpen = ref(false);
const channelLogsLoading = ref(false);
const contactLogs = ref<ChannelLogEntry[]>([]);
const contactLogsDialogRef = ref<HTMLDialogElement | null>(null);
const contactLogsModalOpen = ref(false);
const contactLogsLoading = ref(false);
const contactLogsContactId = ref("");
const addChannelDialogRef = ref<HTMLDialogElement | null>(null);
const addChannelModalOpen = ref(false);
const channelConfigDialogRef = ref<HTMLDialogElement | null>(null);
const channelConfigModalOpen = ref(false);

function syncAddChannelDialog() {
  const d = addChannelDialogRef.value;
  if (!d) return;
  if (addChannelModalOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function syncChannelLogsDialog() {
  const d = channelLogsDialogRef.value;
  if (!d) return;
  if (channelLogsModalOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function syncContactLogsDialog() {
  const d = contactLogsDialogRef.value;
  if (!d) return;
  if (contactLogsModalOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}
function syncChannelConfigDialog() {
  const d = channelConfigDialogRef.value;
  if (!d) return;
  if (channelConfigModalOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(addChannelModalOpen, syncAddChannelDialog);
watch(addChannelDialogRef, syncAddChannelDialog);
watch(channelLogsModalOpen, syncChannelLogsDialog);
watch(channelLogsDialogRef, syncChannelLogsDialog);
watch(contactLogsModalOpen, syncContactLogsDialog);
watch(contactLogsDialogRef, syncContactLogsDialog);
watch(channelConfigModalOpen, syncChannelConfigDialog);
watch(channelConfigDialogRef, syncChannelConfigDialog);
const selectedContactId = ref<string>("");
const contactPillMenu = ref<ContactPillMenuState | null>(null);
const contactSettingsClipboard = ref<ContactSettingsClipboard | null>(null);
const contactSaving = ref(false);
const contactDeleting = ref(false);
const channelRuntimeStates = ref<Record<string, ChannelConnectionStatus | null>>({});
const weixinLoginStates = ref<Record<string, WeixinLoginStatus | null>>({});
const weixinLoginBusy = ref(false);
let weixinLoginPollTimer: ReturnType<typeof setInterval> | null = null;
let channelStatusTimer: ReturnType<typeof setInterval> | null = null;

const selectedChannel = computed(() =>
  channels.value.find((ch) => ch.id === selectedChannelId.value) ?? null,
);

const weixinLoginState = computed(() => {
  const channelId = selectedChannel.value?.id || "";
  return weixinLoginStates.value[channelId] || {
    channelId,
    connected: false,
    status: "",
    message: "",
    sessionKey: "",
    qrcode: "",
    qrcodeImgContent: "",
    accountId: "",
    userId: "",
    baseUrl: "",
    lastError: "",
  };
});

function looksLikeBase64(value: string): boolean {
  if (!value || value.length < 64) return false;
  return /^[A-Za-z0-9+/=]+$/.test(value);
}

const weixinQrImageSrc = computed(() => {
  const raw = String(weixinLoginState.value.qrcodeImgContent || "").trim();
  if (!raw) return "";
  if (raw.startsWith("data:image/")) return raw;
  if (/^https?:\/\//i.test(raw)) {
    return `https://api.qrserver.com/v1/create-qr-code/?size=384x384&margin=0&data=${encodeURIComponent(raw)}`;
  }
  if (looksLikeBase64(raw)) {
    return `data:image/png;base64,${raw}`;
  }
  return raw;
});
const persistedWeixinCredentials = computed(() => {
  const creds = selectedChannel.value?.credentials;
  if (!creds || typeof creds !== "object") {
    return { token: "", accountId: "", userId: "" };
  }
  const record = creds as Record<string, unknown>;
  return {
    token: String(record.token || "").trim(),
    accountId: String(record.accountId || "").trim(),
    userId: String(record.userId || "").trim(),
  };
});
const weixinRuntimeStatus = computed(() =>
  selectedChannel.value ? channelRuntimeStates.value[selectedChannel.value.id] ?? null : null,
);
const weixinStatusText = computed(() => {
  if (weixinRuntimeStatus.value?.connected) return t('config.remoteIm.weixinConnected');
  if (isWeixinLoggedIn.value) return t('config.remoteIm.weixinLoggedIn');
  const status = String(weixinLoginState.value.status || "").trim().toLowerCase();
  if (status === "wait" || status === "scanned" || status === "scaned") return t('config.remoteIm.waitingScanConfirm');
  if (status === "need_login" || status === "idle") return t('config.remoteIm.waitingScan');
  if (status === "confirmed" || status === "logged_in") return t('config.remoteIm.weixinLoggedIn');
  return t('config.remoteIm.waitingScan');
});
const weixinStatusMessage = computed(() => {
  if (weixinRuntimeStatus.value?.connected) {
    return t('config.remoteIm.credentialsSaved');
  }
  if (isWeixinLoggedIn.value) {
    return t('config.remoteIm.credentialsSaved');
  }
  const status = String(weixinLoginState.value.status || "").trim().toLowerCase();
  if (status === "wait" || status === "scanned" || status === "scaned") {
    return t('config.remoteIm.confirmLoginInWeixin');
  }
  const errorMessage = String(weixinLoginState.value.lastError || "").trim();
  return errorMessage || "";
});
const isWeixinLoggedIn = computed(() => {
  const status = String(weixinLoginState.value.status || "").trim().toLowerCase();
  if (weixinLoginState.value.connected) return true;
  if (weixinRuntimeStatus.value?.connected) return true;
  if (status === "confirmed" || status === "logged_in") return true;
  if (!!String(weixinLoginState.value.accountId || "").trim()) return true;
  if (!!persistedWeixinCredentials.value.token) return true;
  return !!persistedWeixinCredentials.value.accountId;
});
const channelPlatformOptions = computed<Array<{ platform: RemoteImPlatform; label: string }>>(() => [
  { platform: "onebot_v11", label: t("config.remoteIm.platformOptions.onebotV11") },
  { platform: "feishu", label: t("config.remoteIm.platformOptions.feishu") },
  { platform: "dingtalk", label: t("config.remoteIm.platformOptions.dingtalk") },
  { platform: "weixin_oc", label: t("config.remoteIm.platformOptions.weixinOc") },
]);

const channelSnapshot = computed(() => {
  const ch = selectedChannel.value;
  if (!ch) return "";
  const credStr = JSON.stringify(ch.credentials || {}, Object.keys(ch.credentials || {}).sort());
  return JSON.stringify({
    name: ch.name,
    platform: ch.platform,
    enabled: ch.enabled,
    receiveFiles: ch.receiveFiles,
    streamingSend: ch.streamingSend,
    showToolCalls: ch.showToolCalls,
    filterMarkdown: ch.filterMarkdown,
    credentials: credStr,
  });
});
const lastSavedChannelSnapshot = ref(channelSnapshot.value);
const channelDirty = computed(() => channelSnapshot.value !== lastSavedChannelSnapshot.value);

const remoteImBreadcrumb = computed<SettingsBreadcrumbItem[]>(() => {
  const channel = selectedChannel.value;
  if (!inDetailMode.value || !channel) return [{ label: t("config.tabs.remoteIm") }];
  const badges: SettingsBreadcrumbBadge[] = [];
  const status = getChannelStatusInfo(channel);
  badges.push({
    text: status.text,
    class: channel.enabled ? "badge-neutral" : "border-base-300 bg-base-100 text-base-content/60",
    dotClass: status.dot,
  });
  if (channelDirty.value) badges.push({ text: t("config.skill.unsaved") });
  return [
    { label: t("config.tabs.remoteIm"), title: t("config.remoteIm.backToChannels"), onClick: backToChannels },
    { label: channel.name || platformLabelText(channel.platform), badges },
  ];
});

function isChannelOperationBusy(channelId: string): boolean {
  return !!channelOperationIds.value[channelId];
}

function setChannelOperationBusy(channelId: string, busy: boolean) {
  if (busy) {
    channelOperationIds.value = { ...channelOperationIds.value, [channelId]: true };
    return;
  }
  const next = { ...channelOperationIds.value };
  delete next[channelId];
  channelOperationIds.value = next;
}

function isContactOperationBusy(contactId: string): boolean {
  return !!contactOperationIds.value[contactId];
}

async function withContactOperation(contactId: string, action: () => Promise<void>) {
  if (isContactOperationBusy(contactId)) return;
  contactOperationIds.value = { ...contactOperationIds.value, [contactId]: true };
  try {
    await action();
  } finally {
    const next = { ...contactOperationIds.value };
    delete next[contactId];
    contactOperationIds.value = next;
  }
}

/** 联系人按平台账号排序（QQ 号这类纯数字按数值大小，其它按字典序），避免按最近消息时间导致列表频繁变动。 */
function compareContactByRemoteId(a: RemoteImContact, b: RemoteImContact): number {
  return String(a.remoteContactId || "").localeCompare(
    String(b.remoteContactId || ""),
    undefined,
    { numeric: true },
  );
}

const currentChannelContacts = computed(() => {
  if (!selectedChannelId.value) return [];
  return contacts.value
    .filter((c) => c.channelId === selectedChannelId.value)
    .sort(compareContactByRemoteId);
});

/** 联系人分组：每个渠道各自一套，只属于一个分组，未分组用空 groupId 表示。 */
const currentChannelGroups = computed(() => (
  contactGroups.value
    .filter((group) => group.channelId === selectedChannelId.value)
    .slice()
    .sort((a, b) => (a.sortOrder - b.sortOrder) || a.name.localeCompare(b.name))
));

function contactGroupIdOf(contact: RemoteImContact): string {
  return String(contact.groupId || "").trim();
}

/** 分组名：未分组、以及指向不存在分组的 groupId，都回落为「未分组」。 */
function contactGroupLabelById(groupId: string): string {
  const id = String(groupId || "").trim();
  if (!id) return t("config.remoteIm.contactGroupUngrouped");
  return currentChannelGroups.value.find((group) => group.id === id)?.name
    ?? t("config.remoteIm.contactGroupUngrouped");
}

/** 联系人列表分段：按自定义分组罗列，未分组固定最后，空分组不占位。 */
type ContactListSection = {
  key: string;
  name: string;
  group: RemoteImContactGroup | null;
  items: RemoteImContact[];
};

const contactListSections = computed<ContactListSection[]>(() => {
  const q = contactSearchQuery.value.trim().toLowerCase();
  const matches = (item: RemoteImContact) => {
    if (!q) return true;
    const name = contactSafeDisplayName(item).toLowerCase();
    const agent = contactAgentLabel(item).toLowerCase();
    const sec = contactSecondaryText(item).toLowerCase();
    return name.includes(q) || agent.includes(q) || sec.includes(q);
  };
  const all = currentChannelContacts.value;
  const sections: ContactListSection[] = [];
  for (const group of currentChannelGroups.value) {
    const items = all.filter((item) => contactGroupIdOf(item) === group.id && matches(item));
    if (items.length > 0) sections.push({ key: group.id, name: group.name, group, items });
  }
  const ungrouped = all.filter((item) => !contactGroupIdOf(item) && matches(item));
  if (ungrouped.length > 0) {
    sections.push({
      key: "ungrouped",
      name: t("config.remoteIm.contactGroupUngrouped"),
      group: null,
      items: ungrouped,
    });
  }
  return sections;
});

const collapsedContactSections = ref<Set<string>>(new Set());

function contactSectionCollapseKey(section: ContactListSection): string {
  return `${selectedChannelId.value}::${section.key}`;
}

function isContactSectionCollapsed(section: ContactListSection): boolean {
  return collapsedContactSections.value.has(contactSectionCollapseKey(section));
}

function toggleContactSectionCollapse(section: ContactListSection) {
  const key = contactSectionCollapseKey(section);
  const next = new Set(collapsedContactSections.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  collapsedContactSections.value = next;
}

const contactActivationModeOrder: RemoteImContact["activationMode"][] = ["always", "keyword", "never"];

const selectedContact = computed(() =>
  currentChannelContacts.value.find((item) => item.id === selectedContactId.value) ?? null,
);
const contactLogsTarget = computed(() =>
  contacts.value.find((item) => item.id === contactLogsContactId.value) ?? null,
);
const contactLogsTitle = computed(() => {
  const target = contactLogsTarget.value;
  if (!target) return "-";
  return contactSafeDisplayName(target);
});
type ContactEditDraft = {
  boundAgentId: string;
  activationKeywordsText: string;
  shellWorkspaces: ShellWorkspace[];
};
const contactDraft = ref<ContactEditDraft | null>(null);
/**
 * 当前展开联系人的会话模型归属：模型存在联系人的会话（preferred_api_config_id）。
 * conversationExists 为 false 表示会话已被删除/归档，此时模型选择器禁用。
 */
const contactConversationModel = ref<{
  conversationId: string;
  conversationExists: boolean;
  preferredApiConfigId: string;
}>({ conversationId: "", conversationExists: false, preferredApiConfigId: "" });
const contactDraftSnapshot = ref("");
const contactDraftDirty = computed(() =>
  !!contactDraft.value && JSON.stringify(contactDraft.value) !== contactDraftSnapshot.value,
);
const contactLogDisplayItems = computed<ContactLogDisplayItem[]>(() =>
  contactLogs.value
    .map((log) => buildContactLogDisplayItem(log, t))
    .filter((item): item is ContactLogDisplayItem => item !== null),
);
const contactLogDisplayLines = computed(() =>
  contactLogDisplayItems.value.map((item) => {
    const parts = [
      formatLogTime(item.timestamp),
      `[${item.kind}]`,
      item.title,
      item.summary,
      item.detail,
    ].filter((value) => String(value || "").trim().length > 0);
    return parts.join("  ");
  }),
);

const contactKeywordDrafts = ref<Record<string, string>>({});

function buildContactDraftFromContact(item: RemoteImContact): ContactEditDraft {
  return {
    boundAgentId: String(item.boundAgentId || "").trim() || "support",
    activationKeywordsText: item.activationKeywords.join(", "),
    shellWorkspaces: (item as any).shellWorkspaces
      ? (item as any).shellWorkspaces.filter((ws: any) => ws.level !== "system").map((ws: any) => ({
          id: ws.id || crypto.randomUUID(),
          name: ws.name || "",
          path: ws.path || "",
          level: ws.level || "secondary",
          access: ws.access || "full_access",
        }))
      : [],
  };
}

function buildContactSettingsClipboard(
  item: RemoteImContact,
): Omit<ContactSettingsClipboard, "preferredApiConfigId"> {
  const isPrivate = isPrivateContact(item);
  return {
    boundAgentId: String(item.boundAgentId || "").trim() || "support",
    processingMode: normalizeProcessingMode(item.processingMode),
    activationMode: isPrivate ? "always" : normalizeActivationMode(item.activationMode || "never"),
    activationKeywordsText: isPrivate ? "" : (Array.isArray(item.activationKeywords) ? item.activationKeywords.join(", ") : ""),
    responseStrategy: isPrivate ? "always_reply" : normalizeResponseStrategy(item.responseStrategy),
    allowReceive: !!item.allowReceive,
    allowSend: !!item.allowSend,
    allowSendFiles: !!item.allowSendFiles,
  };
}

function syncSelectedContactDraft() {
  if (!selectedContact.value) {
    contactDraft.value = null;
    contactDraftSnapshot.value = "";
    return;
  }
  const draft = buildContactDraftFromContact(selectedContact.value);
  contactDraft.value = draft;
  contactDraftSnapshot.value = JSON.stringify(draft);
}

type ContactConversationModel = {
  conversationId: string;
  conversationExists: boolean;
  preferredApiConfigId: string;
};

/**
 * 解析有效文本模型：当模型为空或丢失（不在可用文本模型列表）时，默认切到专家模型。
 */
function resolveAvailableOrExpertModelId(modelId?: string | null): string {
  const raw = String(modelId || "").trim();
  const textConfigs = (props.config.apiConfigs || []).filter((item) => !!item.enableText);
  if (raw && textConfigs.some((item) => item.id === raw)) {
    return raw;
  }
  const expertId = String(props.config.expertApiConfigId || "").trim();
  if (expertId && textConfigs.some((item) => item.id === expertId)) {
    return expertId;
  }
  return textConfigs[0]?.id || "";
}

/** 只读解析联系人的会话模型；会话不存在时返回 conversationExists=false。模型为空或丢失时默认切到专家模型。 */
async function fetchContactConversationModel(contactId: string): Promise<ContactConversationModel> {
  try {
    const result = await invokeTauri<{
      conversationId?: string | null;
      conversationExists?: boolean;
      preferredApiConfigId?: string | null;
    }>("remoteIm.contact.conversationModel", { input: { contactId } });
    const rawModel = String(result.preferredApiConfigId || "").trim();
    const resolvedModel = resolveAvailableOrExpertModelId(rawModel);
    return {
      conversationId: String(result.conversationId || "").trim(),
      conversationExists: !!result.conversationExists,
      preferredApiConfigId: resolvedModel,
    };
  } catch (error) {
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
    return { conversationId: "", conversationExists: false, preferredApiConfigId: "" };
  }
}

async function refreshContactConversationModel(contactId: string) {
  contactConversationModel.value = await fetchContactConversationModel(contactId);
}

async function onContactConversationModelChange(apiConfigIdRaw: string) {
  let target = contactConversationModel.value;
  if (!target.conversationId && selectedContact.value) {
    target = await fetchContactConversationModel(selectedContact.value.id);
    contactConversationModel.value = target;
  }
  if (!target.conversationId) return;
  const nextApiConfigId = String(apiConfigIdRaw || "").trim();
  const previous = target.preferredApiConfigId;
  contactConversationModel.value = { ...target, preferredApiConfigId: nextApiConfigId };
  try {
    await invokeTauri("conversation.preferredModel.set", {
      input: {
        conversationId: target.conversationId,
        preferredApiConfigId: nextApiConfigId || null,
      },
    });
    props.setStatusAction(t("config.remoteIm.contactContinueSession"));
  } catch (error) {
    contactConversationModel.value = { ...target, preferredApiConfigId: previous };
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function asNonEmptyString(value: unknown): string {
  return String(value || "").trim();
}

function validateChannelBeforeEnable(channel: RemoteImChannelConfig): string {
  const creds = channel.credentials || {};
  if (channel.platform === "dingtalk") {
    const clientId = asNonEmptyString(creds.clientId);
    const clientSecret = asNonEmptyString(creds.clientSecret);
    if (!clientId || !clientSecret) {
      return t("config.remoteIm.enableNeedDingtalkCredentials");
    }
  }
  if (channel.platform === "feishu") {
    const appId = asNonEmptyString(creds.appId);
    const appSecret = asNonEmptyString(creds.appSecret);
    if (!appId || !appSecret) {
      return t("config.remoteIm.enableNeedFeishuCredentials");
    }
  }
  if (channel.platform === "weixin_oc") {
    const baseUrl = asNonEmptyString(creds.baseUrl) || "https://ilinkai.weixin.qq.com";
    channel.credentials = {
      ...creds,
      baseUrl,
      botType: WEIXIN_OC_BOT_TYPE,
      qrPollInterval: WEIXIN_OC_QR_POLL_INTERVAL,
      longPollTimeoutMs: WEIXIN_OC_LONG_POLL_TIMEOUT_MS,
      apiTimeoutMs: WEIXIN_OC_API_TIMEOUT_MS,
    };
  }
  return "";
}

function defaultChannelName(platform: RemoteImPlatform): string {
  if (platform === "feishu") return "Feishu";
  if (platform === "dingtalk") return "DingTalk";
  if (platform === "weixin_oc") return t('config.remoteIm.weixinPlatform');
  return "OneBot v11";
}

function newChannel(platform: RemoteImPlatform = "onebot_v11"): RemoteImChannelConfig {
  return {
    id: `remote-im-${Date.now()}`,
    name: defaultChannelName(platform),
    platform,
    enabled: false,
    credentials: {},
    receiveFiles: true,
    streamingSend: false,
    showToolCalls: false,
    filterMarkdown: false,
    allowSendFiles: false,
    behaviorSettings: cloneChannelBehaviorSettings(),
  };
}

function openAddChannelModal() {
  addChannelModalOpen.value = true;
}

function closeAddChannelModal() {
  addChannelModalOpen.value = false;
}

function addChannel(platform: RemoteImPlatform) {
  const ch = newChannel(platform);
  props.config.remoteImChannels.push(ch);
  selectedChannelId.value = ch.id;
  channelConfigModalOpen.value = true;
  addChannelModalOpen.value = false;
}

function removeChannelById(channelId: string) {
  const idx = channels.value.findIndex((ch) => ch.id === channelId);
  if (idx >= 0) {
    props.config.remoteImChannels.splice(idx, 1);
    if (selectedChannelId.value === channelId) {
      const nextIdx = Math.min(idx, channels.value.length - 1);
      selectedChannelId.value = nextIdx >= 0 ? channels.value[nextIdx].id : "";
    }
  }
}

function cloneRemoteImChannel(channel: RemoteImChannelConfig): RemoteImChannelConfig {
  return {
    ...channel,
    credentials: channel.credentials && typeof channel.credentials === "object"
      ? { ...channel.credentials }
      : {},
    behaviorSettings: cloneChannelBehaviorSettings(channel.behaviorSettings),
  };
}

function restoreRemovedChannel(index: number, channel: RemoteImChannelConfig) {
  if (props.config.remoteImChannels.some((item) => item.id === channel.id)) return;
  const insertIndex = Math.max(0, Math.min(index, props.config.remoteImChannels.length));
  props.config.remoteImChannels.splice(insertIndex, 0, cloneRemoteImChannel(channel));
  selectedChannelId.value = channel.id;
  channelConfigModalOpen.value = true;
  lastSavedChannelSnapshot.value = channelSnapshot.value;
}

async function deleteSelectedChannel() {
  const channel = selectedChannel.value;
  if (!channel || saving.value || isChannelOperationBusy(channel.id)) return;
  const channelName = String(channel.name || channel.id).trim() || channel.id;
  const confirmed = window.confirm(t("config.remoteIm.deleteChannelConfirm", { name: channelName }));
  if (!confirmed) return;

  const removedIndex = props.config.remoteImChannels.findIndex((item) => item.id === channel.id);
  if (removedIndex < 0) return;
  const removedChannel = cloneRemoteImChannel(channel);
  const removedChannelId = removedChannel.id;

  saving.value = true;
  setChannelOperationBusy(removedChannelId, true);
  removeChannelById(removedChannelId);
  closeChannelConfigModal();
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      restoreRemovedChannel(removedIndex, removedChannel);
      props.setStatusAction(t("config.remoteIm.deleteChannelFailed", { error: t("config.remoteIm.channelSaveFailed") }));
      return;
    }
    const nextDrafts = { ...credentialDrafts.value };
    delete nextDrafts[removedChannelId];
    credentialDrafts.value = nextDrafts;
    const nextRuntimeStates = { ...channelRuntimeStates.value };
    delete nextRuntimeStates[removedChannelId];
    channelRuntimeStates.value = nextRuntimeStates;
    if (channelLogsModalOpen.value) {
      channelLogs.value = [];
      closeChannelLogsModal();
    }
    await nextTick();
    lastSavedChannelSnapshot.value = channelSnapshot.value;
    props.setStatusAction(t("config.remoteIm.deleteChannelSuccess", { name: channelName }));
  } catch (error) {
    restoreRemovedChannel(removedIndex, removedChannel);
    props.setStatusAction(t("config.remoteIm.deleteChannelFailed", { error: String(error) }));
  } finally {
    setChannelOperationBusy(removedChannelId, false);
    saving.value = false;
  }
}

function syncCredentialJson(channel: RemoteImChannelConfig) {
  const raw = String(credentialDrafts.value[channel.id] || "").trim();
  if (!raw) {
    channel.credentials = {};
    return;
  }
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("credentials json must be object");
    }
    channel.credentials = parsed as Record<string, unknown>;
  } catch (error) {
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function loadNapcatCredentials(channel: RemoteImChannelConfig) {
  suppressCredentialSync.value = true;
  const creds = channel.credentials || {};
  napcatCredentials.value = {
    wsHost: String(creds.wsHost || "0.0.0.0"),
    wsPort: Number(creds.wsPort) || 6199,
    wsToken: String(creds.wsToken || ""),
  };
  nextTick(() => {
    suppressCredentialSync.value = false;
  });
}

function loadDingtalkCredentials(channel: RemoteImChannelConfig) {
  suppressCredentialSync.value = true;
  const creds = channel.credentials || {};
  dingtalkCredentials.value = {
    clientId: String(creds.clientId || creds.clientID || ""),
    clientSecret: String(creds.clientSecret || creds.appSecret || ""),
  };
  nextTick(() => {
    suppressCredentialSync.value = false;
  });
}

function loadWeixinCredentials(channel: RemoteImChannelConfig) {
  suppressCredentialSync.value = true;
  const creds = channel.credentials || {};
  weixinCredentials.value = {
    baseUrl: String(creds.baseUrl || "https://ilinkai.weixin.qq.com"),
    botType: WEIXIN_OC_BOT_TYPE,
    qrPollInterval: WEIXIN_OC_QR_POLL_INTERVAL,
    longPollTimeoutMs: WEIXIN_OC_LONG_POLL_TIMEOUT_MS,
    apiTimeoutMs: WEIXIN_OC_API_TIMEOUT_MS,
  };
  nextTick(() => {
    suppressCredentialSync.value = false;
  });
}

function resetNapcatCredentials() {
  if (!selectedChannel.value) return;
  loadNapcatCredentials(selectedChannel.value);
  // 同时更新 channelSnapshot 以清除 dirty 状态
  lastSavedChannelSnapshot.value = channelSnapshot.value;
}

async function saveChannels() {
  if (saving.value || !selectedChannel.value) return false;
  const savedId = selectedChannelId.value;
  if (isChannelOperationBusy(savedId)) return false;
  if (selectedChannel.value.platform === "feishu") {
    syncCredentialJson(selectedChannel.value);
  }
  saving.value = true;
  setChannelOperationBusy(savedId, true);
  try {
    const result = await Promise.resolve(props.saveConfigAction());
    if (result) {
      if (channels.value.some((ch) => ch.id === savedId)) {
        selectedChannelId.value = savedId;
      }
      if (selectedChannel.value) {
        if (selectedChannel.value.platform === "onebot_v11") {
          loadNapcatCredentials(selectedChannel.value);
        } else if (selectedChannel.value.platform === "dingtalk") {
          loadDingtalkCredentials(selectedChannel.value);
        } else if (selectedChannel.value.platform === "weixin_oc") {
          loadWeixinCredentials(selectedChannel.value);
        }
        if (selectedChannel.value.platform === "onebot_v11" || selectedChannel.value.platform === "dingtalk" || selectedChannel.value.platform === "weixin_oc") {
          try {
            const status = await invokeTauri<ChannelConnectionStatus>(
              "remote_im_restart_channel",
              { channelId: selectedChannel.value.id },
            );
            channelStatus.value = status;
            channelRuntimeStates.value = {
              ...channelRuntimeStates.value,
              [selectedChannel.value.id]: status,
            };
          } catch (err) {
            console.warn("[远程IM] restart channel failed:", err);
            void refreshChannelStatus();
          }
        }
      }
      await nextTick();
      lastSavedChannelSnapshot.value = channelSnapshot.value;
      return true;
    }
    return false;
  } finally {
    setChannelOperationBusy(savedId, false);
    saving.value = false;
  }
}

async function toggleChannelEnabled(channel: RemoteImChannelConfig, enabled: boolean) {
  if (saving.value || isChannelOperationBusy(channel.id)) return;
  const previousEnabled = channel.enabled;
  props.setStatusAction(t('config.remoteIm.togglingChannel', { action: enabled ? t('config.remoteIm.show') : t('config.remoteIm.hide'), name: channel.name || channel.id }));
  if (enabled) {
    const validationError = validateChannelBeforeEnable(channel);
    if (validationError) {
      props.setStatusAction(validationError);
      return;
    }
  }
  channel.enabled = enabled;
  saving.value = true;
  setChannelOperationBusy(channel.id, true);
  try {
    const result = await Promise.resolve(props.saveConfigAction());
    if (result) {
      props.setStatusAction(enabled ? t('config.remoteIm.channelEnabled') : t('config.remoteIm.channelToggled'));
      if (channel.platform === "onebot_v11" || channel.platform === "dingtalk" || channel.platform === "weixin_oc") {
        try {
          const status = await invokeTauri<ChannelConnectionStatus>(
            "remote_im_restart_channel",
            { channelId: channel.id },
          );
          channelStatus.value = status;
          channelRuntimeStates.value = {
            ...channelRuntimeStates.value,
            [channel.id]: status,
          };
        } catch (err) {
          console.warn("[远程IM] restart channel failed:", err);
          props.setStatusAction(t('config.remoteIm.channelToggleFailed', { error: String(err) }));
          void refreshChannelStatus();
        }
      }
      await nextTick();
      lastSavedChannelSnapshot.value = channelSnapshot.value;
    } else {
      channel.enabled = previousEnabled;
      props.setStatusAction(t('config.remoteIm.channelSaveFailed'));
    }
  } catch (error) {
    channel.enabled = previousEnabled;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  } finally {
    setChannelOperationBusy(channel.id, false);
    saving.value = false;
  }
}

async function toggleSelectedChannelEnabled(enabled: boolean) {
  if (!selectedChannel.value) return;
  await toggleChannelEnabled(selectedChannel.value, enabled);
}

async function toggleContactCommunication(item: RemoteImContact, enabled: boolean) {
  const oldSend = item.allowSend;
  const oldReceive = item.allowReceive;
  item.allowSend = enabled;
  item.allowReceive = enabled;
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_allow_send", {
      input: { contactId: item.id, allowSend: enabled },
    });
    await refreshContacts();
  } catch (error) {
    item.allowSend = oldSend;
    item.allowReceive = oldReceive;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
    await refreshContacts();
  }
}

async function toggleContactAllowSendFiles(item: RemoteImContact, enabled: boolean) {
  const oldValue = item.allowSendFiles;
  item.allowSendFiles = enabled;
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_allow_send_files", {
      input: { contactId: item.id, allowSendFiles: enabled },
    });
    await refreshContacts();
  } catch (error) {
    item.allowSendFiles = oldValue;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

async function saveContactActivation(
  item: RemoteImContact,
  patch?: Partial<
    Pick<
      RemoteImContact,
      | "activationMode"
      | "activationKeywords"
      | "responseStrategy"
    >
  >,
) {
  const oldMode = item.activationMode;
  const oldKeywords = [...item.activationKeywords];
  const oldResponseStrategy = normalizeResponseStrategy(item.responseStrategy);
  if (patch?.activationMode) item.activationMode = patch.activationMode;
  if (patch?.activationKeywords) item.activationKeywords = [...patch.activationKeywords];
  if (patch?.responseStrategy) item.responseStrategy = normalizeResponseStrategy(patch.responseStrategy);
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_activation", {
      input: {
        contactId: item.id,
        activationMode: item.activationMode,
        activationKeywords: item.activationKeywords,
        responseStrategy: normalizeResponseStrategy(item.responseStrategy),
      },
    });
    await refreshContacts();
  } catch (error) {
    item.activationMode = oldMode;
    item.activationKeywords = oldKeywords;
    item.responseStrategy = oldResponseStrategy;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

async function copyContactSettings(item: RemoteImContact) {
  // 模型挂在联系人的会话上，复制设置时一并带上，粘贴时写回目标联系人会话。
  const model = await fetchContactConversationModel(item.id);
  contactSettingsClipboard.value = {
    ...buildContactSettingsClipboard(item),
    preferredApiConfigId: model.preferredApiConfigId,
  };
  props.setStatusAction(t("config.remoteIm.contactSettingsCopied"));
}

function buildContactClipboardPatch(
  clipboard: ContactSettingsClipboard,
  target: RemoteImContact,
) {
  const isPrivate = isPrivateContact(target);
  return {
    boundAgentId: clipboard.boundAgentId,
    processingMode: clipboard.processingMode,
    activationMode: isPrivate ? "always" : clipboard.activationMode,
    activationKeywords: isPrivate ? [] : parseActivationKeywords(clipboard.activationKeywordsText),
    responseStrategy: isPrivate ? "always_reply" : clipboard.responseStrategy,
    allowReceive: clipboard.allowReceive,
    allowSend: clipboard.allowSend,
    allowSendFiles: clipboard.allowSendFiles,
  } as const;
}

async function pasteContactSettings(item: RemoteImContact) {
  const clipboard = contactSettingsClipboard.value;
  if (!clipboard) return;
  if (isContactOperationBusy(item.id)) return;
  const patch = buildContactClipboardPatch(clipboard, item);
  await withContactOperation(item.id, async () => {
    try {
      const updated = await invokeTauri<RemoteImContact>("remote_im_patch_contact_settings", {
        input: {
          contactId: item.id,
          agentId: patch.boundAgentId || null,
          processingMode: patch.processingMode,
          activationMode: patch.activationMode,
          activationKeywords: patch.activationKeywords,
          responseStrategy: patch.responseStrategy,
          allowReceive: patch.allowReceive,
          allowSend: patch.allowSend,
          allowSendFiles: patch.allowSendFiles,
        },
      });
      // 会话不存在（被删/归档）时不写模型，避免顺手重建会话。
      const targetModel = await fetchContactConversationModel(item.id);
      if (targetModel.conversationExists && targetModel.conversationId) {
        await invokeTauri("conversation.preferredModel.set", {
          input: {
            conversationId: targetModel.conversationId,
            preferredApiConfigId: clipboard.preferredApiConfigId || null,
          },
        });
      }
      contacts.value = contacts.value.map((contact) => (
        contact.id === updated.id ? updated : contact
      ));
      props.setStatusAction(t("config.remoteIm.contactSettingsPasted"));
    } catch (error) {
      props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
    }
  });
}

function onContactActivationModeChange(item: RemoteImContact, modeRaw: string) {
  const mode = normalizeActivationMode(modeRaw);
  void saveContactActivation(item, { activationMode: mode });
}

function contactActivationModeOptions(
  item: Pick<RemoteImContact, "remoteContactType">,
): Array<{ value: RemoteImContact["activationMode"]; label: string }> {
  if (isPrivateContact(item)) {
    return [{ value: "always", label: t("config.remoteIm.activateModeAlways") }];
  }
  return [
    { value: "always", label: t("config.remoteIm.activateModeAlways") },
    { value: "keyword", label: t("config.remoteIm.activateModeKeyword") },
    { value: "never", label: t("config.remoteIm.activateModeNever") },
  ];
}

async function selectContactActivationMode(
  item: RemoteImContact,
  mode: RemoteImContact["activationMode"],
) {
  const nextMode = normalizeActivationMode(mode);
  if (normalizeActivationMode(item.activationMode || "never") === nextMode) return;
  await withContactOperation(item.id, () => saveContactActivation(item, { activationMode: nextMode }));
}

function closeContactPillMenu() {
  contactPillMenu.value = null;
}

type ContactGroupMenuState = { left: number; top: number; groupId: string };
const contactGroupMenu = ref<ContactGroupMenuState | null>(null);

/** 分组头右键菜单：只对真实分组生效，未分组段没有菜单。 */
function openContactGroupMenu(event: MouseEvent, group: RemoteImContactGroup | null) {
  if (!group) return;
  const rect = (event.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  if (!rect) return;
  const menuWidth = 144;
  const left = Math.max(8, Math.min(window.innerWidth - menuWidth - 8, rect.left));
  const top = Math.min(window.innerHeight - 8, rect.bottom + 4);
  contactGroupMenu.value = { left, top, groupId: group.id };
}

function closeContactGroupMenu() {
  contactGroupMenu.value = null;
}

function contactGroupMenuTarget(): RemoteImContactGroup | null {
  const groupId = contactGroupMenu.value?.groupId;
  if (!groupId) return null;
  return currentChannelGroups.value.find((group) => group.id === groupId) ?? null;
}

function renameContactGroupFromMenu() {
  const group = contactGroupMenuTarget();
  closeContactGroupMenu();
  if (group) startContactGroupRename(group);
}

function deleteContactGroupFromMenu() {
  const group = contactGroupMenuTarget();
  closeContactGroupMenu();
  if (group) void deleteContactGroup(group);
}

function contactPillMenuWidthClass(kind: ContactPillMenuKind): string {
  if (kind === "processing") return "w-40";
  if (kind === "files") return "w-32";
  if (kind === "group") return "w-40";
  return "w-36";
}

function contactPillMenuOptions(
  item: RemoteImContact,
  kind: ContactPillMenuKind,
): ContactPillMenuOption[] {
  if (kind === "activation") {
    const current = isPrivateContact(item) ? "always" : normalizeActivationMode(item.activationMode || "never");
    return contactActivationModeOptions(item).map((option) => ({
      key: option.value,
      label: option.label,
      active: current === option.value,
      value: option.value,
    }));
  }
  if (kind === "processing") {
    const current = normalizeProcessingMode(item.processingMode);
    return contactProcessingModeOptions().map((option) => ({
      key: option.value,
      label: option.label,
      active: current === option.value,
      value: option.value,
    }));
  }
  if (kind === "response") {
    const current = normalizeResponseStrategy(item.responseStrategy);
    return contactResponseStrategyOptions().map((option) => ({
      key: option.value,
      label: option.label,
      active: current === option.value,
      value: option.value,
    }));
  }
  if (kind === "group") {
    return contactGroupOptionsForMove(item);
  }
  return contactSendFilesOptions().map((option) => ({
    key: String(option.value),
    label: option.label,
    active: !!item.allowSendFiles === option.value,
    value: option.value,
  }));
}

function openContactPillMenu(
  event: MouseEvent,
  item: RemoteImContact,
  kind: ContactPillMenuKind,
) {
  if (isContactOperationBusy(item.id)) return;
  const target = event.currentTarget as HTMLElement | null;
  const rect = target?.getBoundingClientRect();
  if (!rect) return;
  const options = contactPillMenuOptions(item, kind);
  const menuHeight = options.length * 32 + 10;
  const menuWidth = kind === "processing" || kind === "group" ? 160 : kind === "files" ? 128 : 144;
  const left = Math.max(8, Math.min(window.innerWidth - menuWidth - 8, rect.left));
  const top = Math.max(8, rect.top - menuHeight - 4);
  contactPillMenu.value = {
    contactId: item.id,
    kind,
    left,
    top,
    widthClass: contactPillMenuWidthClass(kind),
    options,
  };
}

async function selectContactPillMenuOption(option: ContactPillMenuOption) {
  const menu = contactPillMenu.value;
  if (!menu) return;
  const item = contacts.value.find((contact) => contact.id === menu.contactId);
  closeContactPillMenu();
  if (!item) return;
  if (menu.kind === "activation") {
    await selectContactActivationMode(item, option.value as RemoteImContact["activationMode"]);
  } else if (menu.kind === "processing") {
    await selectContactProcessingMode(item, option.value as "continuous" | "qa");
  } else if (menu.kind === "response") {
    await selectContactResponseStrategy(item, option.value as NonNullable<RemoteImContact["responseStrategy"]>);
  } else if (menu.kind === "group") {
    await moveContactsToGroup([item.id], String(option.value || ""));
  } else {
    await selectContactAllowSendFiles(item, option.value === true);
  }
}

function contactActivationModeIndex(item: RemoteImContact): number {
  const mode = normalizeActivationMode(item.activationMode || "never");
  const index = contactActivationModeOrder.indexOf(mode);
  return index >= 0 ? index : contactActivationModeOrder.length - 1;
}

function reorderContactAfterActivationMove(
  contactId: string,
  targetMode: RemoteImContact["activationMode"],
  direction: -1 | 1,
) {
  const moved = contacts.value.find((contact) => contact.id === contactId);
  if (!moved) return;
  const channelId = moved.channelId;
  const channelItems = contacts.value.filter((contact) => contact.channelId === channelId && contact.id !== contactId);
  const targetGroup = channelItems.filter(
    (contact) => normalizeActivationMode(contact.activationMode || "never") === targetMode,
  );
  const rebuiltChannelItems: RemoteImContact[] = [];
  for (const mode of contactActivationModeOrder) {
    const items = channelItems.filter((contact) => normalizeActivationMode(contact.activationMode || "never") === mode);
    if (mode === targetMode && direction === 1) {
      rebuiltChannelItems.push(moved);
    }
    rebuiltChannelItems.push(...items);
    if (mode === targetMode && direction === -1) {
      rebuiltChannelItems.push(moved);
    }
  }
  if (targetGroup.length === 0 && !rebuiltChannelItems.some((contact) => contact.id === contactId)) {
    rebuiltChannelItems.push(moved);
  }
  const next = [...contacts.value];
  let cursor = 0;
  for (let index = 0; index < next.length; index += 1) {
    if (next[index].channelId !== channelId) continue;
    const replacement = rebuiltChannelItems[cursor];
    if (replacement) {
      next[index] = replacement;
      cursor += 1;
    }
  }
  contacts.value = next;
}

async function moveContactActivationMode(item: RemoteImContact, direction: -1 | 1) {
  const currentIndex = contactActivationModeIndex(item);
  const nextIndex = (currentIndex + direction + contactActivationModeOrder.length) % contactActivationModeOrder.length;
  const nextMode = contactActivationModeOrder[nextIndex];
  if (!nextMode) return;
  await withContactOperation(item.id, async () => {
    await saveContactActivation(item, { activationMode: nextMode });
    reorderContactAfterActivationMove(item.id, nextMode, direction);
  });
}

async function onContactAgentChange(
  item: RemoteImContact,
  agentIdRaw: string,
) {
  const oldAgentId = item.boundAgentId;
  const nextAgentId = String(agentIdRaw || "").trim() || "";
  item.boundAgentId = nextAgentId || undefined;
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_agent_binding", {
      input: {
        contactId: item.id,
        agentId: nextAgentId || null,
      },
    });
    props.setStatusAction(t('config.remoteIm.contactContinueSession'));
    await refreshContacts();
  } catch (error) {
    item.boundAgentId = oldAgentId;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

async function onContactProcessingModeChange(
  item: RemoteImContact,
  processingModeRaw: string,
) {
  const oldValue = normalizeProcessingMode(item.processingMode);
  item.processingMode = normalizeProcessingMode(processingModeRaw);
  try {
    await invokeTauri<RemoteImContact>("remote_im_update_contact_processing_mode", {
      input: {
        contactId: item.id,
        processingMode: item.processingMode,
      },
    });
    await refreshContacts();
  } catch (error) {
    item.processingMode = oldValue;
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function contactProcessingModeOptions(): Array<{ value: "continuous" | "qa"; label: string }> {
  return [
    { value: "continuous", label: t("config.remoteIm.processingModeContinuous") },
    { value: "qa", label: t("config.remoteIm.processingModeQa") },
  ];
}

async function selectContactProcessingMode(
  item: RemoteImContact,
  mode: "continuous" | "qa",
) {
  const nextMode = normalizeProcessingMode(mode);
  if (normalizeProcessingMode(item.processingMode) === nextMode) return;
  await withContactOperation(item.id, () => onContactProcessingModeChange(item, nextMode));
}

async function cycleContactProcessingMode(item: RemoteImContact) {
  const current = normalizeProcessingMode(item.processingMode);
  const next = current === "qa" ? "continuous" : "qa";
  await withContactOperation(item.id, () => onContactProcessingModeChange(item, next));
}

function contactResponseStrategyOptions(): Array<{
  value: NonNullable<RemoteImContact["responseStrategy"]>;
  label: string;
}> {
  return [
    { value: "always_reply", label: t("config.remoteIm.responseStrategyAlways") },
    { value: "smart_judge", label: t("config.remoteIm.responseStrategySmart") },
  ];
}

function isPrivateContact(item: Pick<RemoteImContact, "remoteContactType"> | null | undefined): boolean {
  return String(item?.remoteContactType || "").trim().toLowerCase() === "private";
}

function contactResponseStrategy(item: RemoteImContact): NonNullable<RemoteImContact["responseStrategy"]> {
  return isPrivateContact(item) ? "always_reply" : normalizeResponseStrategy(item.responseStrategy);
}

function contactResponseStrategyLabel(item: RemoteImContact): string {
  return contactResponseStrategy(item) === "smart_judge"
    ? t("config.remoteIm.responseStrategySmart")
    : t("config.remoteIm.responseStrategyAlways");
}

async function selectContactResponseStrategy(
  item: RemoteImContact,
  strategy: NonNullable<RemoteImContact["responseStrategy"]>,
) {
  if (isPrivateContact(item)) return;
  const nextStrategy = normalizeResponseStrategy(strategy);
  if (normalizeResponseStrategy(item.responseStrategy) === nextStrategy) return;
  await withContactOperation(item.id, () => saveContactActivation(item, { responseStrategy: nextStrategy }));
}


function contactSendFilesOptions(): Array<{ value: boolean; label: string }> {
  return [
    { value: true, label: t('config.remoteIm.allowSendFiles') },
    { value: false, label: t('config.remoteIm.denySendFiles') },
  ];
}

function contactSendFilesLabel(item: RemoteImContact): string {
  return item.allowSendFiles ? t('config.remoteIm.allowSendFiles') : t('config.remoteIm.denySendFiles');
}

async function selectContactAllowSendFiles(item: RemoteImContact, enabled: boolean) {
  if (!!item.allowSendFiles === enabled) return;
  await withContactOperation(item.id, () => toggleContactAllowSendFiles(item, enabled));
}

async function cycleContactAllowSendFiles(item: RemoteImContact) {
  await withContactOperation(item.id, () => toggleContactAllowSendFiles(item, !item.allowSendFiles));
}

function onContactActivationKeywordsBlur(item: RemoteImContact) {
  const raw = contactKeywordDrafts.value[item.id] ?? item.activationKeywords.join(", ");
  const keywords = parseActivationKeywords(raw);
  contactKeywordDrafts.value[item.id] = keywords.join(", ");
  void saveContactActivation(item, { activationKeywords: keywords });
}

// ========== 联系人工作区配置（迁移会话工作目录最新卡片设计） ==========
const contactMainPath = computed(() => {
  if (!contactDraft.value?.shellWorkspaces) return "";
  const main = contactDraft.value.shellWorkspaces.find((ws) => ws.level === "main");
  if (main) return String(main.path || "").trim();
  return String(contactDraft.value.shellWorkspaces[0]?.path || "").trim();
});

const contactSecondaryPaths = computed(() => {
  if (!contactDraft.value?.shellWorkspaces) return [];
  const main = contactMainPath.value.toLowerCase();
  return contactDraft.value.shellWorkspaces
    .filter((ws) => ws.path.toLowerCase() !== main)
    .map((ws) => String(ws.path || "").trim())
    .filter(Boolean);
});

const contactUnifiedAccess = computed<ShellWorkspaceAccess>(() => {
  if (!contactDraft.value?.shellWorkspaces?.length) return "approval";
  const main = contactDraft.value.shellWorkspaces.find((ws) => ws.level === "main");
  const raw = String(main?.access || contactDraft.value.shellWorkspaces[0]?.access || "approval").trim();
  return (raw === "full_access" ? "full_access" : "approval") as ShellWorkspaceAccess;
});

const contactAvailableWorkspaces = computed(() => {
  const result: Array<{ id: string; name: string; path: string; access: ShellWorkspaceAccess }> = [];
  const seenPaths = new Set<string>();

  // 1. 联系人自己的工作区
  for (const ws of contactDraft.value?.shellWorkspaces || []) {
    const p = String(ws.path || "").trim();
    if (p && !seenPaths.has(p.toLowerCase())) {
      seenPaths.add(p.toLowerCase());
      result.push({
        id: ws.id,
        name: ws.name,
        path: ws.path,
        access: ws.access as ShellWorkspaceAccess,
      });
    }
  }

  // 2. 全局配置里的工作区供联系人挑选
  for (const ws of props.config.shellWorkspaces || []) {
    const p = String(ws.path || "").trim();
    if (p && !seenPaths.has(p.toLowerCase())) {
      seenPaths.add(p.toLowerCase());
      result.push({
        id: ws.id || p,
        name: ws.name || p.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || p,
        path: ws.path,
        access: (ws.access || "approval") as ShellWorkspaceAccess,
      });
    }
  }

  return result;
});

function onContactMainPathUpdate(path: string) {
  if (!contactDraft.value) return;
  const normalized = String(path || "").trim();
  if (!normalized) {
    const mainIdx = contactDraft.value.shellWorkspaces.findIndex((ws) => ws.level === "main");
    if (mainIdx >= 0) contactDraft.value.shellWorkspaces.splice(mainIdx, 1);
    return;
  }

  const existed = contactDraft.value.shellWorkspaces.find(
    (ws) => ws.path.toLowerCase() === normalized.toLowerCase(),
  );

  if (existed) {
    for (const ws of contactDraft.value.shellWorkspaces) {
      ws.level = ws.id === existed.id ? "main" : "secondary";
    }
  } else {
    for (const ws of contactDraft.value.shellWorkspaces) {
      if (ws.level === "main") {
        ws.level = "secondary";
      }
    }
    const name = normalized.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || normalized;
    contactDraft.value.shellWorkspaces.unshift({
      id: crypto.randomUUID(),
      name,
      path: normalized,
      level: "main",
      access: contactUnifiedAccess.value,
    });
  }
}

function onContactAccessUpdate(access: ShellWorkspaceAccess) {
  if (!contactDraft.value) return;
  for (const ws of contactDraft.value.shellWorkspaces) {
    ws.access = access;
  }
}

function onContactAddSecondary(path: string) {
  if (!contactDraft.value) return;
  const normalized = String(path || "").trim();
  if (!normalized) return;
  const existed = contactDraft.value.shellWorkspaces.some(
    (ws) => ws.path.toLowerCase() === normalized.toLowerCase(),
  );
  if (existed) return;
  const name = normalized.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || normalized;
  contactDraft.value.shellWorkspaces.push({
    id: crypto.randomUUID(),
    name,
    path: normalized,
    level: "secondary",
    access: contactUnifiedAccess.value,
  });
}

function onContactRemoveSecondary(path: string) {
  if (!contactDraft.value) return;
  const normalized = String(path || "").trim();
  const idx = contactDraft.value.shellWorkspaces.findIndex(
    (ws) => ws.path.toLowerCase() === normalized.toLowerCase(),
  );
  if (idx >= 0) {
    contactDraft.value.shellWorkspaces.splice(idx, 1);
  }
}

function resetContactDraft() {
  syncSelectedContactDraft();
}

async function saveContactDraft() {
  if (!selectedContact.value || !contactDraft.value || !contactDraftDirty.value || contactSaving.value) return;
  const item = selectedContact.value;
  const draft = contactDraft.value;
  contactSaving.value = true;
  try {
    const nextAgentId = String(draft.boundAgentId || "").trim();
    const currentAgentId = String(item.boundAgentId || "").trim();
    if (nextAgentId !== currentAgentId) {
      await onContactAgentChange(item, nextAgentId);
    }

    // 触发时机：卡片 pill 负责模式切换，展开区只负责点名关键词，故模式取联系人当前值。
    const nextKeywords = parseActivationKeywords(draft.activationKeywordsText);
    const currentKeywords = Array.isArray(item.activationKeywords) ? item.activationKeywords : [];
    const keywordsChanged = JSON.stringify(nextKeywords) !== JSON.stringify(currentKeywords);
    if (keywordsChanged) {
      await saveContactActivation(item, {
        activationMode: normalizeActivationMode(item.activationMode || "never"),
        activationKeywords: nextKeywords,
      });
    }

    // 保存联系人工作区配置
    try {
      await invokeTauri<RemoteImContact>("remote_im_update_contact_workspace", {
        input: {
          contactId: item.id,
          shellWorkspaces: (draft.shellWorkspaces || []).map((ws) => ({
            id: ws.id,
            name: ws.name,
            path: ws.path,
            level: ws.level,
            access: ws.access,
            builtIn: false,
          })),
        },
      });
    } catch (e) {
      console.error("[联系人工作区保存失败]", e);
      props.setStatusAction(t("status.saveConfigFailed", { err: String(e) }));
      return;
    }
    await refreshContacts();
    syncSelectedContactDraft();
  } finally {
    contactSaving.value = false;
  }
}

async function startWeixinLogin() {
  if (!selectedChannel.value || selectedChannel.value.platform !== "weixin_oc") return;
  weixinLoginBusy.value = true;
  try {
    const result = await invokeTauri<WeixinLoginStatus | {
      channelId: string;
      sessionKey: string;
      qrcode: string;
      qrcodeImgContent: string;
      status: string;
      message: string;
    }>("remote_im_weixin_oc_start_login", {
      input: {
        channelId: selectedChannel.value.id,
        forceRefresh: true,
      },
    });
    weixinLoginStates.value = {
      ...weixinLoginStates.value,
      [selectedChannel.value.id]: {
        channelId: result.channelId,
        connected: false,
        status: result.status,
        message: result.message,
        sessionKey: result.sessionKey,
        qrcode: result.qrcode,
        qrcodeImgContent: result.qrcodeImgContent,
        accountId: "",
        userId: "",
        baseUrl: "",
        lastError: "",
      },
    };
    if (weixinLoginPollTimer) clearInterval(weixinLoginPollTimer);
    weixinLoginPollTimer = setInterval(() => {
      void pollWeixinLoginStatus();
    }, 2500);
  } catch (error) {
    props.setStatusAction(t('config.remoteIm.weixinScanLoginFailed', { error: String(error) }));
  } finally {
    weixinLoginBusy.value = false;
  }
}

async function onWeixinLoginButtonClick() {
  if (weixinLoginBusy.value) return;
  if (channelDirty.value) {
    props.setStatusAction(t('config.remoteIm.savingWeixinConfig'));
    const saved = await saveChannels();
    if (!saved) {
      props.setStatusAction(t('config.remoteIm.saveWeixinFirst'));
      return;
    }
  }
  if (isWeixinLoggedIn.value) {
    await logoutWeixin();
  }
  await startWeixinLogin();
}

async function pollWeixinLoginStatus() {
  if (!selectedChannel.value || selectedChannel.value.platform !== "weixin_oc") return;
  const channelId = selectedChannel.value.id;
  try {
    const result = await invokeTauri<WeixinLoginStatus>("remote_im_weixin_oc_get_login_status", {
      input: { channelId },
    });
    weixinLoginStates.value = {
      ...weixinLoginStates.value,
      [channelId]: result,
    };
    if (result.connected || result.status === "expired") {
      if (weixinLoginPollTimer) {
        clearInterval(weixinLoginPollTimer);
        weixinLoginPollTimer = null;
      }
      if (result.connected) {
        await refreshChannelStatus();
        await refreshContacts();
      }
    }
  } catch (error) {
    const errMsg = t('config.remoteIm.weixinStatusQueryFailed', { error: String(error) });
    weixinLoginStates.value = {
      ...weixinLoginStates.value,
      [channelId]: {
        ...(weixinLoginStates.value[channelId] || {
          channelId,
          connected: false,
          status: "wait",
          message: "",
          sessionKey: "",
          qrcode: "",
          qrcodeImgContent: "",
          accountId: "",
          userId: "",
          baseUrl: "",
          lastError: "",
        }),
        message: errMsg,
        lastError: errMsg,
      },
    };
    props.setStatusAction(errMsg);
  }
}

async function syncWeixinContacts() {
  if (!selectedChannel.value || selectedChannel.value.platform !== "weixin_oc") return;
  try {
    const result = await invokeTauri<{ message: string }>("remote_im_weixin_oc_sync_contacts", {
      input: { channelId: selectedChannel.value.id },
    });
    props.setStatusAction(result.message);
    await refreshContacts();
  } catch (error) {
    props.setStatusAction(t('config.remoteIm.weixinContactSyncFailed', { error: String(error) }));
  }
}

async function logoutWeixin() {
  if (!selectedChannel.value || selectedChannel.value.platform !== "weixin_oc") return;
  try {
    await invokeTauri<boolean>("remote_im_weixin_oc_logout", {
      input: { channelId: selectedChannel.value.id },
    });
    weixinLoginStates.value = {
      ...weixinLoginStates.value,
      [selectedChannel.value.id]: {
        channelId: selectedChannel.value.id,
        connected: false,
        status: "logged_out",
        message: t('config.remoteIm.loggedOut'),
      },
    };
    await refreshChannelStatus();
    props.setStatusAction(t('config.remoteIm.weixinLoggedOut'));
  } catch (error) {
    props.setStatusAction(t('config.remoteIm.weixinLogoutFailed', { error: String(error) }));
  }
}

async function refreshContacts() {
  contactsLoading.value = true;
  contactsError.value = "";
  try {
    contacts.value = await invokeTauri<RemoteImContact[]>("remote_im_list_contacts");
    for (const item of contacts.value) {
      item.activationMode = normalizeActivationMode(item.activationMode || "never");
      item.activationKeywords = Array.isArray(item.activationKeywords) ? item.activationKeywords : [];
      item.processingMode = normalizeProcessingMode(item.processingMode);
      item.responseStrategy = normalizeResponseStrategy(item.responseStrategy);
      item.allowSendFiles = !!item.allowSendFiles;
      contactKeywordDrafts.value[item.id] = item.activationKeywords.join(", ");
    }
    if (selectedContactId.value && !contacts.value.some((item) => item.id === selectedContactId.value)) {
      collapseContactSettings();
    }
    if (contactLogsContactId.value && !contacts.value.some((item) => item.id === contactLogsContactId.value)) {
      contactLogsModalOpen.value = false;
      contactLogsContactId.value = "";
      contactLogs.value = [];
    }
  } catch (error) {
    contactsError.value = String(error);
  } finally {
    contactsLoading.value = false;
  }
  await refreshContactGroups();
}

// ==================== 联系人自定义分组 ====================

async function refreshContactGroups() {
  const channelId = selectedChannelId.value;
  if (!channelId) {
    contactGroups.value = [];
    return;
  }
  try {
    const groups = await invokeTauri<RemoteImContactGroup[]>("remote_im_list_contact_groups", {
      channelId,
    });
    if (selectedChannelId.value !== channelId) return;
    contactGroups.value = groups;
  } catch (error) {
    if (selectedChannelId.value !== channelId) return;
    contactGroups.value = [];
    contactGroupError.value = String(error);
  }
}

function startContactGroupCreate() {
  contactGroupEditMode.value = "create";
  contactGroupEditTargetId.value = "";
  contactGroupNameDraft.value = "";
  contactGroupError.value = "";
}

function startContactGroupRename(group: RemoteImContactGroup) {
  contactGroupEditMode.value = "rename";
  contactGroupEditTargetId.value = group.id;
  contactGroupNameDraft.value = group.name;
  contactGroupError.value = "";
}

/** 当前分组是否正处于就地重命名状态：仅对真实分组（非「未分组」段）生效。 */
function isContactGroupRenaming(section: ContactListSection): boolean {
  return contactGroupEditMode.value === "rename"
    && !!section.group
    && contactGroupEditTargetId.value === section.group.id;
}

/** 就地重命名输入框挂载后自动聚焦并全选，省去再次点击。 */
function focusContactGroupRenameInput(el: unknown) {
  const node = el as HTMLInputElement | null;
  if (!node || typeof node.focus !== "function") return;
  nextTick(() => {
    node.focus();
    node.select?.();
  });
}

function cancelContactGroupEdit() {
  contactGroupEditMode.value = null;
  contactGroupEditTargetId.value = "";
  contactGroupNameDraft.value = "";
  contactGroupError.value = "";
}

async function submitContactGroupEdit() {
  const mode = contactGroupEditMode.value;
  const channelId = selectedChannelId.value;
  if (!mode || contactGroupBusy.value || !channelId) return;
  const name = contactGroupNameDraft.value.trim();
  if (!name) {
    contactGroupError.value = t("config.remoteIm.contactGroupNameRequired");
    return;
  }
  contactGroupBusy.value = true;
  contactGroupError.value = "";
  try {
    if (mode === "create") {
      await invokeTauri<RemoteImContactGroup>("remote_im_create_contact_group", {
        input: { channelId, name },
      });
      await refreshContactGroups();
    } else {
      await invokeTauri("remote_im_rename_contact_group", {
        input: { groupId: contactGroupEditTargetId.value, name },
      });
      await refreshContactGroups();
    }
    cancelContactGroupEdit();
    props.setStatusAction(t("config.remoteIm.contactGroupSaved"));
  } catch (error) {
    contactGroupError.value = String(error);
  } finally {
    contactGroupBusy.value = false;
  }
}

async function deleteContactGroup(group: RemoteImContactGroup) {
  const channelId = selectedChannelId.value;
  if (!channelId || contactGroupBusy.value) return;
  const confirmed = window.confirm(
    t("config.remoteIm.contactGroupDeleteConfirm", { name: group.name }),
  );
  if (!confirmed) return;
  contactGroupBusy.value = true;
  contactGroupError.value = "";
  try {
    const result = await invokeTauri<{ detachedContactCount: number }>(
      "remote_im_delete_contact_group",
      { input: { groupId: group.id } },
    );
    await refreshContacts();
    props.setStatusAction(
      t("config.remoteIm.contactGroupDeleted", { count: result.detachedContactCount }),
    );
  } catch (error) {
    contactGroupError.value = String(error);
  } finally {
    contactGroupBusy.value = false;
  }
}

async function moveContactsToGroup(contactIds: string[], groupId: string) {
  const channelId = selectedChannelId.value;
  if (!channelId || contactIds.length === 0) return;
  try {
    await invokeTauri("remote_im_set_contact_group", {
      input: { channelId, contactIds, groupId: groupId || null },
    });
    await refreshContacts();
    props.setStatusAction(t("config.remoteIm.moveToGroupDone"));
  } catch (error) {
    props.setStatusAction(t("status.saveConfigFailed", { err: String(error) }));
  }
}

function contactGroupOptionsForMove(contact: RemoteImContact): ContactPillMenuOption[] {
  const currentGroupId = contactGroupIdOf(contact);
  return [
    {
      key: "ungrouped",
      label: t("config.remoteIm.contactGroupUngrouped"),
      active: !currentGroupId,
      value: "",
    },
    ...currentChannelGroups.value.map((group) => ({
      key: group.id,
      label: group.name,
      active: currentGroupId === group.id,
      value: group.id,
    })),
  ];
}

function contactDisplayName(item: RemoteImContact): string {
  const remark = String(item.remarkName || "").trim();
  if (remark) return remark;
  const remoteName = String(item.remoteContactName || "").trim();
  if (remoteName) return remoteName;
  return item.remoteContactId;
}

function contactSafeDisplayName(item: RemoteImContact): string {
  if (item.platform === "weixin_oc") {
    const remark = String(item.remarkName || "").trim();
    if (remark) return remark;
    const remoteName = String(item.remoteContactName || "").trim();
    if (remoteName && !remoteName.includes("@")) return remoteName;
    return t('config.remoteIm.weixinContact');
  }
  return contactDisplayName(item);
}

function contactSecondaryText(item: RemoteImContact): string {
  if (item.platform === "weixin_oc") {
    return item.remoteContactType === "group" ? t('config.remoteIm.weixinGroupContact') : t('config.remoteIm.weixinPrivateContact');
  }
  return item.remoteContactId;
}

async function deleteContact(item: RemoteImContact) {
  if (contactDeleting.value) return;
  const displayName = contactSafeDisplayName(item);
  const confirmed = window.confirm(t('config.remoteIm.deleteContactConfirm', { name: displayName }));
  if (!confirmed) return;
  contactDeleting.value = true;
  try {
    const removed = await invokeTauri<boolean>("remote_im_delete_contact", {
      input: { contactId: item.id },
    });
    if (!removed) {
      props.setStatusAction(t('config.remoteIm.deleteContactNotFound', { name: displayName }));
      return;
    }
    if (selectedContactId.value === item.id) {
      collapseContactSettings();
    }
    await refreshContacts();
    props.setStatusAction(t('config.remoteIm.deleteContactSuccess', { name: displayName }));
  } catch (error) {
    props.setStatusAction(t('config.remoteIm.deleteContactFailed', { error: String(error) }));
  } finally {
    contactDeleting.value = false;
  }
}

function contactAgentLabel(item: RemoteImContact): string {
  const agentId = String(item.boundAgentId || "").trim() || "support";
  const persona = (props.personas || []).find((agent) => String(agent.id || "").trim() === agentId);
  return String(persona?.name || "").trim() || agentId;
}

function contactProcessingModeLabel(item: RemoteImContact): string {
  return normalizeProcessingMode(item.processingMode) === "qa"
    ? t("config.remoteIm.processingModeQa")
    : t("config.remoteIm.processingModeContinuous");
}

function contactAvatarUrl(item: RemoteImContact): string {
  return String(item.avatarUrl || "").trim();
}

function contactProcessingModeBadgeClass(item: RemoteImContact): string {
  return normalizeProcessingMode(item.processingMode) === "qa" ? "badge-secondary" : "badge-info";
}

function processingModeHintText(item: RemoteImContact): string {
  return normalizeProcessingMode(item.processingMode) === "qa"
    ? t("config.remoteIm.processingModeQaHint")
    : t("config.remoteIm.processingModeContinuousHint");
}

function contactActivationModeLabel(item: RemoteImContact): string {
  if (isPrivateContact(item)) return t("config.remoteIm.activateModeAlways");
  const mode = normalizeActivationMode(item.activationMode || "never");
  if (mode === "always") return t("config.remoteIm.activateModeAlways");
  if (mode === "keyword") return t("config.remoteIm.activateModeKeyword");
  return t("config.remoteIm.activateModeNever");
}

function contactActivationBadgeClass(item: RemoteImContact): string {
  if (isPrivateContact(item)) return "badge-success";
  const mode = normalizeActivationMode(item.activationMode || "never");
  if (mode === "always") return "badge-success";
  if (mode === "keyword") return "badge-primary";
  return "badge-ghost";
}

function contactKeywordModeMissingKeywords(item: RemoteImContact): boolean {
  if (isPrivateContact(item)) return false;
  if (normalizeActivationMode(item.activationMode || "never") !== "keyword") return false;
  return !Array.isArray(item.activationKeywords)
    || item.activationKeywords.every((keyword) => !String(keyword || "").trim());
}

function contactActivationHintText(item: RemoteImContact): string {
  if (isPrivateContact(item)) return t("config.remoteIm.activateModeAlwaysHint");
  const mode = normalizeActivationMode(item.activationMode);
  if (mode === "always") return t("config.remoteIm.activateModeAlwaysHint");
  if (mode === "keyword") return t("config.remoteIm.activateModeKeywordHint");
  return t("config.remoteIm.activateModeNeverHint");
}

function contactResponseStrategyHintText(item: RemoteImContact): string {
  return contactResponseStrategy(item) === "smart_judge"
    ? t("config.remoteIm.responseStrategySmartHint")
    : t("config.remoteIm.responseStrategyAlwaysHint");
}

function contactNeedsQuickModel(item: RemoteImContact): boolean {
  return normalizeResponseStrategy(item.responseStrategy) === "smart_judge";
}

function platformLabelText(platform: string): string {
  const value = String(platform || "").trim().toLowerCase();
  if (value === "weixin_oc") return t("config.remoteIm.platformOptions.weixinOc");
  if (value === "feishu") return t("config.remoteIm.platformOptions.feishu");
  if (value === "dingtalk") return t("config.remoteIm.platformOptions.dingtalk");
  return t("config.remoteIm.platformOptions.onebotV11");
}

async function refreshChannelStatus() {
  if (!selectedChannel.value) return;
  const channelId = selectedChannel.value.id;
  try {
    const status = await invokeTauri<ChannelConnectionStatus>("remote_im_get_channel_status", {
      channelId,
    });
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = status;
    }
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: status,
    };
  } catch (error) {
    console.error("[远程IM] refreshChannelStatus failed:", error);
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = null;
    }
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: null,
    };
  }
}

async function refreshChannelStatusById(channelId: string) {
  try {
    const status = await invokeTauri<ChannelConnectionStatus>("remote_im_get_channel_status", { channelId });
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: status,
    };
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = status;
    }
  } catch {
    channelRuntimeStates.value = {
      ...channelRuntimeStates.value,
      [channelId]: null,
    };
    if (selectedChannel.value?.id === channelId) {
      channelStatus.value = null;
    }
  }
}

async function refreshAllChannelStatuses() {
  const jobs = channels.value
    .filter((item) => item.platform === "onebot_v11" || item.platform === "dingtalk" || item.platform === "weixin_oc")
    .map((item) => refreshChannelStatusById(item.id));
  await Promise.all(jobs);
}

function onebotStatusText(status: ChannelConnectionStatus | null): string {
  if (!status) return t("config.remoteIm.serverNotStarted");
  if (status.connected) return `${t("config.remoteIm.connected")} (${status.peerAddr})`;
  if (status.statusText === "binding_retry") return status.lastError || t('config.remoteIm.portOccupiedRetry');
  if (status.statusText === "bind_failed") return status.lastError || t('config.remoteIm.portBindFailed');
  if (status.statusText === "disabled") return t('config.remoteIm.channelDisabledBadge');
  if (status.statusText === "binding") return t('config.remoteIm.bindingPort');
  if (status.listenAddr) return t('config.remoteIm.onebotListening', { addr: status.listenAddr });
  return t("config.remoteIm.serverNotStarted");
}

function channelStatusPreview(channel: RemoteImChannelConfig): string {
  if (channel.platform === "weixin_oc") {
    const status = channelRuntimeStates.value[channel.id];
    if (!status) return t('config.remoteIm.statusUninitialized');
    if (status.connected) return t('config.remoteIm.statusWeixinConnected');
    if (!channel.enabled) {
      if (status.statusText === "confirmed" || status.statusText === "logged_in") {
        return t('config.remoteIm.statusWeixinLoggedInNotEnabled');
      }
      if (status.accountId) return t('config.remoteIm.statusWeixinLoggedInNotEnabled');
      if (status.statusText === "need_login") return t('config.remoteIm.statusWeixinNotEnabledScan');
      return t("config.remoteIm.disabledState");
    }
    if (status.statusText === "need_login") return t('config.remoteIm.statusWeixinWaitingScan');
    if (status.statusText === "confirmed" || status.statusText === "logged_in") {
      return t('config.remoteIm.statusWeixinLoggedIn');
    }
    if (status.statusText === "wait" || status.statusText === "scaned") return t('config.remoteIm.statusWeixinWaitingConfirm');
    return status.lastError || status.statusText || t('config.remoteIm.statusWeixinNotConnected');
  }
  if (channel.platform === "dingtalk") {
    const status = channelRuntimeStates.value[channel.id];
    if (!channel.enabled) return t("config.remoteIm.disabledState");
    if (!status) return t("config.remoteIm.dingtalkConnectingState");
    if (status.connected) return t("config.remoteIm.connected");
    return t("config.remoteIm.dingtalkConnectingState");
  }
  if (channel.platform === "feishu") {
    return channel.enabled
      ? t("config.remoteIm.feishuSendOnlyState")
      : t("config.remoteIm.disabledState");
  }
  if (channel.platform !== "onebot_v11") {
    return channel.enabled ? t("config.remoteIm.enabledState") : t("config.remoteIm.disabledState");
  }
  const status = channelRuntimeStates.value[channel.id];
  if (!status) {
    return channel.enabled ? t("config.remoteIm.serverNotStarted") : t("config.remoteIm.disabledState");
  }
  if (status.connected) {
    return t("config.remoteIm.connected");
  }
  if (status.statusText === "binding_retry") return status.lastError || t('config.remoteIm.portOccupiedRetry');
  if (status.statusText === "bind_failed") return status.lastError || t('config.remoteIm.portBindFailed');
  if (status.statusText === "disabled") return t('config.remoteIm.channelDisabledBadge');
  if (status.statusText === "binding") return t('config.remoteIm.bindingPort');
  return status.listenAddr ? t('config.remoteIm.onebotListening', { addr: status.listenAddr }) : t("config.remoteIm.serverNotStarted");
}

function channelListStatusBadgeText(channel: RemoteImChannelConfig): string {
  if (!channel.enabled) return t("config.remoteIm.disabledState");
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk" || channel.platform === "weixin_oc") {
    const status = channelRuntimeStates.value[channel.id];
    if (status?.connected) return t("config.remoteIm.connected");
    if (channel.platform === "onebot_v11" && status?.statusText === "binding_retry") return t('config.remoteIm.statusRetryBinding');
    if (channel.platform === "onebot_v11" && status?.statusText === "bind_failed") return t('config.remoteIm.statusBindFailed');
    if (channel.platform === "onebot_v11" && status?.statusText === "disabled") return t('config.remoteIm.statusDisabled');
    if (channel.platform === "onebot_v11" && status?.statusText === "binding") return t('config.remoteIm.statusBinding');
    if (channel.platform === "onebot_v11" && status?.listenAddr) return t('config.remoteIm.statusWaitingConnection');
    if (channel.platform === "weixin_oc" && status?.statusText === "need_login") return t('config.remoteIm.statusWaitingLogin');
    return t("config.remoteIm.enabledState");
  }
  return t("config.remoteIm.enabledState");
}

function channelListStatusBadgeClass(channel: RemoteImChannelConfig): string {
  if (!channel.enabled) return "badge-ghost";
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk" || channel.platform === "weixin_oc") {
    const status = channelRuntimeStates.value[channel.id];
    if (channel.platform === "onebot_v11" && status?.statusText === "bind_failed") return "badge-error";
    if (channel.platform === "onebot_v11" && status?.statusText === "disabled") return "badge-ghost";
    if (channel.platform === "weixin_oc" && status?.statusText === "need_login") return "badge-warning";
    return status?.connected ? "badge-success" : "badge-warning";
  }
  return "badge-success";
}

async function refreshChannelLogs() {
  if (!selectedChannel.value) return;
  channelLogsLoading.value = true;
  try {
    channelLogs.value = await invokeTauri<ChannelLogEntry[]>("remote_im_get_channel_logs", {
      channelId: selectedChannel.value.id,
    });
  } catch {
    channelLogs.value = [];
  } finally {
    channelLogsLoading.value = false;
  }
}

async function refreshContactLogs() {
  if (!contactLogsContactId.value) return;
  contactLogsLoading.value = true;
  try {
    contactLogs.value = await invokeTauri<ChannelLogEntry[]>("remote_im_get_contact_logs", {
      input: { contactId: contactLogsContactId.value },
    });
  } catch {
    contactLogs.value = [];
  } finally {
    contactLogsLoading.value = false;
  }
}

function openChannelLogsModal() {
  if (!selectedChannel.value) return;
  channelLogsModalOpen.value = true;
  void refreshChannelLogs();
}

function openChannelLogsModalForChannel(channelId: string) {
  selectedChannelId.value = channelId;
  channelLogsModalOpen.value = true;
  void refreshChannelLogs();
}

function closeChannelLogsModal() {
  channelLogsModalOpen.value = false;
}

function openContactLogsModal(contactId: string) {
  contactLogsContactId.value = contactId;
  contactLogsModalOpen.value = true;
  void refreshContactLogs();
}

function closeContactLogsModal() {
  contactLogsModalOpen.value = false;
}

function openChannelConfigModal(channelId: string) {
  selectedChannelId.value = channelId;
  channelConfigModalOpen.value = true;
}

function closeChannelConfigModal() {
  channelConfigModalOpen.value = false;
}

function isContactExpanded(contactId: string): boolean {
  return selectedContactId.value === contactId;
}

/** 展开联系人设置：同时只展开一个，草稿跟随展开的联系人。 */
function toggleContactSettings(contactId: string) {
  if (selectedContactId.value === contactId) {
    collapseContactSettings();
    return;
  }
  selectedContactId.value = contactId;
  syncSelectedContactDraft();
  void refreshContactConversationModel(contactId);
}

function collapseContactSettings() {
  selectedContactId.value = "";
  syncSelectedContactDraft();
}

watch(
  channels,
  (list) => {
    if (list.length > 0 && !list.some((ch) => ch.id === selectedChannelId.value)) {
      selectedChannelId.value = list[0].id;
    }
    for (const item of list) {
      if (!(item.id in credentialDrafts.value)) {
        credentialDrafts.value[item.id] = JSON.stringify(item.credentials || {}, null, 2);
      }
    }
  },
  { immediate: true },
);

watch(selectedChannelId, () => {
  if (selectedChannel.value) {
    credentialDrafts.value[selectedChannel.value.id] = JSON.stringify(
      selectedChannel.value.credentials || {}, null, 2,
    );
    if (selectedChannel.value.platform === "onebot_v11") {
      loadNapcatCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "dingtalk") {
      loadDingtalkCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "weixin_oc") {
      loadWeixinCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else {
      channelStatus.value = null;
    }
    if (channelLogsModalOpen.value) {
      void refreshChannelLogs();
    } else {
      channelLogs.value = [];
    }
    // 切渠道即收起联系人设置，避免残留上一个渠道的联系人草稿。
    collapseContactSettings();
  }
  lastSavedChannelSnapshot.value = channelSnapshot.value;
});

watch(napcatCredentials, () => {
  if (suppressCredentialSync.value) return;
  if (selectedChannel.value && selectedChannel.value.platform === "onebot_v11") {
    selectedChannel.value.credentials = {
      wsHost: napcatCredentials.value.wsHost || "0.0.0.0",
      wsPort: napcatCredentials.value.wsPort || 6199,
      wsToken: napcatCredentials.value.wsToken || "",
    };
  }
}, { deep: true });

watch(dingtalkCredentials, () => {
  if (suppressCredentialSync.value) return;
  if (selectedChannel.value && selectedChannel.value.platform === "dingtalk") {
    const current = selectedChannel.value.credentials || {};
    selectedChannel.value.credentials = {
      ...current,
      clientId: dingtalkCredentials.value.clientId || "",
      clientSecret: dingtalkCredentials.value.clientSecret || "",
    };
  }
}, { deep: true });

watch(weixinCredentials, () => {
  if (suppressCredentialSync.value) return;
  if (selectedChannel.value && selectedChannel.value.platform === "weixin_oc") {
    const current = selectedChannel.value.credentials || {};
    selectedChannel.value.credentials = {
      ...current,
      baseUrl: weixinCredentials.value.baseUrl || "https://ilinkai.weixin.qq.com",
      botType: WEIXIN_OC_BOT_TYPE,
      qrPollInterval: WEIXIN_OC_QR_POLL_INTERVAL,
      longPollTimeoutMs: WEIXIN_OC_LONG_POLL_TIMEOUT_MS,
      apiTimeoutMs: WEIXIN_OC_API_TIMEOUT_MS,
    };
  }
}, { deep: true });

onMounted(() => {
  if (channels.value.length > 0 && !selectedChannelId.value) {
    selectedChannelId.value = channels.value[0].id;
  }
  // channels watcher (immediate: true) 已在 selectedChannelId watcher 注册前
  // 就同步设置了 selectedChannelId，导致 selectedChannelId watcher 不会触发。
  // 这里需要手动执行初始化操作：加载 napcatCredentials 和刷新连接状态/日志。
  if (selectedChannel.value) {
    credentialDrafts.value[selectedChannel.value.id] = JSON.stringify(
      selectedChannel.value.credentials || {}, null, 2,
    );
    if (selectedChannel.value.platform === "onebot_v11") {
      loadNapcatCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "dingtalk") {
      loadDingtalkCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    } else if (selectedChannel.value.platform === "weixin_oc") {
      loadWeixinCredentials(selectedChannel.value);
      channelStatus.value = channelRuntimeStates.value[selectedChannel.value.id] ?? null;
      void refreshChannelStatus();
    }
  }
  void refreshAllChannelStatuses();
  channelStatusTimer = setInterval(() => {
    void refreshAllChannelStatuses();
    if (channelLogsModalOpen.value) {
      void refreshChannelLogs();
    }
  }, 3000);
  lastSavedChannelSnapshot.value = channelSnapshot.value;
  void refreshContacts();
});

onUnmounted(() => {
  if (channelStatusTimer) {
    clearInterval(channelStatusTimer);
    channelStatusTimer = null;
  }
  if (weixinLoginPollTimer) {
    clearInterval(weixinLoginPollTimer);
    weixinLoginPollTimer = null;
  }
});
// ==================== 联系人批量设置 ====================

const batchWizardFieldKeys: BatchWizardFieldKey[] = [
  "agent",
  "model",
  "processingMode",
  "activation",
  "responseStrategy",
  "communication",
  "workspaceAccess",
  "group",
];

/** 需要在联系人字段上批量写入的设置项（「移动到分组」走分组命令，「首选模型」走会话模型命令）。 */
const batchWizardPatchFieldKeys: BatchWizardFieldKey[] = batchWizardFieldKeys.filter((key) => key !== "group" && key !== "model");

function personaNameById(agentId: string): string {
  const id = String(agentId || "").trim() || "support";
  const persona = (props.personas || []).find((agent) => String(agent.id || "").trim() === id);
  return String(persona?.name || "").trim() || id;
}

function apiConfigLabelById(apiConfigId: string): string {
  const resolvedId = resolveAvailableOrExpertModelId(apiConfigId);
  if (!resolvedId) return t("config.remoteIm.preferredModelUnset");
  const apiConfig = (props.config.apiConfigs || []).find((item) => String(item.id || "").trim() === resolvedId);
  return String(apiConfig?.name || "").trim() || resolvedId;
}

function workspaceAccessLabel(access: string): string {
  if (access === "full_access") return t("config.tools.workspaceAccessFullAccess");
  if (access === "approval") return t("config.tools.workspaceAccessApproval");
  return t("config.tools.workspaceAccessReadOnly");
}

function batchWizardEnabledLabel(enabled: boolean): string {
  return enabled ? t("config.remoteIm.batchToggleOn") : t("config.remoteIm.batchToggleOff");
}

/** 向导候选集为当前渠道全部联系人，按分组分段呈现，不随主界面切换分组而变。 */
const batchWizardScopedContacts = computed(() => {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return [];
  const scope = currentChannelContacts.value;
  const q = wizard.contactSearchQuery.trim().toLowerCase();
  if (!q) return scope;
  return scope.filter((item) => {
    const name = contactSafeDisplayName(item).toLowerCase();
    const agent = contactAgentLabel(item).toLowerCase();
    return name.includes(q) || agent.includes(q);
  });
});

/** 第二步的分组分段：空分组不占位，未分组固定排在最后。 */
const batchWizardContactSections = computed<BatchWizardContactSection[]>(() => {
  const scope = batchWizardScopedContacts.value;
  const sections: BatchWizardContactSection[] = [];
  for (const group of currentChannelGroups.value) {
    const contacts = scope.filter((item) => contactGroupIdOf(item) === group.id);
    if (contacts.length > 0) sections.push({ key: group.id, name: group.name, contacts });
  }
  const ungrouped = scope.filter((item) => !contactGroupIdOf(item));
  if (ungrouped.length > 0) {
    sections.push({
      key: "ungrouped",
      name: t("config.remoteIm.contactGroupUngrouped"),
      contacts: ungrouped,
    });
  }
  return sections;
});

const batchWizardSelectedContacts = computed(() => {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return [];
  return currentChannelContacts.value.filter((item) => wizard.selectedContactIds.includes(item.id));
});

function batchWizardHasFieldSelection(wizard: BatchSettingsWizardState): boolean {
  return batchWizardFieldKeys.some((key) => wizard.fields[key]);
}

function batchWizardHasPatchFieldSelection(wizard: BatchSettingsWizardState): boolean {
  return batchWizardPatchFieldKeys.some((key) => wizard.fields[key]);
}

function batchWizardGroupSelectedCount(contacts: RemoteImContact[]): number {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return 0;
  return contacts.filter((contact) => wizard.selectedContactIds.includes(contact.id)).length;
}

function batchWizardGroupSelectionState(contacts: RemoteImContact[]): "all" | "none" | "partial" {
  const selected = batchWizardGroupSelectedCount(contacts);
  if (selected === 0) return "none";
  return selected === contacts.length ? "all" : "partial";
}

function batchWizardToggleContactGroup(contacts: RemoteImContact[], selected: boolean) {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return;
  const selectedIds = new Set(wizard.selectedContactIds);
  for (const contact of contacts) {
    if (selected) selectedIds.add(contact.id);
    else selectedIds.delete(contact.id);
  }
  wizard.selectedContactIds = Array.from(selectedIds);
}

const batchWizardCollapsedGroups = ref<Set<string>>(new Set());

function batchWizardIsGroupExpanded(groupKey: string): boolean {
  return !batchWizardCollapsedGroups.value.has(groupKey);
}

function batchWizardToggleGroupExpand(groupKey: string) {
  const next = new Set(batchWizardCollapsedGroups.value);
  if (next.has(groupKey)) {
    next.delete(groupKey);
  } else {
    next.add(groupKey);
  }
  batchWizardCollapsedGroups.value = next;
}

function batchWizardStateBoxClasses(state: "all" | "none" | "partial") {
  if (state === "all") return "border-primary bg-primary text-primary-content shadow-xs";
  if (state === "partial") return "border-primary/50 bg-primary/20 text-primary";
  return "border-base-content/25 bg-transparent text-transparent";
}

/** 按单联系人设置同一口径推导「将会发生什么」，未变化项不列出。 */
function batchWizardPreviewFor(contact: RemoteImContact): BatchWizardChange[] {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return [];
  const changes: BatchWizardChange[] = [];
  const isPrivate = isPrivateContact(contact);

  if (wizard.fields.agent) {
    const fromAgent = contactAgentLabel(contact);
    const toAgent = personaNameById(wizard.agentId);
    if (fromAgent !== toAgent) {
      changes.push({ label: t("config.remoteIm.processingAgent"), from: fromAgent, to: toAgent });
    }
  }

  if (wizard.fields.model) {
    const model = wizard.contactModels[contact.id];
    if (model?.conversationExists) {
      const fromModel = apiConfigLabelById(model.preferredApiConfigId);
      const toModel = apiConfigLabelById(wizard.preferredApiConfigId);
      if (fromModel !== toModel) {
        changes.push({ label: t("config.remoteIm.preferredModel"), from: fromModel, to: toModel });
      }
    }
  }

  if (wizard.fields.processingMode) {
    const from = contactProcessingModeLabel(contact);
    const to = wizard.processingMode === "qa"
      ? t("config.remoteIm.processingModeQa")
      : t("config.remoteIm.processingModeContinuous");
    if (from !== to) {
      changes.push({ label: t("config.remoteIm.processingMode"), from, to });
    }
  }

  if (wizard.fields.activation) {
    const from = contactActivationModeLabel(contact);
    const to = isPrivate
      ? t("config.remoteIm.activateModeAlways")
      : wizard.activationMode === "always"
        ? t("config.remoteIm.activateModeAlways")
        : wizard.activationMode === "keyword"
          ? t("config.remoteIm.activateModeKeyword")
          : t("config.remoteIm.activateModeNever");
    if (from !== to) {
      changes.push({ label: t("config.remoteIm.activateMode"), from, to });
    }
    if (!isPrivate && wizard.activationMode === "keyword") {
      const fromKeywords = (contact.activationKeywords || []).join(", ");
      const toKeywords = parseActivationKeywords(wizard.activationKeywordsText).join(", ");
      if (fromKeywords !== toKeywords) {
        changes.push({
          label: t("config.remoteIm.activateKeywords"),
          from: fromKeywords || t("config.remoteIm.keywordEmpty"),
          to: toKeywords || t("config.remoteIm.keywordEmpty"),
        });
      }
    }
  }

  if (wizard.fields.responseStrategy) {
    const from = contactResponseStrategyLabel(contact);
    const to = isPrivate || wizard.responseStrategy === "always_reply"
      ? t("config.remoteIm.responseStrategyAlways")
      : t("config.remoteIm.responseStrategySmart");
    if (from !== to) {
      changes.push({ label: t("config.remoteIm.responseStrategy"), from, to });
    }
  }

  if (wizard.fields.communication) {
    const fromCommunication = !!(contact.allowReceive || contact.allowSend);
    if (fromCommunication !== wizard.allowCommunication) {
      changes.push({
        label: t("config.remoteIm.batchCommunicationLabel"),
        from: batchWizardEnabledLabel(fromCommunication),
        to: batchWizardEnabledLabel(wizard.allowCommunication),
      });
    }
    if (!!contact.allowSendFiles !== wizard.allowSendFiles) {
      changes.push({
        label: t("config.remoteIm.allowSendFiles"),
        from: batchWizardEnabledLabel(!!contact.allowSendFiles),
        to: batchWizardEnabledLabel(wizard.allowSendFiles),
      });
    }
  }

  if (wizard.fields.workspaceAccess) {
    const workspaces = contact.shellWorkspaces || [];
    if (workspaces.length > 0) {
      const currentAccess = Array.from(new Set(workspaces.map((workspace) => workspace.access)));
      if (currentAccess.some((access) => access !== wizard.workspaceAccess)) {
        changes.push({
          label: t("config.remoteIm.workspace"),
          from: currentAccess.map(workspaceAccessLabel).join(" / "),
          to: workspaceAccessLabel(wizard.workspaceAccess),
        });
      }
    }
  }

  if (wizard.fields.group) {
    const fromGroup = contactGroupLabelById(contactGroupIdOf(contact));
    const toGroup = contactGroupLabelById(wizard.targetGroupId);
    if (fromGroup !== toGroup) {
      changes.push({
        label: t("config.remoteIm.contactGroupLabel"),
        from: fromGroup,
        to: toGroup,
      });
    }
  }

  return changes;
}

const batchWizardPreviews = computed<BatchWizardPreview[]>(() => {
  if (!batchSettingsWizard.value) return [];
  return batchWizardSelectedContacts.value.map((contact) => ({
    contactId: contact.id,
    name: contactSafeDisplayName(contact),
    changes: batchWizardPreviewFor(contact),
  }));
});

const batchWizardChangedPreviews = computed(() => (
  batchWizardPreviews.value.filter((preview) => preview.changes.length > 0)
));

function syncBatchSettingsDialog() {
  const dialog = batchSettingsDialogRef.value;
  if (!dialog) return;
  if (batchSettingsWizard.value) {
    if (!dialog.open) dialog.showModal();
  } else if (dialog.open) {
    dialog.close();
  }
}

watch(batchSettingsWizard, syncBatchSettingsDialog);
watch(batchSettingsDialogRef, syncBatchSettingsDialog);

function openBatchSettingsWizard(defaultGroupKey: string) {
  const all = currentChannelContacts.value;
  const defaultScope = defaultGroupKey === "ungrouped"
    ? all.filter((contact) => !contactGroupIdOf(contact))
    : all.filter((contact) => contactGroupIdOf(contact) === defaultGroupKey);
  if (defaultScope.length === 0) return;
  batchWizardCollapsedGroups.value = new Set();
  batchSettingsWizard.value = {
    step: 1,
    fields: {
      agent: false,
      model: false,
      processingMode: false,
      activation: false,
      responseStrategy: false,
      communication: false,
      workspaceAccess: false,
      group: false,
    },
    agentId: "support",
    setPreferredModel: false,
    preferredApiConfigId: resolveAvailableOrExpertModelId(),
    processingMode: "continuous",
    activationMode: "always",
    activationKeywordsText: "",
    responseStrategy: "always_reply",
    allowCommunication: true,
    allowSendFiles: false,
    workspaceAccess: "read_only",
    targetGroupId: "",
    // 第二步范围是渠道全部联系人，默认只选中进入时的分组，其余分组可自行勾选。
    selectedContactIds: defaultScope.map((contact) => contact.id),
    contactSearchQuery: "",
    contactModels: {},
    preparing: false,
    executing: false,
    error: "",
    result: null,
  };
}

function closeBatchSettingsWizard() {
  batchSettingsWizard.value = null;
}

function batchWizardToggleField(key: BatchWizardFieldKey, enabled: boolean) {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return;
  wizard.fields[key] = enabled;
  if (key === "agent" && enabled && !wizard.agentId) {
    wizard.agentId = "support";
  }
  if (key === "model" && enabled && !wizard.preferredApiConfigId) {
    wizard.preferredApiConfigId = resolveAvailableOrExpertModelId();
  }
  wizard.error = "";
}

function batchWizardToggleContact(contactId: string, selected: boolean) {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return;
  const selectedIds = new Set(wizard.selectedContactIds);
  if (selected) selectedIds.add(contactId);
  else selectedIds.delete(contactId);
  wizard.selectedContactIds = Array.from(selectedIds);
}

function batchWizardToggleAllContacts(selected: boolean) {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return;
  wizard.selectedContactIds = selected
    ? batchWizardScopedContacts.value.map((contact) => contact.id)
    : [];
}

function goToBatchWizardContactsStep() {
  const wizard = batchSettingsWizard.value;
  if (!wizard) return;
  if (!batchWizardHasFieldSelection(wizard)) {
    wizard.error = t("config.remoteIm.batchSelectAtLeastOne");
    return;
  }
  wizard.error = "";
  wizard.step = 2;
}

async function goToBatchWizardPreviewStep() {
  const wizard = batchSettingsWizard.value;
  if (!wizard || wizard.preparing) return;
  if (wizard.selectedContactIds.length === 0) {
    wizard.error = t("config.remoteIm.batchNoContactSelected");
    return;
  }
  wizard.preparing = true;
  wizard.error = "";
  try {
    if (wizard.fields.model) {
      const entries = await Promise.all(
        batchWizardSelectedContacts.value.map(async (contact) => (
          [contact.id, await fetchContactConversationModel(contact.id)] as const
        )),
      );
      wizard.contactModels = Object.fromEntries(entries);
    } else {
      wizard.contactModels = {};
    }
    wizard.step = 3;
  } finally {
    wizard.preparing = false;
  }
}

async function executeBatchSettings() {
  const wizard = batchSettingsWizard.value;
  if (!wizard || wizard.executing) return;
  const targets = batchWizardSelectedContacts.value;
  if (targets.length === 0) {
    wizard.error = t("config.remoteIm.batchNoContactSelected");
    return;
  }
  wizard.executing = true;
  wizard.error = "";
  try {
    const contactIds = targets.map((contact) => contact.id);
    let updatedContactCount = 0;
    let workspaceChangedContactCount = 0;
    let movedContactCount = 0;

    // 移动到分组是整批一次写入，字段设置走批量 patch，两者可同时勾选。
    const channelId = selectedChannelId.value;
    if (wizard.fields.group && channelId) {
      await invokeTauri("remote_im_set_contact_group", {
        input: { channelId, contactIds, groupId: wizard.targetGroupId || null },
      });
      movedContactCount = contactIds.length;
    }

    if (batchWizardHasPatchFieldSelection(wizard)) {
      const input: Record<string, unknown> = { contactIds };
      if (wizard.fields.agent) input.agentId = wizard.agentId;
      if (wizard.fields.processingMode) input.processingMode = wizard.processingMode;
      if (wizard.fields.activation) {
        input.activationMode = wizard.activationMode;
        input.activationKeywords = parseActivationKeywords(wizard.activationKeywordsText);
      }
      if (wizard.fields.responseStrategy) input.responseStrategy = wizard.responseStrategy;
      if (wizard.fields.communication) {
        input.allowCommunication = wizard.allowCommunication;
        input.allowSendFiles = wizard.allowSendFiles;
      }
      if (wizard.fields.workspaceAccess) input.workspaceAccess = wizard.workspaceAccess;

      const result = await invokeTauri<{
        updatedContactIds: string[];
        updatedWorkspaceContactCount: number;
      }>("remote_im_batch_patch_contact_settings", { input });
      updatedContactCount = result.updatedContactIds.length;
      workspaceChangedContactCount = result.updatedWorkspaceContactCount;
    }

    // 首选模型挂在各自会话上，联系人不存储该字段，只能按会话逐个写入。
    let modelUpdatedCount = 0;
    let modelSkippedCount = 0;
    if (wizard.fields.model) {
      for (const contact of targets) {
        const model = wizard.contactModels[contact.id];
        if (!model?.conversationExists || !model.conversationId) {
          modelSkippedCount += 1;
          continue;
        }
        try {
          await invokeTauri("conversation.preferredModel.set", {
            input: {
              conversationId: model.conversationId,
              preferredApiConfigId: wizard.preferredApiConfigId || null,
            },
          });
          modelUpdatedCount += 1;
        } catch (error) {
          modelSkippedCount += 1;
          console.warn("[联系人批量设置] 首选模型写入失败", contact.id, error);
        }
      }
    }

    await refreshContacts();
    wizard.result = {
      updatedContactCount,
      workspaceChangedContactCount,
      modelUpdatedCount,
      modelSkippedCount,
      movedContactCount,
    };
    wizard.step = 4;
    props.setStatusAction(
      movedContactCount > 0 && updatedContactCount === 0
        ? t("config.remoteIm.moveToGroupDone")
        : t("config.remoteIm.batchSettingsDone", { count: updatedContactCount }),
    );
  } catch (error) {
    wizard.error = String(error);
  } finally {
    wizard.executing = false;
  }
}

</script>
