import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

export type ConversationSearchSourceKind = "localConversation" | "contact";
/** 来源筛选：本地会话 / 远程联系人，二选一，不提供同时搜两边的入口。 */
export type ConversationSearchSourceMode = "localConversation" | "contact";

export type ConversationSearchHit = {
  conversationId: string;
  conversationTitle: string;
  conversationSummaryTitle?: string;
  sourceKind: string;
  agentId: string;
  workspacePath: string;
  speaker: string;
  createdAt: string;
  snippet: string;
  conversationUpdatedAt?: string;
};

export type ConversationSearchStats = {
  scannedConversations: number;
  readConversations: number;
  readMessages: number;
};

export type ConversationSearchAgentOption = { id: string; name: string };
export type ConversationSearchWorkspaceOption = { path: string; label: string };
/** 未输入关键词时展示的近期会话。 */
export type ConversationSearchRecentItem = { conversationId: string; title: string; time: string };

/** 命中片段阈值：后端凑够这么多条就不再扫后面的会话，所以实际会略多。 */
const SEARCH_LIMIT = 10;

function sourceKindsForMode(mode: ConversationSearchSourceMode): ConversationSearchSourceKind[] {
  return [mode];
}

/**
 * 会话搜索页状态。每次搜索都让后端按当前筛选临时建内存索引，检索完即释放；
 * 结果只缓存在这里，切走再回来不重新请求。
 */
export function useConversationSearch() {
  const query = ref("");
  const sourceMode = ref<ConversationSearchSourceMode>("localConversation");
  const agentId = ref("");
  const workspacePath = ref("");
  /** 时间范围筛选：只在最近 N 天内有更新的会话里检索，"0" 表示不限。 */
  const days = ref("30");
  const hits = ref<ConversationSearchHit[]>([]);
  const stats = ref<ConversationSearchStats | null>(null);
  const elapsedMs = ref(0);
  const loading = ref(false);
  const error = ref("");
  const searched = ref(false);

  async function search() {
    const trimmed = query.value.trim();
    if (!trimmed || loading.value) return;
    loading.value = true;
    // 一开始搜就切到结果态：搜索栏上移、结果区显示转圈，而不是停在居中的初始布局里毫无反馈。
    searched.value = true;
    error.value = "";
    try {
      const result = await invokeTauri<{
        hits: ConversationSearchHit[];
        stats: ConversationSearchStats;
        elapsedMs: number;
      }>("search_conversation_messages", {
        input: {
          query: trimmed,
          limit: SEARCH_LIMIT,
          agentId: agentId.value.trim() || null,
          sourceKinds: sourceKindsForMode(sourceMode.value),
          workspacePath: workspacePath.value.trim() || null,
          days: Number(days.value) || 0,
        },
      });
      hits.value = Array.isArray(result?.hits) ? result.hits : [];
      stats.value = result?.stats || null;
      elapsedMs.value = Number(result?.elapsedMs || 0);
      searched.value = true;
    } catch (err) {
      hits.value = [];
      stats.value = null;
      elapsedMs.value = 0;
      searched.value = true;
      error.value = String(err);
    } finally {
      loading.value = false;
    }
  }

  return {
    query,
    sourceMode,
    agentId,
    workspacePath,
    days,
    hits,
    stats,
    elapsedMs,
    loading,
    error,
    searched,
    search,
  };
}
