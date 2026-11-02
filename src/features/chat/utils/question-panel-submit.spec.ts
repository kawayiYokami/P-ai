import { describe, expect, it } from "vitest";
import type { QuestionAnswer, QuestionItem } from "../components/chat-question-panel-types";
import {
  buildQuestionSubmitPayload,
  buildSelectedQuestionSubmitPayload,
  countAnsweredQuestions,
  questionAnswerIsComplete,
  questionItemIsAnswered,
  resolveQuestionOptions,
} from "./question-panel-submit";

function item(id: string, options?: QuestionItem["options"]): QuestionItem {
  return { id, title: `题 ${id}`, options };
}

function answer(optionId: string, label: string, comment = ""): QuestionAnswer {
  return { optionId, label, comment };
}

const directOptions = (a: string, b: string): QuestionItem["options"] => [
  { id: "0", label: a, kind: "direct" },
  { id: "1", label: b, kind: "direct" },
];

describe("resolveQuestionOptions", () => {
  it("题目没有选项时回落到默认两选项", () => {
    const options = resolveQuestionOptions(item("q1"));
    expect(options.map((option) => option.id)).toEqual(["deny", "approve"]);
  });

  it("题目自带选项时原样使用", () => {
    const options = resolveQuestionOptions(item("q1", directOptions("发布", "等待")));
    expect(options.map((option) => option.id)).toEqual(["0", "1"]);
  });
});

describe("questionAnswerIsComplete", () => {
  it("必填补充说明为空时不算完整", () => {
    const withInput = item("q1", [
      { id: "deny", label: "拒绝", kind: "withInput", inputRequired: true },
    ]);
    expect(questionAnswerIsComplete(withInput, answer("deny", "拒绝", "   "))).toBe(false);
    expect(questionAnswerIsComplete(withInput, answer("deny", "拒绝", "先别发"))).toBe(true);
  });

  it("选项不存在时视为不完整", () => {
    expect(questionAnswerIsComplete(item("q1"), answer("missing", "x"))).toBe(false);
  });

  it("自己写的答案有内容就算完整，空白不算", () => {
    const q = item("q1", directOptions("发布", "等待"));
    expect(
      questionAnswerIsComplete(q, { optionId: "", label: "灰度发布", comment: "", customText: "灰度发布" }),
    ).toBe(true);
    expect(
      questionAnswerIsComplete(q, { optionId: "", label: "", comment: "", customText: "   " }),
    ).toBe(false);
  });
});

describe("questionItemIsAnswered / countAnsweredQuestions", () => {
  const items = [item("2", directOptions("现在发布", "再等等")), item("5", directOptions("main", "release"))];

  it("选了选项才算已选", () => {
    expect(questionItemIsAnswered(items[0]!, {})).toBe(false);
    expect(questionItemIsAnswered(items[0]!, { "2": answer("0", "现在发布") })).toBe(true);
  });

  it("只统计已选且填好的题", () => {
    expect(countAnsweredQuestions(items, {})).toBe(0);
    expect(countAnsweredQuestions(items, { "2": answer("0", "现在发布") })).toBe(1);
    expect(
      countAnsweredQuestions(items, {
        "2": answer("0", "现在发布"),
        "5": answer("1", "release"),
      }),
    ).toBe(2);
  });

  it("withInput 必填未填时不计入已选", () => {
    const withInput = [
      item("7", [{ id: "deny", label: "拒绝", kind: "withInput", inputRequired: true }]),
    ];
    expect(countAnsweredQuestions(withInput, { "7": answer("deny", "拒绝", "") })).toBe(0);
    expect(countAnsweredQuestions(withInput, { "7": answer("deny", "拒绝", "先别发") })).toBe(1);
  });
});

