<template>
  <SettingsPageShell :breadcrumb="remoteImBreadcrumb" header-class="">
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
      <div v-if="inDetailMode && selectedChannel" :key="'detail-body-' + selectedChannel.id" class="grid grid-cols-1 gap-4 pb-8">
        <!-- 区块一：渠道基本与平台凭证配置 -->
        <div class="space-y-2">
          <div class="flex items-center justify-between px-1">
            <div class="flex items-center gap-2">
              <Settings class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.remoteIm.channelSettings") }}</span>
              <span v-if="channelDirty" class="badge badge-sm badge-warning">{{ t("config.skill.modified") }}</span>
            </div>
            <button
              class="btn btn-sm min-h-[2rem] btn-ghost text-error gap-1 px-2.5 text-xs hover:bg-error/10"
              type="button"
              :disabled="saving || isChannelOperationBusy(selectedChannel.id)"
              @click="deleteSelectedChannel"
            >
              <Trash2 class="h-3.5 w-3.5" />
              <span>{{ t("common.delete") }}</span>
            </button>
          </div>

          <div class="rounded-box border border-base-200/80 bg-base-100 p-4 space-y-4">
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

            <!-- 过滤 Markdown 开关 -->
            <div class="flex items-center justify-between rounded-field border border-base-200 bg-base-200/30 p-3">
              <div class="flex flex-col gap-0.5 min-w-0 pr-2">
                <span class="text-xs font-semibold">{{ t("config.remoteIm.filterMarkdown") }}</span>
                <span class="text-caption opacity-60">{{ t("config.remoteIm.filterMarkdownHint") }}</span>
              </div>
              <input v-model="selectedChannel.filterMarkdown" type="checkbox" class="toggle toggle-primary toggle-sm shrink-0" />
            </div>

            <!-- OneBot 凭证配置 -->
            <template v-if="selectedChannel.platform === 'onebot_v11'">
              <div class="rounded-field border border-base-200 bg-base-200/20 p-3 space-y-3">
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
              <div class="rounded-field border border-base-200 bg-base-200/20 p-3 space-y-3">
                <div class="text-xs font-bold">{{ t("config.remoteIm.dingtalkCredentials") }}</div>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.dingtalkClientId") }}</label>
                    <input v-model="dingtalkCredentials.clientId" class="input input-bordered input-sm h-9 w-full text-xs" placeholder="dingxxxxxxxxxxxxxxxx" />
                  </div>
                  <div>
                    <label class="text-caption opacity-60">{{ t("config.remoteIm.dingtalkClientSecret") }}</label>
                    <div class="flex items-center gap-2">
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
              <div class="rounded-field border border-base-200 bg-base-200/20 p-3 space-y-3">
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
              <div class="rounded-field border border-base-200 bg-base-200/20 p-3 space-y-2">
                <div class="text-xs font-bold">{{ t("config.remoteIm.credentialsJson") }}</div>
                <textarea
                  v-model="credentialDrafts[selectedChannel.id]"
                  class="textarea textarea-bordered w-full min-h-20 font-mono text-xs"
                  spellcheck="false"
                  @blur="syncCredentialJson(selectedChannel)"
                />
              </div>
            </template>
          </div>
        </div>

        <!-- 区块二：联系人列表（直接一张一张列出，去除外层大卡片包裹） -->
        <div class="space-y-3 pt-2">
          <div class="flex flex-col gap-3 px-1 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between">
            <div class="flex shrink-0 items-center gap-2">
              <Users class="h-4 w-4 opacity-70" />
              <span class="text-xs font-semibold uppercase tracking-wider opacity-80">{{ t("config.remoteIm.contactsTitle") }}</span>
              <span class="badge badge-sm badge-neutral">{{ currentChannelContacts.length }}</span>
            </div>

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

          <div v-if="contactsDisabledReason" class="rounded-box px-4 py-2.5 text-xs text-warning bg-warning/10 border border-warning/20 break-all whitespace-pre-wrap" style="overflow-wrap:anywhere">
            {{ contactsDisabledReason }}
          </div>

          <div v-if="contactsError" class="rounded-box px-4 py-3 text-xs text-error bg-error/10 border border-error/20 break-all whitespace-pre-wrap" style="overflow-wrap:anywhere">
            {{ contactsError }}
          </div>
          <div v-else-if="currentChannelContacts.length === 0" class="rounded-box border border-dashed border-base-300 py-12 text-center text-xs opacity-60 italic">
            {{ t("config.remoteIm.contactsEmpty") }}
          </div>
          <div v-else class="space-y-4">
            <div v-for="group in groupedContacts" :key="group.mode" class="space-y-2">
              <div class="text-xs font-bold text-base-content/70 px-1 flex items-center gap-1.5">
                <span>{{ group.label }}</span>
                <span class="badge badge-ghost badge-xs">{{ group.items.length }}</span>
              </div>
              <div class="space-y-2">
                <div
                  v-for="item in group.items"
                  :key="item.id"
                  class="rounded-box border border-base-200/80 bg-base-100 p-3.5 hover:border-base-300 hover:bg-base-200/30 transition-all flex items-start gap-3.5"
                >
                  <div class="avatar placeholder shrink-0">
                    <div class="flex h-10 w-10 items-center justify-center overflow-hidden rounded-full border border-base-300 bg-base-200 text-xs font-semibold leading-none text-base-content/70">
                      <img v-if="contactAvatarUrl(item)" :src="contactAvatarUrl(item)" :alt="contactSafeDisplayName(item)" class="block h-full w-full object-cover" />
                      <span v-else>{{ contactAvatarFallbackText(item) }}</span>
                    </div>
                  </div>

                  <div class="flex-1 min-w-0">
                    <div class="flex items-center justify-between gap-2">
                      <div class="min-w-0 flex-1 truncate font-semibold text-sm">
                        <span class="font-normal opacity-60 text-xs">[{{ contactAgentLabel(item) }}]</span>
                        {{ " " }}
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
                          :disabled="contactsDisabled"
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
                          :title="t('config.remoteIm.channelDetails')"
                          :disabled="contactsDisabled"
                          @click.stop="openContactConfigModal(item.id)"
                        >
                          <Settings class="h-4 w-4" />
                        </button>
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('common.copy')"
                          :disabled="contactsDisabled || isContactOperationBusy(item.id)"
                          @click.stop="copyContactSettings(item)"
                        >
                          <Copy class="h-4 w-4" />
                        </button>
                        <button
                          class="btn btn-ghost btn-circle h-8 w-8 min-h-[2rem]"
                          :title="t('common.paste')"
                          :disabled="contactsDisabled || isContactOperationBusy(item.id) || !contactSettingsClipboard"
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
                          :disabled="contactsDisabled || isContactOperationBusy(item.id)"
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
                          :disabled="contactsDisabled || isContactOperationBusy(item.id)"
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
                          :disabled="contactsDisabled || isContactOperationBusy(item.id) || isPrivateContact(item)"
                          @click.stop="!isPrivateContact(item) && openContactPillMenu($event, item, 'response')"
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
                          :disabled="contactsDisabled || isContactOperationBusy(item.id)"
                          @click.stop="openContactPillMenu($event, item, 'files')"
                        >
                          {{ contactSendFilesLabel(item) }}
                          <ChevronUp class="h-3.5 w-3.5 opacity-70" />
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 一级概览：渠道 2 列卡片矩阵 -->
      <div v-else key="overview-grid" class="grid gap-4 pb-8">
        <div class="config-grid-auto-sm">
          <div
            v-for="ch in filteredChannels"
            :key="ch.id"
            role="button"
            tabindex="0"
            class="rounded-box border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] group"
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
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption">
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

    <!-- 联系人配置模态框 -->
    <dialog ref="contactConfigDialogRef" class="modal" @close="closeContactConfigModal" @cancel.prevent="closeContactConfigModal">
      <div class="modal-box max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
        <div class="flex items-center justify-between shrink-0">
          <div class="font-semibold text-lg flex items-center gap-2">
            <span class="inline-flex h-8 w-8 items-center justify-center rounded-full bg-[#07c160] text-white text-sm font-bold">微</span>
            <span>{{ t("config.remoteIm.contactSettingsTitle", { name: contactModalTitle }) }}</span>
          </div>
          <button class="btn btn-sm btn-circle btn-ghost" @click="closeContactConfigModal">
            <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div v-if="selectedContact && contactDraft" class="mt-4 flex-1 min-h-0 overflow-hidden flex flex-col">
          <div class="flex-1 overflow-y-auto">
            <ul class="list gap-2">
              <li class="list-row flex items-start justify-between gap-3">
                <div class="font-medium">{{ t("config.remoteIm.processingAgent") }}</div>
                <div class="flex w-[28rem] max-w-full flex-col gap-1">
                  <AgentPersonaSelect
                    v-model:agent-id="contactDraft.boundAgentId"
                    :personas="personas"
                    :persona-avatar-url-map="personaAvatarUrlMap"
                    :api-configs="config.apiConfigs"
                    :expert-api-config-id="config.expertApiConfigId"
                    :tool-review-api-config-id="config.toolReviewApiConfigId"
                    :placeholder="t('config.remoteIm.processingAgentPlaceholder')"
                    :show-model="false"
                    :disabled="contactsDisabled"
                  />
                  <span class="text-xs opacity-60">{{ contactDraftRoutingHint }}</span>
                </div>
              </li>

              <li class="list-row flex items-start justify-between gap-3">
                <div class="font-medium">{{ t("config.remoteIm.processingMode") }}</div>
                <div class="flex w-64 flex-col gap-1">
                  <select
                    class="select select-bordered select-sm w-full"
                    v-model="contactDraft.processingMode"
                  >
                    <option value="continuous">{{ t("config.remoteIm.processingModeContinuous") }}</option>
                    <option value="qa">{{ t("config.remoteIm.processingModeQa") }}</option>
                  </select>
                  <span class="text-xs opacity-60">{{ contactDraftProcessingHint }}</span>
                </div>
              </li>

              <li v-if="selectedContact" class="list-row flex items-start justify-between gap-3">
                <div class="font-medium">{{ t("config.remoteIm.activateMode") }}</div>
                <div class="flex w-64 flex-col gap-2">
                  <select
                    class="select select-bordered select-sm w-full"
                    v-model="contactDraft.activationMode"
                  >
                    <option
                      v-for="option in contactActivationModeOptions(selectedContact)"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </option>
                  </select>
                  <span class="text-xs opacity-60">{{ contactDraftActivationHint }}</span>
                  <input
                    v-if="!isPrivateContact(selectedContact) && contactDraft.activationMode === 'keyword'"
                    type="text"
                    class="input input-bordered input-sm w-full"
                    :placeholder="t('config.remoteIm.activateKeywordsPlaceholder')"
                    v-model="contactDraft.activationKeywordsText"
                  />
                </div>
              </li>

              <li
                v-if="selectedContact && !isPrivateContact(selectedContact)"
                class="list-row flex items-start justify-between gap-3"
              >
                <div class="font-medium">{{ t("config.remoteIm.responseStrategy") }}</div>
                <div class="flex w-64 flex-col gap-2">
                  <select
                    class="select select-bordered select-sm w-full"
                    v-model="contactDraft.responseStrategy"
                  >
                    <option value="always_reply">{{ t("config.remoteIm.responseStrategyAlways") }}</option>
                    <option value="smart_judge">{{ t("config.remoteIm.responseStrategySmart") }}</option>
                  </select>
                  <span class="text-xs opacity-60">{{ contactDraftResponseStrategyHint }}</span>
                </div>
              </li>

              <li class="list-row flex items-center justify-between gap-3">
                <div class="font-medium">{{ t("config.remoteIm.allowSendFiles") }}</div>
                <input
                  type="checkbox"
                  class="toggle toggle-sm toggle-primary"
                  v-model="contactDraft.allowSendFiles"
                />
              </li>
            </ul>

              <!-- 工作目录配置 -->
              <li class="list-row flex flex-col gap-2 pt-3 mt-2 border-t border-base-300">
                <div class="flex items-center justify-between">
                  <div class="font-medium">{{ t("config.remoteIm.workspace") }}</div>
                  <span class="text-xs opacity-50">{{ t("config.remoteIm.systemWorkspaceReadonly") }}</span>
                </div>
                <div
                  v-if="contactDraft.shellWorkspaces.length === 0"
                  class="rounded-box border border-dashed border-base-300 bg-base-200/20 px-3 py-4 text-center text-xs opacity-60"
                >
                  {{ t("config.remoteIm.noCustomWorkspace") }}
                </div>
                <div v-else class="divide-y divide-base-300">
                  <div
                    v-for="ws in contactDraft.shellWorkspaces"
                    :key="ws.id"
                    class="py-2 text-left"
                    :title="ws.path"
                  >
                    <div class="flex items-center gap-3">
                      <div class="min-w-0 flex-1 text-left">
                        <div class="flex flex-wrap items-center gap-2">
                          <span class="inline-block w-40 truncate font-medium align-middle" :title="ws.path">{{ ws.name }}</span>
                          <span v-if="ws.level === 'main'" class="badge badge-primary">
                            {{ t("config.tools.workspaceLevelMain") }}
                          </span>
                          <span class="badge" :class="ws.access === 'full_access' ? 'badge-success' : ws.access === 'approval' ? 'badge-warning' : 'badge-ghost'">
                            {{ ws.access === 'full_access' ? t("config.tools.workspaceAccessFullAccess") : ws.access === 'approval' ? t("config.tools.workspaceAccessApproval") : t("config.tools.workspaceAccessReadOnly") }}
                          </span>
                        </div>
                      </div>
                      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
                        <button
                          v-if="ws.level !== 'main'"
                          class="btn btn-xs btn-ghost"
                          type="button"
                          :title="t('config.tools.setWorkspaceAsMain')"
                          @click="setContactWorkspaceMain(ws.id)"
                        >
                          <SquareTerminal class="h-3.5 w-3.5" />
                        </button>
                        <button
                          v-else
                          class="btn btn-xs btn-primary pointer-events-none opacity-100"
                          type="button"
                          aria-disabled="true"
                          tabindex="-1"
                          :title="t('config.tools.currentMainWorkspace')"
                        >
                          <SquareTerminal class="h-3.5 w-3.5" />
                        </button>
                        <select
                          class="select select-sm select-bordered w-32"
                          :value="ws.access"
                          @change="updateContactWorkspaceAccess(ws.id, ($event.target as HTMLSelectElement).value as ShellWorkspace['access'])"
                        >
                          <option value="full_access">{{ t("config.tools.workspaceAccessFullAccess") }}</option>
                          <option value="approval">{{ t("config.tools.workspaceAccessApproval") }}</option>
                          <option value="read_only">{{ t("config.tools.workspaceAccessReadOnly") }}</option>
                        </select>
                        <button
                          class="btn btn-sm btn-ghost text-error"
                          type="button"
                          :title="t('config.tools.delete')"
                          @click="removeContactWorkspace(ws.id)"
                        >
                          <Trash2 class="h-4 w-4" />
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
                <div class="flex justify-end">
                  <button class="btn btn-sm" type="button" @click="addContactWorkspace">
                    {{ t("config.tools.addWorkspace") }}
                  </button>
                </div>
              </li>

          <div class="mt-2 text-xs opacity-60 leading-5">
            {{ t("config.remoteIm.contactConversationHint") }}
          </div>
          </div>
          <div class="mt-3 pt-3 border-t border-base-300 flex items-center justify-between gap-2 shrink-0">
            <button
              class="btn btn-ghost text-error"
              :disabled="contactsDisabled || contactSaving || contactDeleting || !selectedContact"
              @click="selectedContact && deleteContact(selectedContact)"
            >
              <Trash2 class="h-3.5 w-3.5" />
              {{ t("common.delete") }}
            </button>
            <div class="flex items-center gap-2">
            <button class="btn btn-ghost" :disabled="!contactDraftDirty || contactSaving" @click="resetContactDraft">
              <RotateCcw class="h-3.5 w-3.5" />
              {{ t("common.reset") }}
            </button>
            <button class="btn btn-primary" :disabled="contactsDisabled || !contactDraftDirty || contactSaving || contactDeleting" @click="saveContactDraft">
              <Save v-if="!contactSaving" class="h-3.5 w-3.5" />
              <span v-else class="loading loading-spinner loading-xs"></span>
              {{ t("common.save") }}
            </button>
            </div>
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeContactConfigModal">close</button>
      </form>
    </dialog>
  </SettingsPageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertTriangle,
  ChevronRight,
  ChevronUp,
  ClipboardPaste,
  Copy,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  ScrollText,
  Search,
  Settings,
  SquareTerminal,
  Trash2,
  Users,
} from "@lucide/vue";
import { invokeTauri, openTransportFileDialog } from "../../../../services/tauri-api";
import type { AppConfig, PersonaProfile, RemoteImChannelConfig, RemoteImContact, RemoteImPlatform, ShellWorkspace } from "../../../../types/app";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import type { SettingsBreadcrumbBadge, SettingsBreadcrumbItem } from "../../components/SettingsBreadcrumb.vue";
import AgentPersonaSelect from "../../../shared/components/AgentPersonaSelect.vue";
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
type ContactPillMenuKind = "activation" | "processing" | "response" | "files";
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
type ContactSettingsClipboard = {
  boundAgentId: string;
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
const contactConfigDialogRef = ref<HTMLDialogElement | null>(null);
const contactConfigModalOpen = ref(false);

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
function syncContactConfigDialog() {
  const d = contactConfigDialogRef.value;
  if (!d) return;
  if (contactConfigModalOpen.value) {
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
watch(contactConfigModalOpen, syncContactConfigDialog);
watch(contactConfigDialogRef, syncContactConfigDialog);
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
    class: channel.enabled ? "badge-neutral" : "badge-ghost opacity-60",
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

const currentChannelContacts = computed(() => {
  if (!selectedChannelId.value) return [];
  return contacts.value.filter((c) => c.channelId === selectedChannelId.value);
});

const filteredCurrentContacts = computed(() => {
  const all = currentChannelContacts.value;
  const q = contactSearchQuery.value.trim().toLowerCase();
  if (!q) return all;
  return all.filter((c) => {
    const name = contactSafeDisplayName(c).toLowerCase();
    const agent = contactAgentLabel(c).toLowerCase();
    const sec = contactSecondaryText(c).toLowerCase();
    return name.includes(q) || agent.includes(q) || sec.includes(q);
  });
});

const contactsDisabledReason = computed(() => {
  const channel = selectedChannel.value;
  if (!channel) return "";
  if (!channel.enabled) return t('config.remoteIm.channelDisabledHint');
  if (channel.platform === "feishu") return "";
  if (channel.platform === "onebot_v11" || channel.platform === "dingtalk" || channel.platform === "weixin_oc") {
    const status = channelRuntimeStates.value[channel.id];
    if (status?.connected) return "";
    if (channel.platform === "onebot_v11" && status?.statusText === "binding_retry") {
      return status.lastError || t('config.remoteIm.onebotPortOccupied');
    }
    if (channel.platform === "onebot_v11" && status?.statusText === "bind_failed") {
      return status.lastError || t('config.remoteIm.onebotBindFailed');
    }
    if (channel.platform === "onebot_v11" && status?.statusText === "binding") {
      return t('config.remoteIm.onebotBinding');
    }
    if (channel.platform === "onebot_v11" && status?.listenAddr) {
      return t('config.remoteIm.onebotListening', { addr: status.listenAddr });
    }
    return t('config.remoteIm.channelNotConnected');
  }
  return "";
});
const contactsDisabled = computed(() => !!contactsDisabledReason.value);
const contactActivationModeOrder: RemoteImContact["activationMode"][] = ["always", "keyword", "never"];

type ContactGroup = { mode: "always" | "keyword" | "never"; label: string; items: typeof currentChannelContacts.value };
const groupedContacts = computed<ContactGroup[]>(() => {
  const all = filteredCurrentContacts.value;
  const groups: { mode: ContactGroup["mode"]; label: string; items: typeof all }[] = [
    { mode: "always", label: t("config.remoteIm.activateModeAlways"), items: [] },
    { mode: "keyword", label: t("config.remoteIm.activateModeKeyword"), items: [] },
    { mode: "never", label: t("config.remoteIm.activateModeNever"), items: [] },
  ];
  for (const c of all) {
    const mode = isPrivateContact(c) ? "always" : normalizeActivationMode(c.activationMode);
    const target = groups.find((g) => g.mode === mode);
    (target ?? groups[2]).items.push(c);
  }
  return groups.filter((g) => g.items.length > 0);
});
const selectedContact = computed(() =>
  currentChannelContacts.value.find((item) => item.id === selectedContactId.value) ?? null,
);
const contactLogsTarget = computed(() =>
  contacts.value.find((item) => item.id === contactLogsContactId.value) ?? null,
);
const contactModalTitle = computed(() => {
  if (!selectedContact.value) return "-";
  if (selectedContact.value.platform === "weixin_oc") return t("config.remoteIm.weixinContact");
  return contactDisplayName(selectedContact.value);
});
const contactLogsTitle = computed(() => {
  const target = contactLogsTarget.value;
  if (!target) return "-";
  return contactSafeDisplayName(target);
});
type ContactEditDraft = {
  boundAgentId: string;
  processingMode: "qa" | "continuous";
  activationMode: RemoteImContact["activationMode"];
  activationKeywordsText: string;
  responseStrategy: NonNullable<RemoteImContact["responseStrategy"]>;
  allowReceive: boolean;
  allowSend: boolean;
  allowSendFiles: boolean;
  shellWorkspaces: ShellWorkspace[];
};
const contactDraft = ref<ContactEditDraft | null>(null);
const contactDraftSnapshot = ref("");
const contactDraftDirty = computed(() =>
  !!contactDraft.value && JSON.stringify(contactDraft.value) !== contactDraftSnapshot.value,
);
const contactDraftRoutingHint = computed(() => {
  if (!selectedContact.value || !contactDraft.value) return "";
  return t("config.remoteIm.routingHint");
});
const contactDraftProcessingHint = computed(() => {
  if (!selectedContact.value || !contactDraft.value) return "";
  return processingModeHintText({
    ...selectedContact.value,
    processingMode: contactDraft.value.processingMode,
  } as RemoteImContact);
});
const contactDraftActivationHint = computed(() => {
  if (!selectedContact.value || !contactDraft.value) return "";
  return contactActivationHintText({
    ...selectedContact.value,
    activationMode: contactDraft.value.activationMode,
  } as RemoteImContact);
});
const contactDraftResponseStrategyHint = computed(() => {
  if (!selectedContact.value || !contactDraft.value) return "";
  return contactResponseStrategyHintText({
    ...selectedContact.value,
    responseStrategy: contactDraft.value.responseStrategy,
  } as RemoteImContact);
});
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
    boundAgentId: String(item.boundAgentId || ""),
    processingMode: normalizeProcessingMode(item.processingMode),
    activationMode: isPrivateContact(item) ? "always" : normalizeActivationMode(item.activationMode || "never"),
    activationKeywordsText: item.activationKeywords.join(", "),
    responseStrategy: normalizeResponseStrategy(item.responseStrategy),
    allowReceive: !!item.allowReceive,
    allowSend: !!item.allowSend,
    allowSendFiles: !!item.allowSendFiles,
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

function buildContactSettingsClipboard(item: RemoteImContact): ContactSettingsClipboard {
  const isPrivate = isPrivateContact(item);
  return {
    boundAgentId: String(item.boundAgentId || ""),
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

async function toggleChannelFilterMarkdown(channel: RemoteImChannelConfig) {
  if (saving.value || isChannelOperationBusy(channel.id)) return;
  const oldValue = !!channel.filterMarkdown;
  channel.filterMarkdown = !oldValue;
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      channel.filterMarkdown = oldValue;
      props.setStatusAction(t("config.remoteIm.channelSaveFailed"));
      return;
    }
    await nextTick();
    lastSavedChannelSnapshot.value = channelSnapshot.value;
    props.setStatusAction(channel.filterMarkdown
      ? t("config.remoteIm.filterMarkdownEnabled")
      : t("config.remoteIm.filterMarkdownDisabled"));
  } catch (error) {
    channel.filterMarkdown = oldValue;
    props.setStatusAction(t("config.remoteIm.channelSaveFailed"));
  }
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
  if (contactsDisabled.value) return;
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
  if (contactsDisabled.value) return;
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
  if (contactsDisabled.value) return;
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
  contactSettingsClipboard.value = buildContactSettingsClipboard(item);
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
  if (contactsDisabled.value || isContactOperationBusy(item.id)) return;
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
  if (contactsDisabled.value) return;
  const nextMode = normalizeActivationMode(mode);
  if (normalizeActivationMode(item.activationMode || "never") === nextMode) return;
  await withContactOperation(item.id, () => saveContactActivation(item, { activationMode: nextMode }));
}

function closeContactPillMenu() {
  contactPillMenu.value = null;
}

function contactPillMenuWidthClass(kind: ContactPillMenuKind): string {
  if (kind === "processing") return "w-40";
  if (kind === "files") return "w-32";
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
  if (contactsDisabled.value || isContactOperationBusy(item.id)) return;
  const target = event.currentTarget as HTMLElement | null;
  const rect = target?.getBoundingClientRect();
  if (!rect) return;
  const options = contactPillMenuOptions(item, kind);
  const menuHeight = options.length * 32 + 10;
  const menuWidth = kind === "processing" ? 160 : kind === "files" ? 128 : 144;
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
  if (contactsDisabled.value) return;
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
  if (contactsDisabled.value) return;
  const nextMode = normalizeProcessingMode(mode);
  if (normalizeProcessingMode(item.processingMode) === nextMode) return;
  await withContactOperation(item.id, () => onContactProcessingModeChange(item, nextMode));
}

async function cycleContactProcessingMode(item: RemoteImContact) {
  if (contactsDisabled.value) return;
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
  if (contactsDisabled.value || isPrivateContact(item)) return;
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
  if (contactsDisabled.value) return;
  if (!!item.allowSendFiles === enabled) return;
  await withContactOperation(item.id, () => toggleContactAllowSendFiles(item, enabled));
}

async function cycleContactAllowSendFiles(item: RemoteImContact) {
  if (contactsDisabled.value) return;
  await withContactOperation(item.id, () => toggleContactAllowSendFiles(item, !item.allowSendFiles));
}

function onContactActivationKeywordsBlur(item: RemoteImContact) {
  const raw = contactKeywordDrafts.value[item.id] ?? item.activationKeywords.join(", ");
  const keywords = parseActivationKeywords(raw);
  contactKeywordDrafts.value[item.id] = keywords.join(", ");
  void saveContactActivation(item, { activationKeywords: keywords });
}

async function addContactWorkspace() {
  if (!contactDraft.value) return;
  try {
    const picked = await openTransportFileDialog({
      directory: true,
      multiple: false,
    });
    if (!picked || Array.isArray(picked)) return;
    const path = String(picked || "").trim();
    if (!path) return;

    const existed = contactDraft.value.shellWorkspaces.some(
      (ws) => ws.path.toLowerCase() === path.toLowerCase(),
    );
    if (existed) return;

    const hasMain = contactDraft.value.shellWorkspaces.some((ws) => ws.level === "main");
    const name = path.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || path;
    contactDraft.value.shellWorkspaces.push({
      id: crypto.randomUUID(),
      name,
      path,
      level: hasMain ? "secondary" : "main",
      access: (hasMain ? "read_only" : "approval") as unknown as ShellWorkspace["access"],
    });
  } catch {
    // 用户取消选择
  }
}

function removeContactWorkspace(id: string) {
  if (!contactDraft.value) return;
  const idx = contactDraft.value.shellWorkspaces.findIndex((ws) => ws.id === id);
  if (idx >= 0) contactDraft.value.shellWorkspaces.splice(idx, 1);
}

function setContactWorkspaceMain(id: string) {
  if (!contactDraft.value) return;
  for (const ws of contactDraft.value.shellWorkspaces) {
    ws.level = ws.id === id ? "main" : "secondary";
  }
}

function updateContactWorkspaceAccess(id: string, access: ShellWorkspace["access"]) {
  if (!contactDraft.value) return;
  const ws = contactDraft.value.shellWorkspaces.find((w) => w.id === id);
  if (ws) ws.access = access;
}

function resetContactDraft() {
  syncSelectedContactDraft();
}

async function saveContactDraft() {
  if (contactsDisabled.value) return;
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

    const nextProcessingMode = normalizeProcessingMode(draft.processingMode);
    if (nextProcessingMode !== normalizeProcessingMode(item.processingMode)) {
      await onContactProcessingModeChange(item, nextProcessingMode);
    }

    const nextKeywords = parseActivationKeywords(draft.activationKeywordsText);
    const currentKeywords = Array.isArray(item.activationKeywords) ? item.activationKeywords : [];
    const keywordsChanged = JSON.stringify(nextKeywords) !== JSON.stringify(currentKeywords);
    const nextActivationMode = normalizeActivationMode(draft.activationMode);
    const modeChanged = nextActivationMode !== normalizeActivationMode(item.activationMode || "never");
    const nextResponseStrategy = normalizeResponseStrategy(draft.responseStrategy);
    const responseStrategyChanged =
      nextResponseStrategy !== normalizeResponseStrategy(item.responseStrategy);
    if (
      modeChanged
      || keywordsChanged
      || responseStrategyChanged
    ) {
      await saveContactActivation(item, {
        activationMode: nextActivationMode,
        activationKeywords: nextKeywords,
        responseStrategy: nextResponseStrategy,
      });
    }

    if (!!draft.allowReceive !== !!item.allowReceive || !!draft.allowSend !== !!item.allowSend) {
      await toggleContactCommunication(item, !!draft.allowReceive || !!draft.allowSend);
    }
    if (!!draft.allowSendFiles !== !!item.allowSendFiles) {
      await toggleContactAllowSendFiles(item, !!draft.allowSendFiles);
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
      contactConfigModalOpen.value = false;
      selectedContactId.value = "";
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
  if (contactsDisabled.value) return;
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
      contactConfigModalOpen.value = false;
      selectedContactId.value = "";
      contactDraft.value = null;
      contactDraftSnapshot.value = "";
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
  const agentId = String(item.boundAgentId || "").trim();
  if (!agentId) return t("config.remoteIm.processingAgentDefault");
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

function contactAvatarFallbackText(item: RemoteImContact): string {
  const name = contactSafeDisplayName(item).trim();
  if (name) return Array.from(name)[0] || "?";
  return item.remoteContactType === "group" ? t('config.remoteIm.avatarGroup') : t('config.remoteIm.avatarPrivate');
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

function openContactConfigModal(contactId: string) {
  selectedContactId.value = contactId;
  syncSelectedContactDraft();
  contactConfigModalOpen.value = true;
}

function closeContactConfigModal() {
  contactConfigModalOpen.value = false;
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
</script>
