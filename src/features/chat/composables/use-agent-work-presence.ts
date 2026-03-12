import { computed, ref } from "vue";
import type { AgentWorkSignalPayload, DelegateConversationSummary } from "../../../types/app";

const AGENT_WORK_TTL_MS = 10 * 60 * 1000;
const AGENT_WORK_PRUNE_INTERVAL_MS = 30 * 1000;

type AgentWorkRegistry = Record<string, Record<string, number>>;

function trim_text(value: unknown): string {
  return String(value || "").trim();
}

function pruneExpiredRegistry(source: AgentWorkRegistry, now: number): AgentWorkRegistry {
  const next: AgentWorkRegistry = {};
  for (const [agentId, jobs] of Object.entries(source)) {
    const aliveJobs = Object.entries(jobs).filter(([, expiresAt]) => Number(expiresAt) > now);
    if (aliveJobs.length <= 0) continue;
    next[agentId] = Object.fromEntries(aliveJobs);
  }
  return next;
}

function hasAnyActiveJob(source: AgentWorkRegistry): boolean {
  return Object.values(source).some((jobs) => Object.keys(jobs).length > 0);
}

export function useAgentWorkPresence() {
  const workRegistry = ref<AgentWorkRegistry>({});
  let pruneTimer: ReturnType<typeof window.setInterval> | null = null;

  function commitRegistry(next: AgentWorkRegistry) {
    workRegistry.value = next;
    if (hasAnyActiveJob(next)) {
      if (pruneTimer === null) {
        pruneTimer = window.setInterval(() => {
          const pruned = pruneExpiredRegistry(workRegistry.value, Date.now());
          if (pruned !== workRegistry.value) {
            workRegistry.value = pruned;
          }
          if (!hasAnyActiveJob(pruned) && pruneTimer !== null) {
            window.clearInterval(pruneTimer);
            pruneTimer = null;
          }
        }, AGENT_WORK_PRUNE_INTERVAL_MS);
      }
    } else if (pruneTimer !== null) {
      window.clearInterval(pruneTimer);
      pruneTimer = null;
    }
  }

  function markAgentWorkStarted(payload: AgentWorkSignalPayload) {
    const agentId = trim_text(payload.agentId);
    const delegateId = trim_text(payload.delegateId);
    if (!agentId || !delegateId) return;
    const now = Date.now();
    const next = pruneExpiredRegistry(workRegistry.value, now);
    next[agentId] = {
      ...(next[agentId] || {}),
      [delegateId]: now + AGENT_WORK_TTL_MS,
    };
    commitRegistry(next);
  }

  function markAgentWorkStopped(payload: AgentWorkSignalPayload) {
    const agentId = trim_text(payload.agentId);
    const delegateId = trim_text(payload.delegateId);
    if (!agentId || !delegateId) return;
    const next = pruneExpiredRegistry(workRegistry.value, Date.now());
    if (!next[agentId] || !(delegateId in next[agentId])) {
      commitRegistry(next);
      return;
    }
    delete next[agentId][delegateId];
    if (Object.keys(next[agentId]).length <= 0) {
      delete next[agentId];
    }
    commitRegistry(next);
  }

  function seedFromDelegateConversations(items: DelegateConversationSummary[]) {
    if (!Array.isArray(items) || items.length <= 0) return;
    const now = Date.now();
    const next = pruneExpiredRegistry(workRegistry.value, now);
    for (const item of items) {
      const agentId = trim_text(item.agentId);
      const delegateId = trim_text(item.delegateId || item.conversationId);
      if (!agentId || !delegateId) continue;
      next[agentId] = {
        ...(next[agentId] || {}),
        [delegateId]: now + AGENT_WORK_TTL_MS,
      };
    }
    commitRegistry(next);
  }

  const activeWorkCountsByAgent = computed<Record<string, number>>(() => {
    const next: Record<string, number> = {};
    for (const [agentId, jobs] of Object.entries(workRegistry.value)) {
      const count = Object.keys(jobs).length;
      if (count > 0) next[agentId] = count;
    }
    return next;
  });

  function activeWorkCountForAgent(agentId: string): number {
    return Math.max(0, Number(activeWorkCountsByAgent.value[trim_text(agentId)] || 0));
  }

  function cleanup() {
    if (pruneTimer !== null) {
      window.clearInterval(pruneTimer);
      pruneTimer = null;
    }
    workRegistry.value = {};
  }

  return {
    activeWorkCountsByAgent,
    activeWorkCountForAgent,
    markAgentWorkStarted,
    markAgentWorkStopped,
    seedFromDelegateConversations,
    cleanup,
  };
}
