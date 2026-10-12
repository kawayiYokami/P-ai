import { describe, expect, it } from "vitest";
import { normalizeResponseStrategy, platformLabelOf } from "./helpers";

describe("normalizeResponseStrategy", () => {
  it("将缺失或未知策略收敛为智能判断", () => {
    expect(normalizeResponseStrategy()).toBe("smart_judge");
    expect(normalizeResponseStrategy("unknown")).toBe("smart_judge");
  });

  it("保留明确选择的始终回复", () => {
    expect(normalizeResponseStrategy("always_reply")).toBe("always_reply");
  });
});

describe("platformLabelOf", () => {
  it("识别 QQ 官方机器人平台", () => {
    expect(platformLabelOf("qq_official")).toBe("QQ 官方机器人");
    expect(platformLabelOf(" QQ_OFFICIAL ")).toBe("QQ 官方机器人");
  });

  it("未知平台回退为 OneBot v11", () => {
    expect(platformLabelOf("")).toBe("OneBot v11");
  });
});
