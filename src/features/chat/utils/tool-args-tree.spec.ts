import { describe, expect, it } from "vitest";
import { breakCommandLines, isCommandArgLabel, parseToolArgs, TOOL_ARG_EMPTY_MARK, TOOL_ARG_VALUE_MAX_CHARS } from "./tool-args-tree";

const flat = (text: string) => (parseToolArgs(text) || []).map(
  (line) => `${"  ".repeat(line.depth)}${line.prefix ? `${line.prefix} ` : ""}${line.label ? `${line.label}:` : ""}${line.value ?? ""}`,
);

describe("parseToolArgs", () => {
  it("扁平对象摊成一行一个字段，不带引号与大括号", () => {
    const lines = parseToolArgs('{"issue":9,"mode":"queue"}') || [];
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatchObject({ depth: 0, label: "issue", value: "9" });
    expect(lines[1]).toMatchObject({ depth: 0, label: "mode", value: "queue" });
    expect(flat('{"issue":9,"mode":"queue"}')).toEqual(["issue:9", "mode:queue"]);
  });

  it("嵌套对象下一层，键名保持独立行而不是折成 a.b.c", () => {
    expect(flat('{"a":{"b":1,"c":2}}')).toEqual([
      "a:",
      "  b:1",
      "  c:2",
    ]);
  });

  it("标量数组并成一行", () => {
    expect(flat('{"tags":["x","y",3]}')).toEqual(["tags:x, y, 3"]);
  });

  it("对象数组每项一块，首字段带破折号，其余字段对齐缩进", () => {
    expect(flat('{"list":[{"name":"a","url":"u"},{"name":"b"}]}')).toEqual([
      "list:",
      "  - name:a",
      "    url:u",
      "  - name:b",
    ]);
  });

  it("空对象与空数组显示为空占位，不与数组项破折号混淆", () => {
    expect(flat('{"a":{},"b":[]}')).toEqual([`a:${TOOL_ARG_EMPTY_MARK}`, `b:${TOOL_ARG_EMPTY_MARK}`]);
  });

  it("超过上限的字符串截断，截断标注另起一行", () => {
    const long = "字".repeat(TOOL_ARG_VALUE_MAX_CHARS + 20);
    const lines = parseToolArgs(JSON.stringify({ content: long })) || [];
    expect(lines).toHaveLength(2);
    expect(lines[0]?.value).toHaveLength(TOOL_ARG_VALUE_MAX_CHARS);
    expect(lines[1]).toMatchObject({ hint: true, depth: 1 });
    expect(lines[1]?.value).toContain(String(long.length));
  });

  it("单值 JSON 里的布尔与 null 原样输出", () => {
    expect(flat('{"ok":true,"none":null}')).toEqual(["ok:true", "none:null"]);
  });

  it("非 JSON 输入返回 null", () => {
    expect(parseToolArgs("cd /e/github && ls -la")).toBeNull();
    expect(parseToolArgs("")).toBeNull();
    expect(parseToolArgs("   ")).toBeNull();
  });

  it("截断的 JSON 返回 null，交给调用方退回纯文本", () => {
    expect(parseToolArgs('{"a":1,"b":')).toBeNull();
    expect(parseToolArgs('{"a":1')).toBeNull();
  });
});

describe("breakCommandLines", () => {
  it("按 && 与 | 折行，一条命令一步一行", () => {
    const command = "cd /repo && git show HEAD:CHANGELOG.md 2>/dev/null | head -120";
    expect(breakCommandLines(command)).toBe("cd /repo &&\ngit show HEAD:CHANGELOG.md 2>/dev/null |\nhead -120");
  });

  it("多段命令逐级切开，子 shell 照切不误", () => {
    const command = "cd /repo && MB=$(git merge-base HEAD upstream/main) && echo $MB | head -1";
    expect(breakCommandLines(command)).toBe(
      "cd /repo &&\nMB=$(git merge-base HEAD upstream/main) &&\necho $MB |\nhead -1",
    );
  });

  it("操作符前的空格原样保留，不额外加空格", () => {
    expect(breakCommandLines("a || b; c")).toBe("a ||\nb;\nc");
  });

  it("没有操作符的命令原样返回", () => {
    expect(breakCommandLines("git status --short")).toBe("git status --short");
    expect(breakCommandLines("")).toBe("");
  });

  it("只对命令类键名折行", () => {
    expect(isCommandArgLabel("command")).toBe(true);
    expect(isCommandArgLabel("CMD")).toBe(true);
    expect(isCommandArgLabel("query")).toBe(false);
    expect(isCommandArgLabel(undefined)).toBe(false);
  });
});
