import type { Ref } from "vue";

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
  clearForegroundRuntimeState: () => void;
  confirmPlanAndContinue: (input: {
    conversationId: string;
    planMessageId: string;
    departmentId?: string;
    agentId?: string;
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
    options.clearForegroundRuntimeState();
    const planModeDisabled = await options.setConversationPlanMode(session.conversationId, false);
    if (!planModeDisabled) return;
    if (!isConfirmPlanSessionStillCurrent(session)) return;

    try {
      await options.confirmPlanAndContinue({
        conversationId: session.conversationId,
        planMessageId: session.messageId,
        departmentId: session.departmentId || undefined,
        agentId: session.agentId || undefined,
      });
    } catch (error) {
      console.warn("[计划模式] 确认并继续执行失败", {
        messageId: session.messageId,
        error: error instanceof Error
          ? { message: error.message, stack: error.stack }
          : error,
      });
    }
  }

  return {
    handleConfirmPlan,
  };
}
