<template>
  <div class="h-full min-h-0 overflow-hidden">
  <Transition name="ecall-config-mode" mode="out-in">
  <div v-if="props.simpleSetupMode" key="simple" class="h-full min-h-0 overflow-hidden bg-base-200 pl-4">
    <SimpleSetupPanel class="h-full" />
  </div>
  <div v-else key="advanced" class="config-shell flex h-full min-h-0 overflow-hidden">
    <aside class="hidden md:flex relative h-full min-h-0 w-44 shrink-0 flex-col bg-base-200 px-2">
      <OverlayScrollArea class="min-h-0 flex-1" scroller-class="pr-1 h-full">
        <ul class="menu w-full gap-0.5 p-0 pt-2 pb-6 [&>li>a]:w-full">
          <template v-for="(group, groupIndex) in visibleConfigNavGroups" :key="group.id">
            <li class="menu-title px-1 pb-0.5" :class="groupIndex > 0 ? 'pt-2.5' : 'pt-1'">
              <div
                v-if="group.collapsible"
                role="button"
                tabindex="0"
                class="flex w-full cursor-pointer items-center justify-between gap-1 rounded px-1.5 py-1 text-caption font-medium tracking-wider text-base-content/50 uppercase select-none hover:text-base-content/80 hover:bg-base-300/40 transition-colors"
                @click="toggleNavGroup(group)"
                @keydown.enter.prevent="toggleNavGroup(group)"
                @keydown.space.prevent="toggleNavGroup(group)"
              >
                <span class="min-w-0 truncate">{{ t(group.titleKey) }}</span>
                <span class="flex shrink-0 items-center gap-1">
                  <span
                    v-if="isNavGroupCollapsed(group) && groupHasBadge(group)"
                    class="inline-flex h-2 w-2 shrink-0 rounded-full bg-error"
                    :title="t('about.updateAvailableBadge')"
                  ></span>
                  <ChevronRight
                    class="h-3 w-3 shrink-0 transition-transform duration-150"
                    :class="{ 'rotate-90': !isNavGroupCollapsed(group) }"
                  />
                </span>
              </div>
              <div
                v-else
                class="px-1.5 py-1 text-caption font-medium tracking-wider text-base-content/50 uppercase select-none"
              >
                <span class="min-w-0 truncate">{{ t(group.titleKey) }}</span>
              </div>
            </li>
            <template v-if="!isNavGroupCollapsed(group)">
              <li v-for="item in group.items" :key="item.tab">
                <a :class="configNavLinkClass(item.tab)" @click="selectConfigNavTab(item.tab)">
                  <component :is="item.icon" class="h-4 w-4 shrink-0" />
                  <span class="min-w-0 truncate">{{ item.labelKey ? t(item.labelKey) : item.label }}</span>
                  <span
                    v-if="item.tab === 'about' && props.hasAvailableUpdate"
                    class="ml-auto inline-flex h-2.5 w-2.5 shrink-0 rounded-full bg-error"
                    :title="t('about.updateAvailableBadge')"
                  ></span>
                </a>
              </li>
            </template>
          </template>
        </ul>
      </OverlayScrollArea>
    </aside>

    <Transition name="ecall-config-drawer-mask">
      <div
        v-if="configDrawerOpen"
        class="fixed inset-0 z-30 bg-black/40 md:hidden"
        @click="configDrawerOpen = false"
      ></div>
    </Transition>
    <Transition name="ecall-config-drawer">
      <aside
        v-if="configDrawerOpen"
        class="fixed inset-y-0 left-0 z-40 flex h-full w-44 flex-col bg-base-200 px-2 shadow-xl md:hidden"
      >
        <OverlayScrollArea class="min-h-0 flex-1" scroller-class="pr-1 h-full">
          <ul class="menu w-full gap-0.5 p-0 pt-2 pb-6 [&>li>a]:w-full">
            <template v-for="(group, groupIndex) in visibleConfigNavGroups" :key="group.id">
              <li class="menu-title px-1 pb-0.5" :class="groupIndex > 0 ? 'pt-2.5' : 'pt-1'">
                <div
                  v-if="group.collapsible"
                  role="button"
                  tabindex="0"
                  class="flex w-full cursor-pointer items-center justify-between gap-1 rounded px-1.5 py-1 text-caption font-medium tracking-wider text-base-content/50 uppercase select-none hover:text-base-content/80 hover:bg-base-300/40 transition-colors"
                  @click="toggleNavGroup(group)"
                  @keydown.enter.prevent="toggleNavGroup(group)"
                  @keydown.space.prevent="toggleNavGroup(group)"
                >
                  <span class="min-w-0 truncate">{{ t(group.titleKey) }}</span>
                  <span class="flex shrink-0 items-center gap-1">
                    <span
                      v-if="isNavGroupCollapsed(group) && groupHasBadge(group)"
                      class="inline-flex h-2 w-2 shrink-0 rounded-full bg-error"
                      :title="t('about.updateAvailableBadge')"
                    ></span>
                    <ChevronRight
                      class="h-3 w-3 shrink-0 transition-transform duration-150"
                      :class="{ 'rotate-90': !isNavGroupCollapsed(group) }"
                    />
                  </span>
                </div>
                <div
                  v-else
                  class="px-1.5 py-1 text-caption font-medium tracking-wider text-base-content/50 uppercase select-none"
                >
                  <span class="min-w-0 truncate">{{ t(group.titleKey) }}</span>
                </div>
              </li>
              <template v-if="!isNavGroupCollapsed(group)">
                <li v-for="item in group.items" :key="item.tab">
                  <a :class="configNavLinkClass(item.tab)" @click="selectConfigNavTab(item.tab)">
                    <component :is="item.icon" class="h-4 w-4 shrink-0" />
                    <span class="min-w-0 truncate">{{ item.labelKey ? t(item.labelKey) : item.label }}</span>
                    <span
                      v-if="item.tab === 'about' && props.hasAvailableUpdate"
                      class="ml-auto inline-flex h-2.5 w-2.5 shrink-0 rounded-full bg-error"
                      :title="t('about.updateAvailableBadge')"
                    ></span>
                  </a>
                </li>
              </template>
            </template>
          </ul>
        </OverlayScrollArea>
      </aside>
    </Transition>

    <div class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-base-200">
      <div class="flex shrink-0 items-center gap-2 border-b border-base-300 bg-base-100/80 px-3 py-2 md:hidden">
        <button
          class="btn btn-square btn-ghost btn-sm"
          aria-label="打开设置导航"
          title="打开设置导航"
          @click="configDrawerOpen = true"
        >
          <Menu class="h-4 w-4" />
        </button>
        <div class="min-w-0 truncate text-sm font-medium">{{ activeConfigTabTitle }}</div>
      </div>

      <div class="flex min-h-0 flex-1 min-w-0 flex-col overflow-hidden">
      <Transition name="ecall-config-content" mode="out-in">
        <div :key="props.configTab" class="flex min-h-0 flex-1 min-w-0 flex-col overflow-hidden">
      <div v-if="props.configTab === 'api'" class="flex-1 min-h-0">
        <ApiTab
          :config="config"
          :base-url-reference="baseUrlReference"
          :refreshing-models="refreshingModels"
          :model-options="modelOptions"
          :model-refresh-ok="modelRefreshOk"
          :model-refresh-error="modelRefreshError"
          :config-dirty="configDirty"
          :saving-config="savingConfig"
          :save-api-config-action="props.saveConfigAction"
          :restore-api-config-action="props.restoreConfigAction"
          :normalize-api-bindings-action="normalizeApiBindingsAction"
          :last-saved-config-json="props.lastSavedConfigJson"
          :set-status-action="setStatusAction"
          @refresh-models="$emit('refreshModels')"
        />
      </div>

      <div v-else-if="props.configTab === 'mcp'" class="flex-1 min-h-0">
        <McpTab :set-status-action="setStatusAction" />
      </div>

      <div v-else-if="props.configTab === 'skill'" class="flex-1 min-h-0">
        <SkillTab />
      </div>

      <div v-else-if="props.configTab === 'catalog'" class="flex-1 min-h-0">
        <CatalogTab />
      </div>

      <div v-else-if="props.configTab === 'persona'" class="flex-1 min-h-0">
        <PersonaTab
          :personas="personas"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :assistant-personas="assistantPersonas"
          :persona-editor-id="personaEditorId"
          :selected-persona="selectedPersona"
          :selected-persona-avatar-url="selectedPersonaAvatarUrl"
          :avatar-saving="avatarSaving"
          :avatar-error="avatarError"
          :persona-saving="personaSaving"
          :persona-dirty="personaDirty"
          :config-saving="savingConfig"
          :text-capable-api-configs="textCapableApiConfigs"
          :expert-api-config-id="props.config?.expertApiConfigId ?? ''"
          :tool-review-api-config-id="props.config?.toolReviewApiConfigId ?? ''"
          :save-relations="props.savePersonaRelations"
          :set-status-action="setStatusAction"
          @update:persona-editor-id="$emit('update:personaEditorId', $event)"
          @add-persona="$emit('addPersona')"
          @remove-selected-persona="$emit('removeSelectedPersona')"
          @reset-personas="$emit('resetPersonas')"
          @open-avatar-editor="openAvatarEditorForSelected"
          @import-persona-memories="$emit('importPersonaMemories', $event)"
          @save-personas="$emit('savePersonas')"
          @convert-private-persona-to-public="$emit('convertPrivatePersonaToPublic', $event)"
        />
      </div>

      <div v-else-if="props.configTab === 'demo' && SHOW_DEV_DEMO_TAB" class="flex-1 min-h-0">
        <DemoTab
          :config="config"
          :personas="personas"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :assistant-agent-id="assistantAgentId"
          @update:config-tab="$emit('update:configTab', $event)"
          @update:persona-editor-id="$emit('update:personaEditorId', $event)"
        />
      </div>

      <div v-else-if="props.configTab === 'remoteIm'" class="flex-1 min-h-0">
        <RemoteImTab
          :config="config"
          :personas="personas"
          :persona-avatar-url-map="props.personaAvatarUrlMap"
          :save-config-action="saveConfigAction"
          :set-status-action="setStatusAction"
        />
      </div>

      <SettingsStickyLayout v-else>
          <WelcomeTab
            v-if="props.configTab === 'welcome'"
            :config="config"
            @jump="$emit('update:configTab', $event)"
            @start-chat="$emit('start-chat')"
          />

          <HotkeyTab
            v-else-if="props.configTab === 'hotkey'"
            :config="config"
            :hotkey-test-recording="hotkeyTestRecording"
            :hotkey-test-recording-ms="hotkeyTestRecordingMs"
            :hotkey-test-audio-ready="hotkeyTestAudioReady"
            :microphone-permission-state="microphonePermissionState"
            :microphone-permission-requesting="microphonePermissionRequesting"
            :background-voice-screenshot-keywords="backgroundVoiceScreenshotKeywords"
            :background-voice-screenshot-mode="backgroundVoiceScreenshotMode"
            @start-hotkey-record-test="$emit('startHotkeyRecordTest')"
            @stop-hotkey-record-test="$emit('stopHotkeyRecordTest')"
            @play-hotkey-record-test="$emit('playHotkeyRecordTest')"
            @request-microphone-permission="$emit('requestMicrophonePermission')"
            @capture-hotkey="$emit('captureHotkey', $event)"
            @update:record-hotkey="onRecordHotkeyChanged"
            @update:record-background-wake-enabled="onRecordBackgroundWakeChanged"
            @update:min-record-seconds="onMinRecordSecondsChanged"
            @update:max-record-seconds="onMaxRecordSecondsChanged"
            @update:background-voice-screenshot-keywords="$emit('update:backgroundVoiceScreenshotKeywords', $event)"
            @update:background-voice-screenshot-mode="$emit('update:backgroundVoiceScreenshotMode', $event)"
            @patch-chat-settings="$emit('patchChatSettings', $event)"
          />

          <ChatSettingsTab
            v-else-if="props.configTab === 'chatSettings'"
            :config="config"
            :text-capable-api-configs="textCapableApiConfigs"
            :image-capable-api-configs="imageCapableApiConfigs"
            :stt-capable-api-configs="sttCapableApiConfigs"
            :response-style-options="responseStyleOptions"
            :response-style-id="responseStyleId"
            :pdf-read-mode="pdfReadMode"
            :instruction-presets="instructionPresets"
            :tool-statuses="toolStatuses"
            :saving-config="savingConfig"
            :save-config-action="saveConfigAction"
            @update:response-style-id="$emit('update:responseStyleId', $event)"
            @update:pdf-read-mode="$emit('update:pdfReadMode', $event)"
            @update:instruction-presets="$emit('update:instructionPresets', $event)"
            @patch-conversation-api-settings="$emit('patchConversationApiSettings', $event)"
            @patch-chat-settings="$emit('patchChatSettings', $event)"
          />
          <NotificationTab
            v-else-if="props.configTab === 'notification'"
            :config="config"
            :saving-config="savingConfig"
            :save-config-action="saveConfigAction"
            :last-saved-config-json="lastSavedConfigJson"
          />
          <NetworkAccessTab
            v-else-if="props.configTab === 'networkAccess'"
            :config="config"
            :saving-config="savingConfig"
            :save-config-action="saveConfigAction"
            :last-saved-config-json="lastSavedConfigJson"
          />
          <UsageTab
            v-else-if="props.configTab === 'usage'"
          />

          <MemoryTab
            v-else-if="props.configTab === 'memory'"
            :sync-locked="memorySyncLocked"
            @sync-lock-change="onMemorySyncLockChange"
          />

          <TaskTab
            v-else-if="props.configTab === 'task'"
            :config="config"
            :personas="personas"
            :persona-avatar-url-map="props.personaAvatarUrlMap"
          />

          <LogTab
            v-else-if="props.configTab === 'logs'"
            :config="config"
            :open-runtime-logs="() => $emit('openRuntimeLogs')"
            :open-conversation-list="() => $emit('openConversationList')"
            :open-prompt-preview="() => $emit('openPromptPreview')"
            :open-system-prompt-preview="() => $emit('openSystemPromptPreview')"
            :save-config-action="saveConfigAction"
          />

          <AppearanceTab
            v-else-if="props.configTab === 'appearance'"
            :ui-language="uiLanguage"
            :ui-font="uiFont"
            :code-font="codeFont"
            :locale-options="localeOptions"
            :current-theme="currentTheme"
            :theme-mode="themeMode"
            :auto-light-theme="autoLightTheme"
            :auto-dark-theme="autoDarkTheme"
            :generated-theme-controls="generatedThemeControls"
            :generated-theme-tokens="generatedThemeTokens"
            :generated-light-tokens="generatedLightTokens"
            :generated-dark-tokens="generatedDarkTokens"
            :ui-size-scale="uiSizeScale"
            @update:ui-language="$emit('update:uiLanguage', $event)"
            @update:ui-font="$emit('update:uiFont', $event)"
            @update:code-font="$emit('update:codeFont', $event)"
            @update:ui-size-scale="$emit('update:uiSizeScale', $event)"
            @set-theme="$emit('setTheme', $event)"
            @set-theme-mode="$emit('setThemeMode', $event)"
            @set-auto-theme="(side, value) => $emit('setAutoTheme', side, value)"
            @activate-generated-theme="$emit('activateGeneratedTheme')"
            @update-generated-theme-controls="$emit('updateGeneratedThemeControls', $event)"
            @reset-generated-theme="$emit('resetGeneratedTheme')"
          />

          <StorageTab
            v-else-if="props.configTab === 'migration'"
          />

          <AboutTab
            v-else-if="props.configTab === 'about'"
            :github-update-method="props.config.githubUpdateMethod || 'auto'"
            :checking-update="checkingUpdate"
            :current-theme="currentTheme"
            @update:github-update-method="$emit('update:githubUpdateMethod', $event)"
            @check-update="$emit('checkUpdate')"
            @open-github="$emit('openGithub')"
          />
      </SettingsStickyLayout>
        </div>
      </Transition>
    </div>
    </div>
  </div>
  </Transition>

    <!-- Dialogs -->

    <input ref="avatarFileInput" type="file" accept="image/*" class="hidden" @change="onAvatarFilePicked" />
    <dialog ref="avatarEditorDialog" class="modal">
    <div class="modal-box p-3 max-w-sm">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.editAvatar") }}</h3>
      <div class="rounded border border-base-300 bg-base-100 p-3">
        <div class="flex items-center gap-3">
          <div v-if="avatarEditorAvatarUrl" class="avatar">
            <div class="w-14 rounded-full">
              <img :src="avatarEditorAvatarUrl" :alt="avatarEditorName" :title="avatarEditorName" />
            </div>
          </div>
          <div v-else class="avatar placeholder">
            <div class="bg-neutral text-neutral-content w-14 rounded-full">
              <span>{{ avatarInitial(avatarEditorName) }}</span>
            </div>
          </div>
          <div class="text-sm opacity-70 break-all">{{ avatarEditorName }}</div>
        </div>
        <div class="mt-3 flex gap-2">
          <button class="btn btn-sm" :disabled="!avatarEditorTargetId || avatarSaving" @click="openAvatarPickerForEditor">{{ t("config.persona.uploadAvatar") }}</button>
          <button class="btn btn-sm btn-ghost" :disabled="!avatarEditorTargetHasAvatar || avatarSaving" @click="clearAvatarFromEditor">{{ t("config.persona.clearAvatar") }}</button>
        </div>
        <div class="mt-2 text-xs opacity-60">{{ t("config.persona.pasteImageHint") }}</div>
        <div v-if="cropError || avatarError" class="mt-2 text-sm text-error break-all">{{ cropError || avatarError }}</div>
      </div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeAvatarEditor">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
    </dialog>
    <AvatarCropDialog
      :open="cropOpen"
      :source="cropSource"
      :saving="avatarSaving"
      :error="cropError || avatarError"
      @update:open="onCropOpenChange"
      @confirm="onCropConfirmed"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ChatSettingsPatch, ConversationApiSettingsPatch, PersonaProfile, PromptCommandPreset, ResponseStyleOption, ToolLoadStatus } from "../../../types/app";
