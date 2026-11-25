import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invokeTauri, onTransportNotification } from "../../../services/tauri-api";

export type HomeAutomationTask = {
  id: string;
  description: string;
};

/**
 * 首页磁贴墙的自动化任务：按当前会话拉，重载广播来了再拉一次。
 *
 * 磁贴只做提示，不可关闭；清单的真源是会话自动化配置，这里不缓存也不合并。
 */
export function useHomeAutomation(activeConversationId: () => string) {
  const homeAutomationTasks = ref<HomeAutomationTask[]>([]);

  async function refreshHomeAutomationTasks() {
    const conversationId = String(activeConversationId() || "").trim();
    if (!conversationId) {
      homeAutomationTasks.value = [];
      return;
    }
    try {
      const list = await invokeTauri<unknown[]>("automation_list_tasks", { conversationId });
      // 拉取期间可能已经切走：回来的结果不属于当前会话就丢掉
      if (String(activeConversationId() || "").trim() !== conversationId) return;
      homeAutomationTasks.value = (Array.isArray(list) ? list : [])
        .map((row) => {
          const item = (row || {}) as Record<string, unknown>;
          return {
            id: String(item.id ?? "").trim(),
            description: String(item.description ?? "").trim(),
          };
        })
        .filter((task) => task.id !== "");
    } catch {
      homeAutomationTasks.value = [];
    }
  }

  let stopAutomationUpdated: (() => void) | null = null;

  watch(
    () => String(activeConversationId() || "").trim(),
    () => {
      void refreshHomeAutomationTasks();
    },
    { immediate: true },
  );

  onMounted(() => {
    stopAutomationUpdated = onTransportNotification<{ conversationId?: string }>(
      "automation.updated",
      (payload) => {
        const updated = String(payload?.conversationId || "").trim();
        if (updated && updated !== String(activeConversationId() || "").trim()) return;
        void refreshHomeAutomationTasks();
      },
    );
  });

  onBeforeUnmount(() => {
    stopAutomationUpdated?.();
    stopAutomationUpdated = null;
  });

  return { homeAutomationTasks, refreshHomeAutomationTasks };
}
