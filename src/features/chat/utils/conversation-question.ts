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

/**
 * 会话问题条目的落地清洗。
 *
 * 编号是题目唯一对外引用（后端不回收、不复用），缺号或非正整数的条目留着也点不了，
 * 只会让面板多出一个无法作答的空格子，直接丢掉。其余字段只做类型兜底，不改内容。
 *
 * 快照（切会话）与推送（工具改题、答题、拒绝）都先过这里，两条来路得到同一个形状。
 */
export function normalizeConversationQuestions(raw: unknown): ConversationQuestionView[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item: any) => ({
      number: Number(item?.number),
      title: String(item?.title ?? ""),
      background: String(item?.background ?? ""),
      options: (Array.isArray(item?.options) ? item.options : []).map((option: any) =>
        String(option ?? "")),
    }))
    .filter((item) => Number.isInteger(item.number) && item.number >= 1);
}

/** 会话问题的已选答复：键是题目编号字符串，与面板 `QuestionItem.id` 对齐。 */
export type ConversationQuestionAnswerMap = Record<
  string,
  { optionId: string; label: string; comment: string; customText?: string }
>;

/**
 * 题目内容指纹：只比「这一题自己长什么样」，不含顺序与总数。
 *
 * 面板折叠着的时候靠它判断「题目到底有没有被更新」，所以选项的每个可见字段都要进指纹：
 * 只比题干会漏掉「换了选项」这一类改动。
 */
export function conversationQuestionFingerprints(items: QuestionItem[]): Map<string, string> {
  const fingerprints = new Map<string, string>();
  for (const item of Array.isArray(items) ? items : []) {
    const options = (item.options ?? [])
      .map((option) => [
        option.id,
        option.label,
        option.kind,
        option.placeholder ?? "",
        option.inputRequired ? "1" : "0",
      ].join("\u0001"))
      .join("\u0002");
    fingerprints.set(
      item.id,
      [item.title, item.description ?? "", item.previewText ?? "", options].join("\u0003"),
    );
  }
  return fingerprints;
}

/**
 * 题目是否有更新：新题进来、或已有题目的内容被改，都算更新；只有题目减少或换了顺序不算。
 *
 * 面板据此决定折叠着的时候要不要自己展开。答掉/拒掉一题也是「列表变了」，
 * 但那正是用户刚做完的动作，再把自己弹回来只是打扰。
 */
export function conversationQuestionsGotUpdates(
  previous: Map<string, string>,
  items: QuestionItem[],
): boolean {
  for (const [id, fingerprint] of conversationQuestionFingerprints(items)) {
    const before = previous.get(id);
    if (before === undefined || before !== fingerprint) return true;
  }
  return false;
}

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

/** 一条答题入参：自己写的内容与选项下标只能出现一个，后端也按同一口径校验。 */
export type ConversationQuestionAnswerPayload = {
  number: number;
  optionIndex?: number;
  customText?: string;
};

/**
 * 面板答复 → 答题命令参数：编号与选项下标都按字符串回传，这里只做数字转换与非法项过滤。
 * 自己写的答案优先：有内容就只带 `customText`，不带选项下标。
 */
export function buildConversationQuestionAnswerPayload(
  answers: Array<{ id: string; optionId: string; customText?: string }>,
): ConversationQuestionAnswerPayload[] {
  const parseIndex = (raw: string): number => {
    const text = String(raw ?? "").trim();
    if (!text) return Number.NaN;
    const value = Number(text);
    return Number.isFinite(value) ? value : Number.NaN;
  };
  const payload: ConversationQuestionAnswerPayload[] = [];
  for (const answer of Array.isArray(answers) ? answers : []) {
    const number = parseIndex(answer.id);
    if (!Number.isInteger(number) || number < 1) continue;
    const customText = String(answer.customText ?? "").trim();
    if (customText) {
      payload.push({ number, customText });
      continue;
    }
    const optionIndex = parseIndex(answer.optionId);
    if (!Number.isInteger(optionIndex) || optionIndex < 0) continue;
    payload.push({ number, optionIndex });
  }
  return payload;
}

/**
 * 把已选的答复拼成一条用户消息的正文。
 *
 * 拼装放在前端、不放后端，是因为答复必须能立刻发出去：若等后端把文本回传过来，
 * 发送就得排在一次会话写操作的往返之后。这里用的题干与选项都来自后端下发的视图，
 * 与用户点的是同一份数据；后端仍然独立校验编号与选项下标，不信任这段文本。
 *
 * 题目按编号升序排列；非法或已不在列表里的答案直接跳过，全部非法时返回空串（不发送）。
 */
export function buildConversationQuestionAnswerText(
  items: ConversationQuestionView[],
  answers: Array<{ id: string; optionId: string; customText?: string }>,
  supplement?: string,
): string {
  const byNumber = new Map<number, ConversationQuestionView>();
  for (const item of Array.isArray(items) ? items : []) {
    if (Number.isInteger(item?.number)) byNumber.set(item.number, item);
  }
  const blocks: Array<{ number: number; text: string }> = [];
  // 空串不能当 0 号选项：`Number("")` 是 0，直接转会把「没选」误读成第一个选项。
  const parseIndex = (raw: string | undefined): number => {
    const text = String(raw ?? "").trim();
    if (!text) return Number.NaN;
    const value = Number(text);
    return Number.isInteger(value) ? value : Number.NaN;
  };
  for (const answer of Array.isArray(answers) ? answers : []) {
    const number = parseIndex(answer?.id);
    if (!Number.isInteger(number)) continue;
    const question = byNumber.get(number);
    if (!question) continue;
    const customText = String(answer.customText ?? "").trim();
    let answerText = customText;
    if (!answerText) {
      const optionIndex = parseIndex(answer.optionId);
      if (!Number.isInteger(optionIndex)) continue;
      const label = question.options?.[optionIndex];
      if (typeof label !== "string") continue;
      answerText = label.trim();
    }
    if (!answerText) continue;
    blocks.push({
      number,
      text: `【${String(question.title ?? "").trim()}】\n→ ${answerText}`,
    });
  }
  blocks.sort((left, right) => left.number - right.number);
  const text = blocks.map((block) => block.text).join("\n\n");
  if (!text) return "";
  // 补充说明只进这条消息正文，不进后端答案记录：它是给模型的额外一句话，不是某一题的答案。
  const extra = String(supplement ?? "").trim();
  return extra ? `${text}\n\n【补充】\n→ ${extra}` : text;
}