import type { GeneratedThemeControls, GeneratedThemeTokens, ThemeMode, ThemeModeKind } from "../../shell/theme/theme-types";
import type { StatusTone } from "../../shell/composables/use-app-core";
import SettingsStickyLayout from "../components/SettingsStickyLayout.vue";
import AvatarCropDialog from "../components/AvatarCropDialog.vue";
import WelcomeTab from "./config-tabs/WelcomeTab.vue";
import HotkeyTab from "./config-tabs/HotkeyTab.vue";
import ApiTab from "./config-tabs/ApiTab.vue";
import McpTab from "./config-tabs/McpTab.vue";
import SkillTab from "./config-tabs/SkillTab.vue";
import CatalogTab from "./config-tabs/CatalogTab.vue";
import PersonaTab from "./config-tabs/PersonaTab.vue";
import DemoTab from "./config-tabs/DemoTab.vue";
import ChatSettingsTab from "./config-tabs/ChatSettingsTab.vue";
import NotificationTab from "./config-tabs/NotificationTab.vue";
import NetworkAccessTab from "./config-tabs/NetworkAccessTab.vue";
import RemoteImTab from "./config-tabs/RemoteImTab.vue";
import UsageTab from "./config-tabs/UsageTab.vue";
import MemoryTab from "./config-tabs/MemoryTab.vue";
import TaskTab from "./config-tabs/TaskTab.vue";
import LogTab from "./config-tabs/LogTab.vue";
import AppearanceTab from "./config-tabs/AppearanceTab.vue";
import StorageTab from "./config-tabs/StorageTab.vue";
import AboutTab from "./config-tabs/AboutTab.vue";
import SimpleSetupPanel from "./config-tabs/SimpleSetupPanel.vue";
import { toErrorMessage } from "../../../utils/error";
import { ArrowLeftRight, Beaker, Bell, ChevronRight, ClipboardList, Code, Cpu, Database, Home, Info, Keyboard, Menu, Palette, Puzzle, Radio, ScrollText, Star, Store, User, Wifi } from "@lucide/vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import { useUnsavedChangesGuard } from "../composables/use-unsaved-changes-guard";

