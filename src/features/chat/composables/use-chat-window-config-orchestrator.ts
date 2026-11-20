import { useConfigCore } from "../../config/composables/use-config-core";
import { useChatConfigActionsGlue } from "./use-chat-config-actions-glue";
import { useChatConfigDerivedState } from "./use-chat-config-derived-state";
import { useChatConfigUiDerivedState } from "./use-chat-config-ui-derived-state";

export function useChatWindowConfigOrchestrator(bindings: Record<string, any>) {
  const configDerived = useChatConfigDerivedState(bindings.config, bindings.currentChatPreferredApiConfigId);
  const configCore = useConfigCore({
    config: bindings.config,
    textCapableApiConfigs: configDerived.textCapableApiConfigs,
    t: bindings.t,
  });
  const configUi = useChatConfigUiDerivedState({
    config: bindings.config,
    apiModelOptions: bindings.apiModelOptions,
    modelRefreshOkFlags: bindings.modelRefreshOkFlags,
    selectedApiConfig: configDerived.selectedApiConfig,
    personas: bindings.personas,
    lastSavedConfigJson: bindings.lastSavedConfigJson,
    lastSavedPersonasJson: bindings.lastSavedPersonasJson,
    buildConfigSnapshotJson: configCore.buildConfigSnapshotJson,
    t: bindings.t,
  });
  const configActions = useChatConfigActionsGlue({
    t: bindings.t,
    config: bindings.config,
    locale: bindings.locale,
    personas: bindings.personas,
    configTab: bindings.configTab,
    lastSavedConfigJson: bindings.lastSavedConfigJson,
    normalizeLocale: bindings.normalizeLocale,
    applyUiLanguage: bindings.applyUiLanguage,
    buildConfigSnapshotJson: configCore.buildConfigSnapshotJson,
    refreshToolsStatus: bindings.refreshToolsStatus,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
  });

  return {
    configDerived,
    configCore,
    configUi,
    configActions,
  };
}