describe("buildSelectedQuestionSubmitPayload（manual：选了的就发）", () => {
  const items = [
    item("2", directOptions("现在发布", "再等等")),
    item("5", directOptions("main", "release")),
    item("7", directOptions("我来", "你来")),
  ];

  it("只选一题就只发一题", () => {
    expect(buildSelectedQuestionSubmitPayload(items, { "5": answer("1", "release") })).toEqual([
      { id: "5", optionId: "1", label: "release", comment: "" },
    ]);
  });

  it("选三题就发三题，且按题目原有顺序而不是点选顺序", () => {
    const payload = buildSelectedQuestionSubmitPayload(items, {
      "7": answer("0", "我来"),
      "2": answer("0", "现在发布"),
      "5": answer("1", "release"),
    });
    expect(payload?.map((entry) => entry.id)).toEqual(["2", "5", "7"]);
  });

  it("未选的题不进 payload，也不拦截整批", () => {
    const payload = buildSelectedQuestionSubmitPayload(items, {
      "2": answer("0", "现在发布"),
      "7": answer("1", "你来"),
    });
    expect(payload?.map((entry) => entry.id)).toEqual(["2", "7"]);
  });

  it("一题都没选时不发送", () => {
    expect(buildSelectedQuestionSubmitPayload(items, {})).toBeNull();
  });

  it("withInput 必填未填的题被跳过，其余照发", () => {
    const mixed = [
      item("2", directOptions("现在发布", "再等等")),
      item("7", [{ id: "deny", label: "拒绝", kind: "withInput", inputRequired: true }]),
    ];
    const payload = buildSelectedQuestionSubmitPayload(mixed, {
      "2": answer("0", "现在发布"),
      "7": answer("deny", "拒绝", "  "),
    });
    expect(payload?.map((entry) => entry.id)).toEqual(["2"]);
  });

  it("连发两轮：第一轮发已选的，剩下的下一轮再发各自正确", () => {
    const firstPayload = buildSelectedQuestionSubmitPayload(items, {
      "2": answer("0", "现在发布"),
      "5": answer("1", "release"),
    });
    expect(firstPayload?.map((entry) => entry.id)).toEqual(["2", "5"]);

    const answered = new Set(firstPayload?.map((entry) => entry.id));
    const remaining = items.filter((entry) => !answered.has(entry.id));
    expect(remaining.map((entry) => entry.id)).toEqual(["7"]);

    const secondPayload = buildSelectedQuestionSubmitPayload(remaining, {
      "7": answer("0", "我来"),
    });
    expect(secondPayload).toEqual([{ id: "7", optionId: "0", label: "我来", comment: "" }]);
  });
});

describe("buildQuestionSubmitPayload（all：终端审批不变）", () => {
  const items = [item("2", directOptions("现在发布", "再等等")), item("5", directOptions("main", "release"))];

  it("全部答完时按题目顺序输出", () => {
    expect(
      buildQuestionSubmitPayload(items, {
        "2": answer("0", "现在发布"),
        "5": answer("1", "release"),
      }),
    ).toEqual([
      { id: "2", optionId: "0", label: "现在发布", comment: "" },
      { id: "5", optionId: "1", label: "release", comment: "" },
    ]);
  });

  it("任意一题缺答案时整体不提交", () => {
    expect(buildQuestionSubmitPayload(items, { "2": answer("0", "现在发布") })).toBeNull();
  });

  it("必填说明缺失时整体不提交", () => {
    const withInput = [
      item("2", directOptions("现在发布", "再等等")),
      item("7", [{ id: "deny", label: "拒绝", kind: "withInput", inputRequired: true }]),
    ];
    expect(
      buildQuestionSubmitPayload(withInput, {
        "2": answer("0", "现在发布"),
        "7": answer("deny", "拒绝", ""),
      }),
    ).toBeNull();
  });

  it("空 items 不提交", () => {
    expect(buildQuestionSubmitPayload([], {})).toBeNull();
  });

  it("单题面板（终端审批常见形态）仍然全量提交", () => {
    const single = [item("q1", [{ id: "approve", label: "同意", kind: "direct" }])];
    expect(buildQuestionSubmitPayload(single, { q1: answer("approve", "同意") })).toEqual([
      { id: "q1", optionId: "approve", label: "同意", comment: "" },
    ]);
  });
});