type ConfigTab = "welcome" | "hotkey" | "api" | "mcp" | "skill" | "catalog" | "persona" | "demo" | "chatSettings" | "notification" | "networkAccess" | "remoteIm" | "usage" | "memory" | "task" | "logs" | "appearance" | "migration" | "about";
type AvatarTarget = { agentId: string };
type ConfigNavItem = {
  tab: ConfigTab;
  icon: Component;
  labelKey?: string;
  label?: string;
  devOnly?: boolean;
};
type ConfigNavGroup = {
  id: string;
  titleKey: string;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
  items: ConfigNavItem[];
};
const SHOW_DEV_DEMO_TAB = import.meta.env.DEV;

const CONFIG_NAV_GROUPS: ConfigNavGroup[] = [
  {
    id: "general",
    titleKey: "config.navGroups.general",
    items: [
      { tab: "welcome", icon: Home, labelKey: "config.tabs.welcome" },
      { tab: "chatSettings", icon: Star, labelKey: "config.tabs.chatSettings" },
      { tab: "appearance", icon: Palette, labelKey: "config.tabs.appearance" },
      { tab: "hotkey", icon: Keyboard, labelKey: "config.tabs.hotkey" },
      { tab: "notification", icon: Bell, labelKey: "config.tabs.notification" },
    ],
  },
  {
    id: "ai",
    titleKey: "config.navGroups.ai",
    items: [
      { tab: "api", icon: Cpu, labelKey: "config.tabs.api" },
      { tab: "mcp", icon: Puzzle, labelKey: "config.tabs.mcp" },
      { tab: "skill", icon: Code, labelKey: "config.tabs.skill" },
      { tab: "catalog", icon: Store, labelKey: "config.tabs.catalog" },
    ],
  },
  {
    id: "agents",
    titleKey: "config.navGroups.agents",
    items: [
      { tab: "persona", icon: User, labelKey: "config.tabs.persona" },
    ],
  },
  {
    id: "remote",
    titleKey: "config.navGroups.remote",
    items: [
      { tab: "networkAccess", icon: Wifi, labelKey: "config.tabs.networkAccess" },
      { tab: "remoteIm", icon: Radio, labelKey: "config.tabs.remoteIm" },
    ],
  },
  {
    id: "data",
    titleKey: "config.navGroups.data",
    collapsible: true,
    defaultCollapsed: true,
    items: [
      { tab: "memory", icon: Database, labelKey: "config.tabs.memory" },
      { tab: "task", icon: ClipboardList, labelKey: "config.tabs.task" },
      { tab: "usage", icon: ScrollText, labelKey: "config.tabs.usage" },
    ],
  },
  {
    id: "system",
    titleKey: "config.navGroups.system",
    collapsible: true,
    defaultCollapsed: true,
    items: [
      { tab: "migration", icon: ArrowLeftRight, labelKey: "config.tabs.migration" },
      { tab: "logs", icon: ScrollText, labelKey: "config.tabs.logs" },
      { tab: "about", icon: Info, labelKey: "config.tabs.about" },
      { tab: "demo", icon: Beaker, labelKey: "config.tabs.demo", devOnly: true },
    ],
  },
];

