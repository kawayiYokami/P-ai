import { describe, expect, it } from "vitest";
import { buildContactLogDisplayItem } from "./contact-log-display";

const t = (key: string, params?: Record<string, unknown>) => params
  ? `${key}:${JSON.stringify(params)}`
  : key;

describe("buildContactLogDisplayItem", () => {
  it("解析收到的联系人消息与附件计数", () => {
    const item = buildContactLogDisplayItem({
      timestamp: "2026-07-13T00:00:00Z",
      level: "info",
      message: "[联系人消息] 收到: sender=小明, preview=你好, image_count=2, audio_count=0, attachment_count=1",
    }, t);

    expect(item?.summary).toBe("[小明]你好");
    expect(item?.detail).toContain("imageCount");
    expect(item?.detail).toContain("attachmentCount");
  });

  it("忽略成功入队这类内部噪声", () => {
    const item = buildContactLogDisplayItem({ timestamp: "now", level: "info", message: "[联系人消息] 入队: reason=ok" }, t);
    expect(item).toBeNull();
  });

  it("展示按消息头过滤的消息，但不暴露原文", () => {
    const item = buildContactLogDisplayItem({
      timestamp: "now",
      level: "info",
      message: "[联系人消息] 过滤跳过: contact=小明, prefix=#, text_len=20",
    }, t);

    expect(item?.kind).toBe("config.remoteIm.logKindFilter");
    expect(item?.summary).toContain("#");
  });

  it("保留未知异常日志的通用展示", () => {
    const item = buildContactLogDisplayItem({ timestamp: "now", level: "error", message: "未知错误" }, t);
    expect(item?.kind).toBe("config.remoteIm.logKindSystem");
    expect(item?.title).toBe("config.remoteIm.logAbnormalTitle");
  });

  it("在场切换解析出状态转换、原因与预计离场", () => {
    const item = buildContactLogDisplayItem({
      timestamp: "now",
      level: "info",
      message: "[在场切换] contact=小明, presence=Away -> Present, 预计离场=10:05:00, 原因=命中群昵称",
    }, t);

    expect(item?.title).toBe(
      "config.remoteIm.logPresenceAway → config.remoteIm.logPresencePresent",
    );
    expect(item?.summary).toBe("命中群昵称");
    expect(item?.detail).toContain("10:05:00");
  });

  it("巡检日志用中英文冒号都能切出动作名", () => {
    const start = buildContactLogDisplayItem({
      timestamp: "now",
      level: "info",
      message: "[巡检] 开始: contact=小明, 路径=Mention",
    }, t);
    const finish = buildContactLogDisplayItem({
      timestamp: "now",
      level: "info",
      message: "[巡检] 结束：联系人=小明，处理=已回复",
    }, t);

    expect(start?.title).toBe("开始");
    expect(start?.summary).toBe("Mention");
    expect(finish?.title).toBe("结束");
    expect(finish?.summary).toBe("已回复");
  });
});
