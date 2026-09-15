import { invokeTauri } from "../../../services/tauri-api";
import { useAppLifecycle } from "../../shell/composables/use-app-lifecycle";
import { useAppWatchers } from "../../shell/composables/use-app-watchers";

export function useChatWindowLifecycleSetup(bindings: Record<string, any>) {
  useAppLifecycle({
      appBootstrapMount: bindings.appBootstrap.mount,
      appBootstrapUnmount: bindings.appBootstrap.unmount,
      restoreThemeFromStorage: bindings.restoreThemeFromStorage,
      onPaste: bindings.onPaste,
      onDragOver: bindings.onDragOver,
      onDrop: bindings.onDrop,
      onTransportFileDrop: bindings.onTransportFileDrop,
      onNativeDragState: (active) => {
        bindings.mediaDragActive.value = active;
      },
      recordHotkeyMount: bindings.recordHotkey.mount,
      recordHotkeyUnmount: bindings.recordHotkey.unmount,
      prepareInitialData: bindings.ensureMessageStoreMigrationGate,
      refreshAllViewData: bindings.refreshAllViewData,
      afterRefreshData: () => {
        bindings.startupDataReady.value = true;
        if (bindings.viewMode.value === "chat") {
          // 不阻塞启动遮罩：未归档会话概览后台加载，失败仅告警
          void bindings.refreshChatUnarchivedConversations().catch((error: unknown) => {
            console.warn("[聊天追踪][会话概览] 启动数据就绪后加载失败", error);
          });
        }
      },
      viewMode: bindings.viewMode,
      syncWindowControlsState: bindings.syncWindowControlsState,
      stopRecording: bindings.stopRecording,
      cleanupSpeechRecording: bindings.cleanupSpeechRecording,
      cleanupChatMedia: bindings.cleanupChatMedia,
      onBackendReadyChange: (ready) => {
        bindings.startupOverlayVisible.value = !ready;
      },
      onStartupStepFailed: (label, error) => {
        bindings.setStatus(`启动步骤失败：${label}：${bindings.formatRequestFailed(error)}`);
      },
      afterInitialDataReady: () => {
        // 不阻塞启动遮罩：通知后端启动后台服务即可，预热在后台完成
        void invokeTauri<boolean>("remoteIm.services.start")
          .then((started) => {
            console.info("[启动] 迁移后会话准备完成，已通知后端启动后台服务", { started });
          })
          .catch((error: unknown) => {
            console.warn("[启动] 通知后端启动后台服务失败", error);
          });
      },
      afterMountedReady: async () => {
        void bindings.refreshGithubUpdateState();
      },
  });
  useAppWatchers({
    config: bindings.config,
    configTab: bindings.configTab,
    viewMode: bindings.viewMode,
    personas: bindings.personas,
    userPersona: bindings.userPersona,
    assistantPersonas: bindings.assistantPersonas,
    assistantAgentId: bindings.assistantAgentId,
    personaEditorId: bindings.personaEditorId,
    selectedApiConfig: bindings.selectedApiConfig,
    toolApiConfig: bindings.toolApiConfig,
    modelRefreshError: bindings.modelRefreshError,
    toolStatuses: bindings.toolStatuses,
    defaultApiTools: bindings.defaultApiTools,
    t: bindings.tr,
    normalizeApiBindingsLocal: bindings.normalizeApiBindingsLocal,
    syncUserAliasFromPersona: bindings.syncUserAliasFromPersona,
    syncTrayIcon: bindings.syncTrayIcon,
    refreshToolsStatus: bindings.refreshToolsStatus,
  });
}