const CONFIG_NAV_ITEMS: ConfigNavItem[] = CONFIG_NAV_GROUPS.flatMap((group) => group.items);

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  simpleSetupMode?: boolean;
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  uiFont?: string;
  codeFont?: string;
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
  themeMode: ThemeModeKind;
  autoLightTheme: string;
  autoDarkTheme: string;
  generatedThemeControls: GeneratedThemeControls;
  generatedThemeTokens: GeneratedThemeTokens;
  generatedLightTokens: GeneratedThemeTokens;
  generatedDarkTokens: GeneratedThemeTokens;
  uiSizeScale: number;
  selectedApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshOk: boolean;
  modelRefreshError: string;
  toolStatuses: ToolLoadStatus[];
  personas: PersonaProfile[];
  personaAvatarUrlMap: Record<string, string>;
  assistantPersonas: PersonaProfile[];
  userPersona: PersonaProfile | null;
  personaEditorId: string;
  assistantAgentId: string;
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  pdfReadMode: "text" | "image";
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  instructionPresets: PromptCommandPreset[];
  selectedPersona: PersonaProfile | null;
  toolPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  userPersonaAvatarUrl: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  avatarSaving: boolean;
  avatarError: string;
  personaSaving: boolean;
  personaDirty: boolean;
  configDirty: boolean;
  savingConfig: boolean;
  normalizeApiBindingsAction: () => void;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
  microphonePermissionState: "granted" | "denied" | "prompt" | "unsupported" | "unknown";
  microphonePermissionRequesting: boolean;
  checkingUpdate: boolean;
  hasAvailableUpdate: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  updateRecordHotkeyAction: (value: string) => Promise<boolean> | boolean;
  updateRecordBackgroundWakeEnabledAction: (value: boolean) => Promise<boolean> | boolean;
  restoreConfigAction: () => boolean;
  lastSavedConfigJson: string;
  setStatusAction: (text: string, tone?: StatusTone) => void;
  savePersonaRelations?: (updates: { agentId: string; childAgentIds: string[] }[]) => Promise<boolean>;
}>();

