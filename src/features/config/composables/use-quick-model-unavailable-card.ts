import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

export type QuickModelUnavailableInfo = {
  /** 头像没生成出来的那个人格；为空时只保存配置、不重试。 */
  agentId: string;
  /** 后端原始错误，交给卡片提炼成一句可读的原因。 */
  error: string;
};

/**
 * 头像生成失败时的「快速模型不可用」卡片状态。
 *
 * 两个宿主共用这一份：招募人格在聊天窗口弹、保存人格在配置窗口弹。
 * 未配置快速模型时后端根本不会发失败事件，所以卡片不会出现——这是「禁用不打扰」的实现方式。
 */
export function useQuickModelUnavailableCard(options: {
  patchConversationApiSettings: (
    patch: { toolReviewApiConfigId: string | null },
  ) => Promise<unknown>;
  /** 卡片里显示「当前用的是哪个」，方便用户知道自己在换掉什么。 */
  currentModelId: () => string;
}) {
  const state = ref<QuickModelUnavailableInfo | null>(null);
  const currentModelId = ref("");

  function show(info: QuickModelUnavailableInfo | null | undefined) {
    const agentId = String(info?.agentId || "").trim();
    if (!agentId) return;
    currentModelId.value = String(options.currentModelId() || "").trim();
    state.value = { agentId, error: String(info?.error || "") };
  }

  function close() {
    state.value = null;
  }

  /**
   * 选中一个新快速模型：先落盘配置，再重试一次头像。
   * 重试失败会由后端再推一次失败事件，卡片自然重新出现，这里不额外提示。
   */
  async function selectModel(modelId: string) {
    const target = state.value;
    const next = String(modelId || "").trim();
    state.value = null;
    if (!next) return;
    try {
      await options.patchConversationApiSettings({ toolReviewApiConfigId: next });
    } catch (error) {
      console.warn("[快速模型卡片] 保存配置失败：", error);
      return;
    }
    if (!target?.agentId) return;
    try {
      await invokeTauri("generate_agent_avatar", { input: { agentId: target.agentId } });
    } catch (error) {
      // 命令入口失败只抛 Err、不发事件（事件只属于后台 enqueue 那条路），
      // 所以这里要自己把卡片放回来：否则换了模型仍然不可用，界面上一点交代都没有。
      const text = String(error ?? "").trim() || "unknown";
      show({ agentId: target.agentId, error: text });
    }
  }

  return { state, currentModelId, show, close, selectModel };
}
