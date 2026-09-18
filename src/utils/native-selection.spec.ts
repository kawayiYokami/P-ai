import { afterAll, beforeAll, describe, expect, it } from "vitest";
import {
  EDITABLE_SELECTOR,
  INTERACTIVE_TRIGGER_SELECTOR,
  shouldClearSelectionForTarget,
} from "./native-selection";

// 测试环境是 node，没有 DOM；这里用最小替身表达「元素自身语义 + 祖先链」，
// 只实现判定逻辑真正用到的 matches / closest / control 三件事。
class StubElement {
  tokens: string[] = [];
  editable = false;
  control: StubElement | null = null;
  parent: StubElement | null = null;

  matches(selector: string): boolean {
    if (this.editable && selector === EDITABLE_SELECTOR) return true;
    const parts = selector.split(",").map((part) => part.trim());
    return this.tokens.some((token) => parts.includes(token));
  }

  closest(selector: string): StubElement | null {
    let node: StubElement | null = this;
    while (node) {
      if (node.matches(selector)) return node;
      node = node.parent;
    }
    return null;
  }
}

type StubOptions = {
  tokens?: string[];
  editable?: boolean;
  control?: StubElement | null;
  parent?: StubElement | null;
};

function makeElement(options: StubOptions = {}): StubElement {
  const element = new StubElement();
  element.tokens = options.tokens ?? [];
  element.editable = options.editable ?? false;
  element.control = options.control ?? null;
  element.parent = options.parent ?? null;
  return element;
}

// 替身只实现判定逻辑用到的 Element 语义，其余事件相关成员与判定无关。
function asTarget(element: StubElement | null): EventTarget | null {
  return element as unknown as EventTarget | null;
}

function shouldClear(element: StubElement | null): boolean {
  return shouldClearSelectionForTarget(asTarget(element));
}

const realElement = globalThis.Element;

beforeAll(() => {
  (globalThis as { Element?: unknown }).Element = StubElement;
});

afterAll(() => {
  (globalThis as { Element?: unknown }).Element = realElement;
});

describe("原生选区清理触发面", () => {
  it("点击交互元素时清理选区", () => {
    expect(shouldClear(makeElement({ tokens: ["button"] }))).toBe(true);
    expect(shouldClear(makeElement({ tokens: ["summary"] }))).toBe(true);
    expect(shouldClear(makeElement({ tokens: ["[role='button']"] }))).toBe(true);
    expect(shouldClear(makeElement({ tokens: ["a[href]"] }))).toBe(true);
    expect(shouldClear(makeElement({ tokens: ["input[type='submit']"] }))).toBe(true);
  });

  it("交互元素自身的触发面完整覆盖声明", () => {
    const declared = INTERACTIVE_TRIGGER_SELECTOR.split(",").map((part) => part.trim());
    expect(declared).toContain("button");
    expect(declared).toContain("a[href]");
    expect(declared).toContain("summary");
    expect(declared).toContain("[role='button']");
  });

  it("点击可编辑元素时不清理选区", () => {
    expect(shouldClear(makeElement({ editable: true }))).toBe(false);
  });

  it("可编辑元素内部的点击同样不清理选区", () => {
    const editable = makeElement({ editable: true });
    const inner = makeElement({ parent: editable });
    expect(shouldClear(inner)).toBe(false);
  });

  it("preserve 标记内的元素不清理选区", () => {
    const preserve = makeElement({ tokens: ["[data-preserve-native-selection]"] });
    expect(shouldClear(preserve)).toBe(false);
    const inner = makeElement({ tokens: ["button"], parent: preserve });
    expect(shouldClear(inner)).toBe(false);
  });

  it("label 关联可编辑控件时不清理选区", () => {
    const label = makeElement({ tokens: ["label"], control: makeElement({ editable: true }) });
    expect(shouldClear(label)).toBe(false);
    const inner = makeElement({ parent: label });
    expect(shouldClear(inner)).toBe(false);
  });

  it("label 未关联可编辑控件时清理选区", () => {
    expect(shouldClear(makeElement({ tokens: ["label"] }))).toBe(true);
    const label = makeElement({ tokens: ["label"], control: makeElement({ tokens: ["button"] }) });
    expect(shouldClear(label)).toBe(true);
  });

  it("非元素目标不清理选区", () => {
    expect(shouldClear(null)).toBe(false);
    expect(shouldClearSelectionForTarget({} as EventTarget)).toBe(false);
  });

  it("普通容器内的点击不清理选区", () => {
    expect(shouldClear(makeElement({ tokens: ["div"] }))).toBe(false);
  });
});
