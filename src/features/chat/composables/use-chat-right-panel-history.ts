import { computed, ref, watch } from "vue";

/**
 * 右面板的一个标签：种类 + 标识。
 *
 * 文件用路径，追问/队员会话用会话 id；首页、工具审查、看板这些页没有标识。
 * 「是不是同一个标签」只看这两个字段是否都相同，与标签种类无关的
 * 压栈、上限、分片、前进后退全在这套结构上做，加新种类只要多一个 kind
 * 和它对应的打开方式。
 */
export type ChatRightPanelTabKind = "home" | "file" | "monitor" | "sideChat" | "teamMember" | "kanban";

export type ChatRightPanelTab = {
  kind: ChatRightPanelTabKind;
  id: string;
};

type StoredHistory = { entries: ChatRightPanelTab[]; cursor: number };

/** 每个会话各记一份，栈上限 30 页：浏览历史是给人来回切的，不需要无限长。 */
const MAX_HISTORY_ENTRIES = 30;
const HISTORY_STORAGE_PREFIX = "easy_call.chat_right_panel_history.v1.";

const TAB_KINDS: ChatRightPanelTabKind[] = ["home", "file", "monitor", "sideChat", "teamMember", "kanban"];

function normalizeTabKind(value: string): ChatRightPanelTabKind | "" {
  const kind = String(value || "").trim() as ChatRightPanelTabKind;
  return TAB_KINDS.includes(kind) ? kind : "";
}

function isSameTab(left: ChatRightPanelTab | undefined, right: ChatRightPanelTab): boolean {
  if (!left) return false;
  return left.kind === right.kind && left.id === right.id;
}

/** 需要定位到具体对象的标签种类：会话看会话 id，文件看路径。 */
function isConversationKind(kind: ChatRightPanelTabKind): boolean {
  return kind === "sideChat" || kind === "teamMember";
}

/** 还没定位到具体对象的标签不算一次浏览：新建页、子会话列表尚未到达、文件面板还没恢复完。 */
function isUnresolvedTab(tab: ChatRightPanelTab): boolean {
  if (isConversationKind(tab.kind) || tab.kind === "file") return !tab.id;
  return false;
}

function loadStoredHistory(scope: string): StoredHistory {
  if (!scope || typeof window === "undefined") return { entries: [], cursor: -1 };
  try {
    const parsed = JSON.parse(window.localStorage.getItem(`${HISTORY_STORAGE_PREFIX}${scope}`) || "null");
    const entries = (Array.isArray(parsed?.entries) ? parsed.entries : [])
      .map((item: any) => ({
        kind: normalizeTabKind(item?.kind),
        id: String(item?.id || "").trim(),
      }))
      .filter((item: { kind: ChatRightPanelTabKind | ""; id: string }) => !!item.kind) as ChatRightPanelTab[];
    const cursor = Number(parsed?.cursor);
    return {
      entries: entries.slice(-MAX_HISTORY_ENTRIES),
      cursor: Number.isInteger(cursor) ? Math.min(Math.max(cursor, -1), entries.length - 1) : entries.length - 1,
    };
  } catch {
    return { entries: [], cursor: -1 };
  }
}

function storeHistory(scope: string, history: StoredHistory) {
  if (!scope || typeof window === "undefined") return;
  window.localStorage.setItem(`${HISTORY_STORAGE_PREFIX}${scope}`, JSON.stringify(history));
}

/**
 * 从某会话的浏览历史里找它最近看的那个子会话标签：
 * 从当前位置往回找第一条追问/队员会话页。刷新、切回父会话时靠它回到原来的标签。
 */
export function rememberedSideConversationInHistory(scope: string): string {
  const { entries, cursor } = loadStoredHistory(String(scope || "").trim());
  for (let index = cursor; index >= 0; index -= 1) {
    const tab = entries[index];
    if (tab && isConversationKind(tab.kind) && tab.id) return tab.id;
  }
  return "";
}

/**
 * 右面板的浏览历史：像浏览器一样后退/前进，按会话各记一份并落 localStorage。
 *
 * 记录的是「看过哪个标签」而不是某一次点击——从队伍对话点进某个文件，
 * 后退回队伍对话，前进再回到那个文件。切到新标签时截断前进分支，与浏览器一致。
 */
export function useChatRightPanelHistory(options: {
  /** 读当前标签；拿不到具体对象（文件面板未就绪等）时 id 留空，这次浏览会被忽略。 */
  currentTab: () => ChatRightPanelTab;
  /** 打开某个标签；返回 false 表示这一页已经打不开（会话被删一类），调用方跳过它。 */
  openTab: (tab: ChatRightPanelTab) => boolean;
  /** 历史归属的会话；为空时只在内存里记，不落盘。 */
  scope?: () => string;
}) {
  const scopeOf = options.scope ?? (() => "");
  const entries = ref<ChatRightPanelTab[]>([]);
  const cursor = ref(-1);
  let activeScope = "";
  /** 回放期间标签的变化不能当成新的一次浏览记进栈。 */
  let replaying: ChatRightPanelTab | null = null;

  function persist() {
    storeHistory(activeScope, { entries: entries.value, cursor: cursor.value });
  }

  watch(
    () => [String(scopeOf() || "").trim(), options.currentTab()] as const,
    ([scope, next]) => {
      if (scope !== activeScope) {
        // 换了会话：换上那个会话自己的历史，不把上一个会话的标签带过去
        activeScope = scope;
        const stored = loadStoredHistory(scope);
        entries.value = stored.entries;
        cursor.value = stored.cursor;
        replaying = null;
        // 这个会话已经有历史：换分片就够了，此刻的当前标签可能还是上一个会话的，
        // 写进来会凭空多一条用户没看过的页
        if (stored.entries.length > 0) return;
        // 第一次进这个会话：以当前标签作为起点，否则第一页就退不回去了
        if (isUnresolvedTab(next)) return;
        entries.value = [next];
        cursor.value = 0;
        persist();
        return;
      }
      if (replaying && isSameTab(replaying, next)) {
        replaying = null;
        return;
      }
      replaying = null;
      if (isUnresolvedTab(next)) return;
      if (isSameTab(entries.value[cursor.value], next)) return;
      // 从中间某页切到新标签：后面的前进分支作废
      const kept = entries.value.slice(0, cursor.value + 1);
      kept.push(next);
      entries.value = kept.length > MAX_HISTORY_ENTRIES
        ? kept.slice(kept.length - MAX_HISTORY_ENTRIES)
        : kept;
      cursor.value = entries.value.length - 1;
      persist();
    },
    { immediate: true },
  );

  const canGoBack = computed(() => cursor.value > 0);
  const canGoForward = computed(() => cursor.value >= 0 && cursor.value < entries.value.length - 1);

  function openAt(target: number): boolean {
    const tab = entries.value[target];
    if (!tab) return false;
    replaying = tab;
    if (!options.openTab(tab)) {
      replaying = null;
      return false;
    }
    cursor.value = target;
    persist();
    return true;
  }

  /** 往回退：遇到已经打不开的标签就继续往前退。 */
  function goBack() {
    for (let target = cursor.value - 1; target >= 0; target -= 1) {
      if (openAt(target)) return;
    }
  }

  function goForward() {
    for (let target = cursor.value + 1; target < entries.value.length; target += 1) {
      if (openAt(target)) return;
    }
  }

  return { canGoBack, canGoForward, goBack, goForward };
}