const emit = defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:simpleSetupMode", value: boolean): void;
  (e: "update:uiLanguage", value: string): void;
  (e: "update:uiFont", value: string): void;
  (e: "update:codeFont", value: string): void;
  (e: "update:uiSizeScale", value: number): void;
  (e: "update:githubUpdateMethod", value: AppConfig["githubUpdateMethod"]): void;
  (e: "update:personaEditorId", value: string): void;
  (e: "update:responseStyleId", value: string): void;
  (e: "update:pdfReadMode", value: "text" | "image"): void;
  (e: "update:backgroundVoiceScreenshotKeywords", value: string): void;
  (e: "update:backgroundVoiceScreenshotMode", value: "desktop" | "focused_window"): void;
  (e: "update:instructionPresets", value: PromptCommandPreset[]): void;
  (e: "patchConversationApiSettings", value: ConversationApiSettingsPatch): void;
  (e: "patchChatSettings", value: ChatSettingsPatch): void;
  (e: "setTheme", value: string): void;
  (e: "setThemeMode", value: ThemeModeKind): void;
  (e: "setAutoTheme", side: ThemeMode, value: string): void;
  (e: "activateGeneratedTheme"): void;
  (e: "updateGeneratedThemeControls", value: Partial<GeneratedThemeControls>): void;
  (e: "resetGeneratedTheme"): void;
  (e: "refreshModels"): void;
  (e: "openMemoryViewer"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "resetPersonas"): void;
  (e: "savePersonas"): void;
  (e: "convertPrivatePersonaToPublic", agentId: string): void;
  (e: "importPersonaMemories", value: { agentId: string; file: File }): void;
  (e: "openConversationList"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "openRuntimeLogs"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
  (e: "requestMicrophonePermission"): void;
  (e: "captureHotkey", value: string): void;
  (e: "saveAgentAvatar", value: { agentId: string; mime: string; bytesBase64: string }): void;
  (e: "clearAgentAvatar", value: { agentId: string }): void;
  (e: "checkUpdate"): void;
  (e: "openGithub"): void;
  (e: "start-chat"): void;
}>();

