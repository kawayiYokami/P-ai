import { nextTick, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

type ForceCompactionPreviewResult = {
  canCompact: boolean;
};

type ConfirmPlanSessionContext = {
  messageId: string;
  apiConfigId: string;
  agentId: string;
  departmentId: string;
  conversationId: string;
};

type UseConfirmPlanOptions = {
  currentApiConfigId: Ref<string>;
  currentAgentId: Ref<string>;
  currentDepartmentId: Ref<string>;
  currentConversationId: Ref<string>;
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  compactingConversation: Ref<boolean>;
  setConversationPlanMode: (conversationId: string, value: boolean) => Promise<boolean>;
  forceCompactNow: () => Promise<void>;
  sendChat: (input: {
    text: string;
    displayText: string;
    skipInstructionPrompts?: boolean;
  }) => Promise<void>;
};

export function useConfirmPlan(options: UseConfirmPlanOptions) {
  function currentConfirmPlanSessionContext(messageId: string): ConfirmPlanSessionContext {
    return {
      messageId: String(messageId || "").trim(),
      apiConfigId: String(options.currentApiConfigId.value || "").trim(),
      agentId: String(options.currentAgentId.value || "").trim(),
      departmentId: String(options.currentDepartmentId.value || "").trim(),
      conversationId: String(options.currentConversationId.value || "").trim(),
    };
  }

  function isConfirmPlanSessionStillCurrent(session: ConfirmPlanSessionContext): boolean {
    return (
      session.conversationId === String(options.currentConversationId.value || "").trim()
      && session.apiConfigId === String(options.currentApiConfigId.value || "").trim()
      && session.agentId === String(options.currentAgentId.value || "").trim()
      && session.departmentId === String(options.currentDepartmentId.value || "").trim()
    );
  }

  async function handleConfirmPlan(payload: { messageId: string }) {
    const session = currentConfirmPlanSessionContext(String(payload?.messageId || ""));
    if (
      !session.messageId
      || !session.conversationId
      || options.chatting.value
      || options.forcingArchive.value
      || options.compactingConversation.value
    ) return;
    const planModeDisabled = await options.setConversationPlanMode(session.conversationId, false);
    if (!planModeDisabled) return;
    if (!isConfirmPlanSessionStillCurrent(session)) return;

    if (session.apiConfigId && session.agentId) {
      try {
        const preview = await invokeTauri<ForceCompactionPreviewResult>("preview_force_compact_current", {
          input: {
            apiConfigId: session.apiConfigId,
            agentId: session.agentId,
            departmentId: session.departmentId || null,
            conversationId: session.conversationId,
          },
        });
        if (!isConfirmPlanSessionStillCurrent(session)) return;
        if (preview?.canCompact) {
          await options.forceCompactNow();
          if (!isConfirmPlanSessionStillCurrent(session)) return;
        }
      } catch (error) {
        console.warn("[计划模式] 确认执行前压缩跳过", {
          messageId: session.messageId,
          error,
        });
      }
    }

    await nextTick();
    if (!isConfirmPlanSessionStillCurrent(session)) return;
    await options.sendChat({
      text: "我同意，并执行计划。",
      displayText: "我同意，并执行计划。",
      skipInstructionPrompts: true,
    });
  }

  return {
    handleConfirmPlan,
  };
}
