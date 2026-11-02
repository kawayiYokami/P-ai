import type { QuestionItem } from "../components/chat-question-panel-types";

/** 后端 `ConversationQuestionView` 的前端形状：只有编号，没有内部 id。 */
export type ConversationQuestionView = {
  number: number;
  title: string;
  background?: string;
  options: string[];
};

/**
 * 会话问题 → 面板条目。
 *
 * 编号是会话内的稳定标识，不回收也不保证连续，必须原样带出：
 * 库里是 #2 / #5 / #7，面板就显示 #2 / #5 / #7，绝不能按下标重编。
 * 选项 id 取数组下标，答题命令按 `optionIndex` 回传。
 */
export function buildConversationQuestionItems(
  items: ConversationQuestionView[],
): QuestionItem[] {
  return (Array.isArray(items) ? items : []).map((item) => ({
    id: String(item.number),
    title: `#${item.number} ${item.title}`,
    description: String(item.background ?? "").trim() || undefined,
    options: (Array.isArray(item.options) ? item.options : []).map((label, index) => ({
      id: String(index),
      label,
      kind: "direct" as const,
    })),
  }));
}

/** 会话问题的已选答复：键是题目编号字符串，与面板 `QuestionItem.id` 对齐。 */
export type ConversationQuestionAnswerMap = Record<
  string,
  { optionId: string; label: string; comment: string }
>;

/**
 * 题目列表变化后裁剪已选答复：只保留仍然存在的题目。
 *
 * 拒绝单题、以及工具自行增删题都走这里：用户拒掉一题不该把别的题已经选好的选项一起打回重选。
 * 按编号对齐是安全的，因为编号不回收也不复用（后端有测试盯着）。
 */
export function pruneConversationQuestionAnswers(
  items: ConversationQuestionView[],
  answers: ConversationQuestionAnswerMap,
): ConversationQuestionAnswerMap {
  const alive = new Set((Array.isArray(items) ? items : []).map((item) => String(item.number)));
  const kept: ConversationQuestionAnswerMap = {};
  for (const [key, value] of Object.entries(answers ?? {})) {
    if (alive.has(key)) kept[key] = value;
  }
  return kept;
}

/** 面板答复 → 答题命令参数：编号与选项下标都按字符串回传，这里只做数字转换与非法项过滤。 */
export function buildConversationQuestionAnswerPayload(
  answers: Array<{ id: string; optionId: string }>,
): Array<{ number: number; optionIndex: number }> {
  const parseIndex = (raw: string): number => {
    const text = String(raw ?? "").trim();
    if (!text) return Number.NaN;
    const value = Number(text);
    return Number.isFinite(value) ? value : Number.NaN;
  };
  return (Array.isArray(answers) ? answers : [])
    .map((answer) => ({
      number: parseIndex(answer.id),
      optionIndex: parseIndex(answer.optionId),
    }))
    .filter(
      (answer) =>
        Number.isInteger(answer.number)
        && answer.number >= 1
        && Number.isInteger(answer.optionIndex)
        && answer.optionIndex >= 0,
    );
}
