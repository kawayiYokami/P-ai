import { computed } from "vue";
import { useGithubUpdateView } from "../../shell/composables/use-github-update-view";
import type { GithubUpdateMethod } from "../../../types/app";
import { useAgentWorkPresence } from "./use-agent-work-presence";
import { useArchiveImport } from "./use-archive-import";
import { useArchivesView } from "./use-archives-view";
import { useConversationPlanMode } from "./use-conversation-plan-mode";
import { useConversationWheelchairMode } from "./use-conversation-wheelchair-mode";
import { useGoalTask } from "./use-goal-task";

export function useChatWindowShellDataOrchestrator(bindings: Record<string, any>) {
  const githubUpdate = useGithubUpdateView({
    t: bindings.tr,
    viewMode: bindings.viewMode,
    status: bindings.status,
    updateMethod: computed<GithubUpdateMethod | undefined>(
      () => (bindings.config.githubUpdateMethod || "auto") as GithubUpdateMethod,
    ),
    skippedVersion: computed(() => bindings.config.skippedGithubUpdateVersion || ""),
    onSkippedVersionSaved: (saved) => {
      bindings.config.skippedGithubUpdateVersion = saved.skippedGithubUpdateVersion || "";
    },
  });

  const archivesView = useArchivesView({
    t: bindings.tr,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
  });
  const archiveImport = useArchiveImport({
    buildArchiveImportPreview: archivesView.buildArchiveImportPreview,
    importArchivePayload: archivesView.importArchivePayload,
    setStatusError: bindings.setStatusError,
  });

  const conversationPlanMode = useConversationPlanMode({
    currentConversationId: bindings.currentChatConversationId,
    unarchivedConversations: archivesView.unarchivedConversations,
  });
  const conversationWheelchairMode = useConversationWheelchairMode({
    currentConversationId: bindings.currentChatConversationId,
    unarchivedConversations: archivesView.unarchivedConversations,
  });

  const goalTask = useGoalTask({
    t: bindings.tr,
    currentConversationId: bindings.currentChatConversationId,
    setStatus: bindings.setStatus,
  });

  return {
    githubUpdate,
    archivesView,
    archiveImport,
    conversationPlanMode,
    conversationWheelchairMode,
    goalTask,
    agentWorkPresence: useAgentWorkPresence(),
  };
}
