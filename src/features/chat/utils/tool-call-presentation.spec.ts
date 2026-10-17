import { describe, expect, it } from "vitest";
import { createToolCallPresentation } from "./tool-call-presentation";

function createPresentation(agentNames: Record<string, string> = {}) {
  const dictionary: Record<string, string> = {
    "status.toolTimeline.actionBackground": "后台任务",
    "status.toolTimeline.actionConfig": "配置 PAI",
    "status.toolTimeline.actionConnectMcp": "链接到",
    "status.toolTimeline.actionContactSendFiles": "发送附件",
    "status.toolTimeline.actionDeepRecall": "深度回忆",
    "status.toolTimeline.actionDelegate": "委托",
    "status.toolTimeline.actionDelete": "删除文件",
    "status.toolTimeline.actionExec": "执行命令",
    "status.toolTimeline.actionFetch": "访问网页",
    "status.toolTimeline.actionGetSession": "查询会话",
    "status.toolTimeline.actionGoal": "目标",
    "status.toolTimeline.actionImageEdit": "编辑图片",
    "status.toolTimeline.actionImageGenerate": "生成图片",
    "status.toolTimeline.actionInformSession": "通知会话",
    "status.toolTimeline.actionListDir": "浏览目录",
    "status.toolTimeline.actionMeme": "保存表情",
    "status.toolTimeline.actionMove": "移动文件",
    "status.toolTimeline.actionOperate": "桌面操作",
    "status.toolTimeline.actionPatch": "应用补丁",
    "status.toolTimeline.actionPlan": "计划",
    "status.toolTimeline.actionRead": "阅读",
    "status.toolTimeline.actionRecall": "检索记忆",
    "status.toolTimeline.actionRemember": "记录记忆",
    "status.toolTimeline.actionSearch": "搜索文件",
    "status.toolTimeline.actionTask": "定时任务",
    "status.toolTimeline.actionTeamDismiss": "请离队员",
    "status.toolTimeline.actionTeamInfo": "查看队伍",
    "status.toolTimeline.actionTeamRecruit": "招募队员",
    "status.toolTimeline.actionTeamReport": "汇报队长",
    "status.toolTimeline.actionTeamSend": "发送给队员",
    "status.toolTimeline.teamMemberNumber": "{number} 号队员",
    "status.toolTimeline.teamScopeActive": "当前队员",
    "status.toolTimeline.teamScopeHistory": "历史队员",
    "status.toolTimeline.actionTodo": "待办清单",
    "status.toolTimeline.actionUpdate": "更新",
    "status.toolTimeline.actionWait": "等待",
    "status.toolTimeline.actionWebSearch": "搜索网页",
    "status.toolTimeline.actionWrite": "写入文件",
    "status.toolTimeline.missingArgs": "缺少参数",
  };

  return createToolCallPresentation({
    t: (key, params) => {
      const match = dictionary[key];
      if (match) {
        if (params && Object.keys(params).length > 0) {
          return Object.entries(params).reduce(
            (acc, [name, value]) => acc.replace(new RegExp(`\\{${name}\\}`, "g"), String(value)),
            match,
          );
        }
        return match;
      }
      const name = key.split(".").pop() || key;
      return params && Object.keys(params).length > 0 ? `${name}:${JSON.stringify(params)}` : name;
    },
    agentName: (id) => agentNames[id] || id,
  });
}

