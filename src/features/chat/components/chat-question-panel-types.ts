/**
 * 提问卡的公共类型。
 *
 * 单独放在 `.ts` 而不是留在 `ChatQuestionPanel.vue` 里：`.ts` 工具文件从 `.vue`
 * 导入具名类型过不了 `tsc`（`pnpm build` 会用 tsc，只有 vue-tsc 认这种导出），
 * 类型放这里，组件与工具函数都从这里取，两边不会再分叉。
 */
export type QuestionOptionKind = "direct" | "withInput";

export type QuestionOption = {
  id: string;
  label: string;
  kind: QuestionOptionKind;
  placeholder?: string;
  inputRequired?: boolean;
};

export type QuestionItem = {
  id: string;
  title: string;
  description?: string;
  previewText?: string;
  options?: QuestionOption[];
  canRememberWorkspace?: boolean;
  workspaceLabel?: string;
};

export type QuestionAnswer = {
  optionId: string;
  label: string;
  comment: string;
  /** 用户自己写的答案：非空时它就是这道题的答案，不对应任何给定选项。 */
  customText?: string;
};
