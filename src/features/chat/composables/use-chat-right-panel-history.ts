import { computed, ref, watch } from "vue";
import type { ChatRightPanelMode } from "./chat-ui-layout-storage";

/**
 * 右面板「浏览到的一页」：面板模式 + 模式内定位到的会话。
 * 文件页不带目标——具体打开哪个文件由文件阅读器自己维持，
 * 这里只负责把「当时在看文件页」这件事记下来。
 */
export type ChatRightPanelView = {
  mode: ChatRightPanelMode;
  conversationId: string;
};

/** 栈上限：浏览历史是给人来回切的，不需要无限长。 */
const MAX_HISTORY_ENTRIES = 30;

function isSameView(left: ChatRightPanelView | undefined, right: ChatRightPanelView): boolean {
  if (!left) return false;
  return left.mode === right.mode && left.conversationId === right.conversationId;
}

/**
 * 右面板的浏览历史：像浏览器一样后退/前进。
 *
 * 记录的是「看过的页面」而不是某一次点击——从队伍对话点进文件页，后退回队伍对话，
 * 前进再回到文件页。切到新页面时截断前进分支，与浏览器一致。
 */
export function useChatRightPanelHistory(options: {
  currentView: () => ChatRightPanelView;
  /** 应用某个历史视图；返回 false 表示这一页已经打不开（会话被删一类），调用方跳过它。 */
  applyView: (view: ChatRightPanelView) => boolean;
}) {
  const entries = ref<ChatRightPanelView[]>([]);
  const cursor = ref(-1);
  /** 回放期间视图的变化不能当成新的一次浏览记进栈。 */
  let replaying: ChatRightPanelView | null = null;

  watch(
    options.currentView,
    (next) => {
      if (replaying && isSameView(replaying, next)) {
        replaying = null;
        return;
      }
      replaying = null;
      if (isSameView(entries.value[cursor.value], next)) return;
      // 从中间某页切到新页面：后面的前进分支作废
      const kept = entries.value.slice(0, cursor.value + 1);
      kept.push(next);
      entries.value = kept.length > MAX_HISTORY_ENTRIES
        ? kept.slice(kept.length - MAX_HISTORY_ENTRIES)
        : kept;
      cursor.value = entries.value.length - 1;
    },
    { immediate: true },
  );

  const canGoBack = computed(() => cursor.value > 0);
  const canGoForward = computed(() => cursor.value >= 0 && cursor.value < entries.value.length - 1);

  function applyAt(target: number): boolean {
    const view = entries.value[target];
    if (!view) return false;
    replaying = view;
    if (!options.applyView(view)) {
      replaying = null;
      return false;
    }
    cursor.value = target;
    return true;
  }

  /** 往回退：遇到已经打不开的页面就继续往前退。 */
  function goBack() {
    for (let target = cursor.value - 1; target >= 0; target -= 1) {
      if (applyAt(target)) return;
    }
  }

  function goForward() {
    for (let target = cursor.value + 1; target < entries.value.length; target += 1) {
      if (applyAt(target)) return;
    }
  }

  return { canGoBack, canGoForward, goBack, goForward };
}
