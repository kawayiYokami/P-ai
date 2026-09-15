import { describe, expect, it } from "vitest";
import { createToolCallPresentation } from "./tool-call-presentation";

function createPresentation(agentNames: Record<string, string> = {}) {
  return createToolCallPresentation({
    t: (key, params) => {
      const name = key.split(".").pop() || key;
      return params && Object.keys(params).length > 0 ? `${name}:${JSON.stringify(params)}` : name;
    },
    agentName: (id) => agentNames[id] || id,
  });
}

describe("createToolCallPresentation", () => {
  it("从 apply_patch 输入提取操作和文件名", () => {
    const presentation = createPresentation();
    const summary = presentation.toolCallSummaryText({
      name: "apply_patch",
      argsText: JSON.stringify({ input: "*** Begin Patch\n*** Update File: src/app.ts\n*** End Patch" }),
    });

    expect(summary).toBe("patchUpdate src/app.ts");
  });

  it("保留 read 工具的路径和分页参数", () => {
    const presentation = createPresentation();
    const summary = presentation.toolCallSummaryText({
      name: "read",
      argsText: JSON.stringify({ absolute_path: "src/app.ts", offset: 20, limit: 40 }),
    });

    expect(summary).toBe("src/app.ts · offset: 20 · limit: 40");
  });

  it("通过注入的人格名称生成委托摘要", () => {
    const presentation = createPresentation({ coding: "开发人格" });
    const summary = presentation.toolCallSummaryText({
      name: "delegate",
      argsText: JSON.stringify({ task_name: "修复", agent_id: "coding", mode: "wait", question: "检查问题" }),
    });

    expect(summary).toContain("开发人格");
    expect(summary).toContain("等待结果");
  });
});