describe("createToolCallPresentation", () => {
  it("阅读文件：提取纯文件名、扩展名及行号区间", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "read",
      argsText: JSON.stringify({
        absolute_path: "E:\\github\\easy_call_ai\\src\\agentStore.ts",
        start_line: 1090,
        end_line: 1210,
      }),
    });

    expect(result.action).toBe("阅读");
    expect(result.target).toBe("agentStore.ts");
    expect(result.fileExt).toBe("ts");
    expect(result.lineRange).toBe("#1090-1210");
    expect(result.text).toBe("阅读 agentStore.ts #1090-1210");
  });

  it("阅读文件：支持 offset 和 limit 转化为区间", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "read_file",
      argsText: JSON.stringify({
        path: "/repo/src/components/ChatView.vue",
        offset: 100,
        limit: 50,
      }),
    });

    expect(result.action).toBe("阅读");
    expect(result.target).toBe("ChatView.vue");
    expect(result.fileExt).toBe("vue");
    expect(result.lineRange).toBe("#100-150");
  });

  it("更新文件：提取纯文件名及行数变更统计 (+100 -30)", () => {
    const presentation = createPresentation();
    const oldLines = Array(30).fill("old line").join("\n");
    const newLines = Array(100).fill("new line").join("\n");
    const result = presentation.toolCallSemanticPresentation({
      name: "update",
      argsText: JSON.stringify({
        path: "C:\\projects\\agentStore.ts",
        oldString: oldLines,
        newString: newLines,
      }),
    });

    expect(result.action).toBe("更新");
    expect(result.target).toBe("agentStore.ts");
    expect(result.adds).toBe(100);
    expect(result.removes).toBe(30);
    expect(result.text).toBe("更新 agentStore.ts");
  });

  it("写入文件：提取文件名及增加行数 (+100)", () => {
    const presentation = createPresentation();
    const content = Array(100).fill("const x = 1;").join("\n");
    const result = presentation.toolCallSemanticPresentation({
      name: "write",
      argsText: JSON.stringify({
        target: "src/utils/math.rs",
        content,
      }),
    });

    expect(result.action).toBe("写入文件");
    expect(result.target).toBe("math.rs");
    expect(result.adds).toBe(100);
    expect(result.text).toBe("写入文件 math.rs");
  });

  it("删除文件：提取文件名与删除动作", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "delete",
      argsText: JSON.stringify({
        path: "src/temp.json",
      }),
    });

    expect(result.action).toBe("删除文件");
    expect(result.target).toBe("temp.json");
    expect(result.text).toBe("删除文件 temp.json");
  });

  it("移动文件：提取两端文件名与移动路径", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "move",
      argsText: JSON.stringify({
        from: "src/oldName.ts",
        to: "src/newName.ts",
      }),
    });

    expect(result.action).toBe("移动文件");
    expect(result.target).toBe("oldName.ts → newName.ts");
    expect(result.text).toBe("移动文件 oldName.ts → newName.ts");
  });

  it("执行命令：单行与多行命令精简展示", () => {
    const presentation = createPresentation();
    const single = presentation.toolCallSemanticPresentation({
      name: "exec",
      argsText: JSON.stringify({ command: "git status --short" }),
    });
    expect(single.action).toBe("执行命令");
    expect(single.target).toBe("git status --short");
    expect(single.text).toBe("执行命令 git status --short");

    const multi = presentation.toolCallSemanticPresentation({
      name: "shell_exec",
      argsText: JSON.stringify({ command: "pnpm build\npnpm test\npnpm smoke" }),
    });
    expect(multi.action).toBe("执行命令");
    expect(multi.target).toBe("pnpm build (3 行)");
    expect(multi.text).toBe("执行命令 pnpm build (3 行)");
  });

  it("配置 PAI：提取 command 命令", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "config",
      argsText: JSON.stringify({ command: "help" }),
    });

    expect(result.action).toBe("配置 PAI");
    expect(result.target).toBe("help");
    expect(result.text).toBe("配置 PAI help");
  });

  it("计划：提取计划文件名与动作状态", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "plan",
      argsText: JSON.stringify({ action: "present", path: "plans/20261002_migration.md" }),
    });

    expect(result.action).toBe("计划");
    expect(result.target).toBe("20261002_migration.md");
    expect(result.extra).toBe("提交");
    expect(result.text).toBe("计划 20261002_migration.md 提交");
  });

  it("记忆管理：提取 remember 的 judgment 和 recall 的 query", () => {
    const presentation = createPresentation();
    const memRes = presentation.toolCallSemanticPresentation({
      name: "remember",
      argsText: JSON.stringify({
        action: "create",
        memory: {
          memoryType: "knowledge",
          judgment: "用户偏好紧凑排版与纯文字界面",
        },
      }),
    });
    expect(memRes.action).toBe("记录记忆");
    expect(memRes.target).toBe("用户偏好紧凑排版与纯文字界面");

    const recallRes = presentation.toolCallSemanticPresentation({
      name: "recall",
      argsText: JSON.stringify({ query: "用户偏好" }),
    });
    expect(recallRes.action).toBe("检索记忆");
    expect(recallRes.target).toBe("用户偏好");
  });

  it("桌面操作：提取脚本首步与步骤数", () => {
    const presentation = createPresentation();
    const result = presentation.toolCallSemanticPresentation({
      name: "operate",
      argsText: JSON.stringify({
        script: "mouse left click @960,540\nwait 1\nkey Enter",
      }),
    });

    expect(result.action).toBe("桌面操作");
    expect(result.target).toContain("mouse left click @960,540");
    expect(result.target).toContain("(3 步)");
  });

  it("网络访问与网页搜索", () => {
    const presentation = createPresentation();
    const fetchRes = presentation.toolCallSemanticPresentation({
      name: "fetch",
      argsText: JSON.stringify({ url: "https://tauri.app/v2" }),
    });
    expect(fetchRes.action).toBe("访问网页");
    expect(fetchRes.target).toBe("https://tauri.app/v2");

    const searchRes = presentation.toolCallSemanticPresentation({
      name: "websearch",
      argsText: JSON.stringify({ query: "Tauri 2 release notes" }),
    });
    expect(searchRes.action).toBe("搜索网页");
    expect(searchRes.target).toBe("Tauri 2 release notes");
  });

  it("会话与后台工具：get_session, background, inform_session", () => {
    const presentation = createPresentation();
    const sessRes = presentation.toolCallSemanticPresentation({
      name: "get_session",
      argsText: JSON.stringify({ keyword: "设计讨论" }),
    });
    expect(sessRes.action).toBe("查询会话");
    expect(sessRes.target).toBe("设计讨论");

    const bgRes = presentation.toolCallSemanticPresentation({
      name: "background",
      argsText: JSON.stringify({ action: "status", id: "bg_12345" }),
    });
    expect(bgRes.action).toBe("后台任务");
    expect(bgRes.target).toBe("status bg_12345");

    const notifyRes = presentation.toolCallSemanticPresentation({
      name: "inform_session",
      argsText: JSON.stringify({ session_id: "sess_main", content: "任务已完成" }),
    });
    expect(notifyRes.action).toBe("通知会话");
    expect(notifyRes.target).toBe("sess_main");
  });

  it("深度回忆：deeprecall, deeprecall_search, deeprecall_context", () => {
    const presentation = createPresentation();
    const drRes = presentation.toolCallSemanticPresentation({
      name: "deeprecall",
      argsText: JSON.stringify({ query: "当初的部署方案" }),
    });
    expect(drRes.action).toBe("深度回忆");
    expect(drRes.target).toBe("当初的部署方案");

    const ctxRes = presentation.toolCallSemanticPresentation({
      name: "deeprecall_context",
      argsText: JSON.stringify({ conversation_index: 3, start_index: 10, end_index: 25 }),
    });
    expect(ctxRes.action).toBe("深度回忆");
    expect(ctxRes.target).toBe("会话 3 #10-25");
  });

  it("附件、表情与图片生成编辑：contact_send_files, meme, image_generate, image_edit", () => {
    const presentation = createPresentation();
    const sendRes = presentation.toolCallSemanticPresentation({
      name: "contact_send_files",
      argsText: JSON.stringify({ file_paths: ["C:\\docs\\report.pdf", "C:\\img\\spec.png"] }),
    });
    expect(sendRes.action).toBe("发送附件");
    expect(sendRes.target).toBe("report.pdf, spec.png");

    const memeRes = presentation.toolCallSemanticPresentation({
      name: "meme",
      argsText: JSON.stringify({ emotion: "坏笑", path: "/tmp/evil.png" }),
    });
    expect(memeRes.action).toBe("保存表情");
    expect(memeRes.target).toBe(":坏笑:");

    const genRes = presentation.toolCallSemanticPresentation({
      name: "image_generate",
      argsText: JSON.stringify({ prompt: "未来城市夜景", resolution: "1024x1024" }),
    });
    expect(genRes.action).toBe("生成图片");
    expect(genRes.target).toBe("未来城市夜景");

    const editRes = presentation.toolCallSemanticPresentation({
      name: "image_edit",
      argsText: JSON.stringify({ prompt: "去除背景人物", images: ["/tmp/avatar.png"] }),
    });
    expect(editRes.action).toBe("编辑图片");
    expect(editRes.target).toBe("去除背景人物");
  });

  it("队伍工具：名册、招募、请离、发送与汇报", () => {
    const presentation = createPresentation();

    const infoActive = presentation.toolCallSemanticPresentation({
      name: "team_info",
      argsText: JSON.stringify({ scope: "active" }),
    });
    expect(infoActive.action).toBe("查看队伍");
    expect(infoActive.target).toBe("当前队员");
    expect(infoActive.text).toBe("查看队伍 当前队员");

    const infoHistory = presentation.toolCallSemanticPresentation({
      name: "team_info",
      argsText: JSON.stringify({ scope: "history" }),
    });
    expect(infoHistory.target).toBe("历史队员");

    const recruit = presentation.toolCallSemanticPresentation({
      name: "team_recruit",
      argsText: JSON.stringify({ name: "李工", task: "跟进重构" }),
    });
    expect(recruit.action).toBe("招募队员");
    expect(recruit.target).toBe("李工");
    expect(recruit.text).toBe("招募队员 李工");

    const dismiss = presentation.toolCallSemanticPresentation({
      name: "team_dismiss",
      argsText: JSON.stringify({ number: 2 }),
    });
    expect(dismiss.action).toBe("请离队员");
    expect(dismiss.target).toBe("2 号队员");

    const send = presentation.toolCallSemanticPresentation({
      name: "team_send",
      argsText: JSON.stringify({ number: 1, content: "把登录接口的测试补上" }),
    });
    expect(send.action).toBe("发送给队员");
    expect(send.target).toBe("1 号队员 把登录接口的测试补上");

    const report = presentation.toolCallSemanticPresentation({
      name: "team_report",
      argsText: JSON.stringify({ content: "已完成测试，分支 feature/login-test" }),
    });
    expect(report.action).toBe("汇报队长");
    expect(report.target).toContain("已完成测试");

    expect(presentation.toolCallDisplayName("team_info")).toBe("查看队伍");
    expect(presentation.toolCallDisplayName("team_recruit")).toBe("招募队员");
    expect(presentation.toolCallDisplayName("team_dismiss")).toBe("请离队员");
    expect(presentation.toolCallDisplayName("team_send")).toBe("发送给队员");
    expect(presentation.toolCallDisplayName("team_report")).toBe("汇报队长");
  });

  it("外部 / MCP 工具：统一显示「链接到 {mcp工具名}」", () => {
    const presentation = createPresentation();
    const mcp1 = presentation.toolCallSemanticPresentation({
      name: "gitlab_get_issue",
      argsText: JSON.stringify({ project: "easy_call_ai", issue_id: 42 }),
    });
    expect(mcp1.action).toBe("链接到");
    expect(mcp1.target).toBe("gitlab_get_issue");
    expect(mcp1.text).toBe("链接到 gitlab_get_issue");

    // akasha 与 tavily 作为 MCP 外部工具，正确显示「链接到」
    const akashaRes = presentation.toolCallSemanticPresentation({
      name: "akasha_search",
      argsText: JSON.stringify({ query: "知识库搜索" }),
    });
    expect(akashaRes.action).toBe("链接到");
    expect(akashaRes.target).toBe("akasha_search");
    expect(akashaRes.text).toBe("链接到 akasha_search");

    const tavilyRes = presentation.toolCallSemanticPresentation({
      name: "tavily_search",
      argsText: JSON.stringify({ query: "联网搜索" }),
    });
    expect(tavilyRes.action).toBe("链接到");
    expect(tavilyRes.target).toBe("tavily_search");
    expect(tavilyRes.text).toBe("链接到 tavily_search");

    // toolCallDisplayName 也统一包含「链接到」
    expect(presentation.toolCallDisplayName("gitlab_get_issue")).toBe("链接到 gitlab_get_issue");
    expect(presentation.toolCallDisplayName("akasha_read")).toBe("链接到 akasha_read");
    expect(presentation.toolCallDisplayName("read")).toBe("阅读");
    expect(presentation.toolCallDisplayName("image_generate")).toBe("生成图片");
  });
});
