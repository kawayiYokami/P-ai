export type ChatFlowRuntimeEventHandlers = {
  handleExternalAssistantDelta: (payload: unknown) => Promise<void> | void;
  handleExternalHistoryFlushed: (payload: unknown) => Promise<void> | void;
  handleExternalMessageAppended?: (payload: unknown) => Promise<void> | void;
  handleExternalMessagesAfterSynced?: (payload: unknown) => Promise<void> | void;
  handleExternalRoundCompleted: (payload: unknown) => Promise<void> | void;
  handleExternalRoundFailed: (payload: unknown) => Promise<void> | void;
  handleExternalRoundStarted: (payload: unknown) => Promise<void> | void;
  handleExternalRuntimeStateUpdated?: (payload: unknown) => Promise<void> | void;
  handleExternalStreamRebindRequired: (payload: unknown) => Promise<void> | void;
  handleExternalTodosUpdated?: (payload: unknown) => Promise<void> | void;
  handleExternalQuestionItemsUpdated?: (payload: unknown) => Promise<void> | void;
};

type RegisteredChatFlowRuntime = {
  bindingId: string;
  getConversationId: () => string;
  flow: ChatFlowRuntimeEventHandlers;
};

const registeredChatFlowRuntimes = new Map<string, RegisteredChatFlowRuntime>();

function normalized(value: unknown): string {
  return String(value || "").trim();
}

export function registerChatFlowRuntime(input: RegisteredChatFlowRuntime): () => void {
  const bindingId = normalized(input.bindingId);
  if (!bindingId) return () => {};
  registeredChatFlowRuntimes.set(bindingId, {
    bindingId,
    getConversationId: input.getConversationId,
    flow: input.flow,
  });
  return () => {
    const current = registeredChatFlowRuntimes.get(bindingId);
    if (current?.flow === input.flow) {
      registeredChatFlowRuntimes.delete(bindingId);
    }
  };
}

export function chatFlowRuntimesForConversation(conversationId: string): ChatFlowRuntimeEventHandlers[] {
  const targetConversationId = normalized(conversationId);
  if (!targetConversationId) return [];
  const flows: ChatFlowRuntimeEventHandlers[] = [];
  const seen = new Set<ChatFlowRuntimeEventHandlers>();
  for (const runtime of registeredChatFlowRuntimes.values()) {
    if (normalized(runtime.getConversationId()) !== targetConversationId) continue;
    if (seen.has(runtime.flow)) continue;
    seen.add(runtime.flow);
    flows.push(runtime.flow);
  }
  return flows;
}