const { t } = useI18n();
const unsavedGuard = useUnsavedChangesGuard();

const avatarFileInput = ref<HTMLInputElement | null>(null);
const avatarEditorDialog = ref<HTMLDialogElement | null>(null);
const cropSource = ref("");
const cropOpen = ref(false);
const cropError = ref("");
const avatarEditorTargetId = ref("");
const configDrawerOpen = ref(false);
const memorySyncLocked = ref(false);
let cropTarget: AvatarTarget | null = null;
const MIN_RECORD_SECONDS = 1;
const MAX_MIN_RECORD_SECONDS = 30;
const MAX_RECORD_SECONDS = 600;
const collapsedNavGroups = ref<Record<string, boolean>>({
  data: true,
  system: true,
});

function isNavGroupCollapsed(group: ConfigNavGroup): boolean {
  if (!group.collapsible) return false;
  return !!collapsedNavGroups.value[group.id];
}

function toggleNavGroup(group: ConfigNavGroup) {
  if (!group.collapsible) return;
  collapsedNavGroups.value[group.id] = !collapsedNavGroups.value[group.id];
}

function groupHasBadge(group: { items: ConfigNavItem[] }): boolean {
  return group.items.some((item) => item.tab === "about" && props.hasAvailableUpdate);
}

function ensureActiveNavGroupExpanded(tab: ConfigTab) {
  const group = CONFIG_NAV_GROUPS.find((g) => g.items.some((item) => item.tab === tab));
  if (group && collapsedNavGroups.value[group.id]) {
    collapsedNavGroups.value[group.id] = false;
  }
}

