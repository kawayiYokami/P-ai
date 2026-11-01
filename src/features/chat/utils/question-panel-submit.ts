import type { QuestionAnswer, QuestionItem, QuestionOption } from "../components/chat-question-panel-types";

export type QuestionSubmitPayloadItem = {
  id: string;
  optionId: string;
  label: string;
  comment: string;
};

/** 题目没有显式选项时，沿用面板的默认两选项（与终端审批一致）。 */
export function resolveQuestionOptions(item: QuestionItem): QuestionOption[] {
  if (Array.isArray(item.options) && item.options.length > 0) return item.options;
  return [
    {
      id: "deny",
      label: "拒绝",
      kind: "withInput",
      placeholder: "补充说明（拒绝必填）",
      inputRequired: true,
    },
    { id: "approve", label: "同意", kind: "direct" },
  ];
}

/** 必填补充说明没填时本题不算已选。 */
export function questionAnswerIsComplete(item: QuestionItem, answer: QuestionAnswer): boolean {
  const option = resolveQuestionOptions(item).find((item) => item.id === answer.optionId);
  if (!option) return false;
  if (option.kind === "withInput" && option.inputRequired) {
    return String(answer.comment ?? "").trim().length > 0;
  }
  return true;
}

/** 本题是否已经是一份可提交的答案（选了选项、必填说明也填好了）。 */
export function questionItemIsAnswered(
  item: QuestionItem,
  answers: Record<string, QuestionAnswer>,
): boolean {
  const answer = answers[item.id];
  if (!answer) return false;
  return questionAnswerIsComplete(item, answer);
}

/** 已选且完整的题目条数：面板用它决定发送按钮文案与可用态。 */
export function countAnsweredQuestions(
  items: QuestionItem[],
  answers: Record<string, QuestionAnswer>,
): number {
  return items.filter((item) => questionItemIsAnswered(item, answers)).length;
}

function toPayloadItem(item: QuestionItem, answer: QuestionAnswer): QuestionSubmitPayloadItem {
  return {
    id: item.id,
    optionId: answer.optionId,
    label: answer.label,
    comment: answer.comment,
  };
}

/**
 * 终端审批（`submitMode: "all"`）：全部题目都要有完整答案。
 * 任意一题没选或缺必填说明就返回 `null`，绝不提交半套答案。
 */
export function buildQuestionSubmitPayload(
  items: QuestionItem[],
  answers: Record<string, QuestionAnswer>,
): QuestionSubmitPayloadItem[] | null {
  if (items.length === 0) return null;
  const payload: QuestionSubmitPayloadItem[] = [];
  for (const item of items) {
    const answer = answers[item.id];
    if (!answer || !questionAnswerIsComplete(item, answer)) return null;
    payload.push(toPayloadItem(item, answer));
  }
  return payload;
}

/**
 * 会话提问（`submitMode: "manual"`）：只发已选的题。
 * 未选与未填完必填说明的题直接跳过，不拦截整批；一题都没选时返回 `null`。
 * 顺序按 `items` 原有顺序，与用户的点选先后无关。
 */
export function buildSelectedQuestionSubmitPayload(
  items: QuestionItem[],
  answers: Record<string, QuestionAnswer>,
): QuestionSubmitPayloadItem[] | null {
  const payload: QuestionSubmitPayloadItem[] = [];
  for (const item of items) {
    const answer = answers[item.id];
    if (!answer || !questionAnswerIsComplete(item, answer)) continue;
    payload.push(toPayloadItem(item, answer));
  }
  return payload.length > 0 ? payload : null;
}
