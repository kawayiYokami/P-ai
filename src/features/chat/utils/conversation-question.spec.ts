import { describe, expect, it } from "vitest";
import {
  buildConversationQuestionAnswerPayload,
  buildConversationQuestionItems,
  type ConversationQuestionView,
} from "./conversation-question";

function question(number: number, title: string, options: string[]): ConversationQuestionView {
  return { number, title, options };
}

describe("buildConversationQuestionItems", () => {
  it("编号不连续时按实际编号显示，不按位置重编", () => {
    const items = buildConversationQuestionItems([
      question(2, "要不要现在发布？", ["现在发布", "再等等"]),
      question(5, "用哪个分支？", ["main", "release"]),
      question(7, "谁来回滚？", ["我", "你"]),
    ]);

    expect(items.map((item) => item.id)).toEqual(["2", "5", "7"]);
    expect(items.map((item) => item.title)).toEqual([
      "#2 要不要现在发布？",
      "#5 用哪个分支？",
      "#7 谁来回滚？",
    ]);
  });

  it("选项 id 取数组下标，与答题命令的 optionIndex 对齐", () => {
    const items = buildConversationQuestionItems([
      question(3, "选一个", ["甲", "乙", "丙"]),
    ]);

    expect(items[0]?.options?.map((option) => option.id)).toEqual(["0", "1", "2"]);
    expect(items[0]?.options?.map((option) => option.label)).toEqual(["甲", "乙", "丙"]);
    expect(items[0]?.options?.every((option) => option.kind === "direct")).toBe(true);
  });

  it("背景为空串时不提供 description，避免面板渲染空白段", () => {
    const items = buildConversationQuestionItems([
      { number: 1, title: "题一", background: "   ", options: ["A"] },
      { number: 2, title: "题二", options: [] },
    ]);

    expect(items[0]?.description).toBeUndefined();
    expect(items[1]?.description).toBeUndefined();
  });

  it("空列表原样返回空数组", () => {
    expect(buildConversationQuestionItems([])).toEqual([]);
  });
});

describe("buildConversationQuestionAnswerPayload", () => {
  it("编号与选项下标按字符串回传后转成数字，保持面板给出的编号", () => {
    expect(
      buildConversationQuestionAnswerPayload([
        { id: "5", optionId: "1" },
        { id: "2", optionId: "0" },
      ]),
    ).toEqual([
      { number: 5, optionIndex: 1 },
      { number: 2, optionIndex: 0 },
    ]);
  });

  it("丢掉无法解析成数字的项", () => {
    expect(
      buildConversationQuestionAnswerPayload([
        { id: "", optionId: "0" },
        { id: "3", optionId: "" },
        { id: "4", optionId: "2" },
      ]),
    ).toEqual([{ number: 4, optionIndex: 2 }]);
  });
});