watch(
  () => props.configTab,
  (newTab) => {
    ensureActiveNavGroupExpanded(newTab);
  },
  { immediate: true },
);

const visibleConfigNavGroups = computed(() =>
  CONFIG_NAV_GROUPS.map((group) => ({
    ...group,
    items: group.items.filter((item) => !item.devOnly || SHOW_DEV_DEMO_TAB),
  })).filter((group) => group.items.length > 0),
);

const visibleConfigNavItems = computed(() =>
  visibleConfigNavGroups.value.flatMap((group) => group.items),
);
const activeConfigNavItem = computed(() =>
  visibleConfigNavItems.value.find((item) => item.tab === props.configTab)
  ?? visibleConfigNavItems.value.find((item) => item.tab === "welcome")
  ?? null,
);
const activeConfigTabTitle = computed(() => {
  const item = activeConfigNavItem.value;
  if (!item) return "";
  return item.labelKey ? t(item.labelKey) : (item.label || "");
});

function isConfigNavItemLocked(tab: ConfigTab): boolean {
  return memorySyncLocked.value && tab !== "memory";
}

function configNavLinkClass(tab: ConfigTab) {
  return {
    active: props.configTab === tab,
    "menu-active": props.configTab === tab,
    "opacity-50 pointer-events-none": isConfigNavItemLocked(tab),
  };
}

async function selectConfigNavTab(tab: ConfigTab) {
  if (isConfigNavItemLocked(tab)) return;
  await requestTabChange(tab);
  configDrawerOpen.value = false;
}

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function openAvatarPicker(target: AvatarTarget) {
  cropTarget = target;
  if (avatarFileInput.value) {
    avatarFileInput.value.value = "";
    avatarFileInput.value.click();
  }
}

function openAvatarEditorForSelected() {
  if (!props.selectedPersona) return;
  avatarEditorTargetId.value = props.selectedPersona.id;
  cropTarget = { agentId: props.selectedPersona.id };
  cropError.value = "";
  avatarEditorDialog.value?.showModal();
}

function closeAvatarEditor() {
  avatarEditorDialog.value?.close();
}

function openAvatarPickerForEditor() {
  if (!avatarEditorTargetId.value) return;
  openAvatarPicker({ agentId: avatarEditorTargetId.value });
}

function ensureEditorCropTarget() {
  if (cropTarget || !avatarEditorTargetId.value) return;
  cropTarget = { agentId: avatarEditorTargetId.value };
}

function clearAvatarFromEditor() {
  if (!avatarEditorTargetId.value) return;
  emit("clearAgentAvatar", { agentId: avatarEditorTargetId.value });
}

function avatarById(id: string): PersonaProfile | null {
  return props.personas.find((p) => p.id === id) ?? null;
}

const avatarEditorTarget = () => avatarById(avatarEditorTargetId.value);

const avatarEditorName = computed(() => avatarEditorTarget()?.name || t("config.persona.avatarFallbackName"));
const avatarEditorAvatarUrl = computed(() => {
  const target = avatarEditorTarget();
  if (!target) return "";
  if (target.id === props.userPersona?.id) return props.userPersonaAvatarUrl;
  if (target.id === props.selectedPersona?.id) return props.selectedPersonaAvatarUrl;
  return "";
});
const avatarEditorTargetHasAvatar = computed(() => !!avatarEditorTarget()?.avatarPath);

