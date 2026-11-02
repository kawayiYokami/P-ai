import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import DelegateScheduleTimeline from "./DelegateScheduleTimeline.vue";
import { i18n } from "../../../i18n";
import type { ArchiveBlockPage, ScheduleRun } from "../../../types/app";

describe("DelegateScheduleTimeline", () => {
  it("renders thought, semantic tool calls, and final text cleanly", async () => {
    const initialRuns: ScheduleRun[] = [
      {
        runId: "run-1",
        conversationId: "conv-1",
        status: "completed",
        startedAt: "2026-11-02T10:00:00Z",
        updatedAt: "2026-11-02T10:01:14Z",
        elapsedMs: 74000,
        requestCount: 14,
        toolCallCount: 1,
        model: "workbuddy/deepseek-v4.1-flash",
        events: [],
      },
    ];

    const initialBlockPage: ArchiveBlockPage = {
      messages: [
        {
          id: "msg-1",
          role: "assistant",
          parts: [{ type: "text", text: "已为您完成看板要求与规范检索。" }],
          activityItems: [
            {
              kind: "reasoning",
              id: "act-1",
              text: "先检索关于「人格只显示这个人格负责的议题的状态」的要求与规则...",
            },
            {
              kind: "tool",
              id: "act-2",
              name: "deeprecall_search",
              argsText: JSON.stringify({ query: "人格 议题状态" }),
              resultText: "检索结果：包含3条相关规则",
              status: "done",
            },
          ],
        },
      ],
      blocks: [],
      selectedBlockId: 0,
      hasPrevBlock: false,
      hasNextBlock: false,
    };

    const app = createSSRApp({
      render: () =>
        h(DelegateScheduleTimeline, {
          conversationId: "conv-1",
          initialRuns,
          initialBlockPage,
        }),
    });
    app.use(i18n);

    const html = await renderToString(app);

    // 1. 顶部信息
    expect(html).toContain("已完成");
    expect(html).toContain("14 步");
    expect(html).toContain("workbuddy/deepseek-v4.1-flash");

    // 2. 思考与工具
    expect(html).toContain("执行过程");
    expect(html).toContain("深度回忆");
    expect(html).toContain("思考");

    // 3. 最终答复
    expect(html).toContain("最终答复");
    expect(html).toContain("已为您完成看板要求与规范检索。");
  });

  it("falls back to schedule events when blockPage is empty", async () => {
    const initialRuns: ScheduleRun[] = [
      {
        runId: "run-2",
        conversationId: "conv-2",
        status: "running",
        startedAt: "2026-11-02T10:00:00Z",
        updatedAt: "2026-11-02T10:00:10Z",
        elapsedMs: 10000,
        requestCount: 2,
        toolCallCount: 1,
        model: "deepseek-chat",
        events: [
          {
            id: "ev-1",
            runId: "run-2",
            conversationId: "conv-2",
            createdAt: "2026-11-02T10:00:00Z",
            phase: "model_round_end",
            detail: { reasoningPreview: "分析文件结构中" },
            elapsedMs: 100,
            success: true,
          },
          {
            id: "ev-2",
            runId: "run-2",
            conversationId: "conv-2",
            createdAt: "2026-11-02T10:00:01Z",
            phase: "tool_call",
            detail: { toolName: "read_file", argPreview: '{"path":"src/test.ts"}' },
            elapsedMs: 200,
            success: true,
          },
          {
            id: "ev-3",
            runId: "run-2",
            conversationId: "conv-2",
            createdAt: "2026-11-02T10:00:02Z",
            phase: "tool_result",
            detail: { toolName: "read_file", textPreview: "content" },
            elapsedMs: 300,
            success: true,
          },
        ],
      },
    ];

    const initialBlockPage: ArchiveBlockPage = {
      messages: [],
      blocks: [],
      selectedBlockId: 0,
      hasPrevBlock: false,
      hasNextBlock: false,
    };

    const app = createSSRApp({
      render: () =>
        h(DelegateScheduleTimeline, {
          conversationId: "conv-2",
          initialRuns,
          initialBlockPage,
        }),
    });
    app.use(i18n);

    const html = await renderToString(app);

    expect(html).toContain("进行中");
    expect(html).toContain("deepseek-chat");
    expect(html).toContain("执行过程");
    expect(html).toContain("阅读");
    expect(html).toContain("test.ts");
  });
});