async function readFileAsDataUrl(file: File): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function loadImage(dataUrl: string): Promise<HTMLImageElement> {
  return await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("load image failed"));
    img.src = dataUrl;
  });
}

async function downscaleDataUrl(dataUrl: string, maxSide = 1024): Promise<string> {
  const img = await loadImage(dataUrl);
  const w = img.naturalWidth || img.width;
  const h = img.naturalHeight || img.height;
  if (w <= maxSide && h <= maxSide) return dataUrl;
  const scale = Math.min(1, maxSide / Math.max(w, h));
  const targetW = Math.max(1, Math.round(w * scale));
  const targetH = Math.max(1, Math.round(h * scale));
  const canvas = document.createElement("canvas");
  canvas.width = targetW;
  canvas.height = targetH;
  const ctx = canvas.getContext("2d");
  if (!ctx) return dataUrl;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, 0, 0, targetW, targetH);
  return canvas.toDataURL("image/webp", 0.9);
}

function onCropOpenChange(open: boolean) {
  cropOpen.value = open;
  if (!open) {
    // 裁剪目标随本次裁剪作废：下次选图或粘贴重新认领，避免头像存到上一个人格
    cropTarget = null;
  }
}

function onCropConfirmed(payload: { mime: string; bytesBase64: string }) {
  if (!cropTarget) return;
  emit("saveAgentAvatar", { agentId: cropTarget.agentId, ...payload });
}

// `config` is a shared reactive object from the root app state.
// Direct mutation here is intentional and immediately reflected upstream.
async function onRecordHotkeyChanged(value: string) {
  const next = String(value || "").trim();
  if (props.config.recordHotkey === next) return;
  await Promise.resolve(props.updateRecordHotkeyAction(next));
}

async function onRecordBackgroundWakeChanged(value: boolean) {
  const next = !!value;
  if (!!props.config.recordBackgroundWakeEnabled === next) return;
  await Promise.resolve(props.updateRecordBackgroundWakeEnabledAction(next));
}

function onMinRecordSecondsChanged(value: number) {
  const next = Math.max(MIN_RECORD_SECONDS, Math.min(MAX_MIN_RECORD_SECONDS, Math.round(Number(value) || MIN_RECORD_SECONDS)));
  props.config.minRecordSeconds = next;
  if (props.config.maxRecordSeconds < next) {
    props.config.maxRecordSeconds = next;
  }
}

function onMaxRecordSecondsChanged(value: number) {
  const next = Math.max(
    props.config.minRecordSeconds,
    Math.min(MAX_RECORD_SECONDS, Math.round(Number(value) || props.config.minRecordSeconds)),
  );
  props.config.maxRecordSeconds = next;
}

async function requestTabChange(nextTab: ConfigTab) {
  if (memorySyncLocked.value && nextTab !== "memory") {
    return;
  }
  const targetTab = (!SHOW_DEV_DEMO_TAB && nextTab === "demo") ? "hotkey" : nextTab;
  if (targetTab === props.configTab) {
    return;
  }
  const allow = await unsavedGuard.confirmLeaveIfDirty();
  if (!allow) return;

  emit("update:configTab", targetTab);
}

function onMemorySyncLockChange(locked: boolean) {
  memorySyncLocked.value = !!locked;
}

async function onAvatarFilePicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  void processAvatarFile(file);
}

async function processAvatarFile(file: File) {
  ensureEditorCropTarget();
  if (!cropTarget) return;
  cropError.value = "";
  try {
    const dataUrl = await readFileAsDataUrl(file);
    cropSource.value = await downscaleDataUrl(dataUrl, 1024);
    cropOpen.value = true;
  } catch (e) {
    cropError.value = t("config.persona.avatarReadFailed", { err: String(e) });
  }
}

function handleAvatarPaste(event: ClipboardEvent) {
  if (!avatarEditorDialog.value?.open) return;
  const items = event.clipboardData?.items;
  if (!items || items.length === 0) {
    cropError.value = t("config.persona.pasteNoImage");
    return;
  }
  const imageItem = Array.from(items).find((item) => item.type.startsWith("image/"));
  if (!imageItem) {
    cropError.value = t("config.persona.pasteNoImage");
    return;
  }
  const file = imageItem.getAsFile();
  if (!file) {
    cropError.value = t("config.persona.pasteReadFailed");
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  void processAvatarFile(file);
}

onMounted(() => {
  window.addEventListener("paste", handleAvatarPaste);
});

onBeforeUnmount(() => {
  window.removeEventListener("paste", handleAvatarPaste);
});
</script>
