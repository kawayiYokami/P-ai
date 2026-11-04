import { describe, expect, it } from "vitest";
import { resolveNextTailSpacerMinHeight } from "./tail-spacer-calculator";

describe("resolveNextTailSpacerMinHeight", () => {
  it("新回合开始：允许留白依据新内容高度重新撑大，保证新消息能够顶格对齐", () => {
    // 假设上一回合留白已收缩至 0，新发送一条消息（内容仅 40px，视口目标 600px）
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 600,
      tailContentHeight: 40,
      currentSpacer: 0,
      isNewTurn: true,
      isStreaming: true,
    });
    expect(result).toBe(560);
  });

  it("同一回合流式期间：正文增长导致计算高度变小，允许正常收缩", () => {
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 600,
      tailContentHeight: 100,
      currentSpacer: 560,
      isNewTurn: false,
      isStreaming: true,
    });
    expect(result).toBe(500);
  });

  it("同一回合流式期间：已被 absorb 扣减后，滞后的缓存计算值偏大时坚决不反弹", () => {
    // absorb 已经把留白实时扣到了 340px
    // 但是此时 Virtua 缓存还滞留在上一帧，计算出的 next 仍为 370px
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 600,
      tailContentHeight: 230, // 对应 600 - 230 = 370px
      currentSpacer: 340,
      isNewTurn: false,
      isStreaming: true,
    });
    // 坚决保持当前已被吸收的 340px，绝不反弹至 370px
    expect(result).toBe(340);
  });

  it("非流式态（如窗口缩放 / 会话切入完成）：允许正常双向校准", () => {
    // 视口被用户手动拉高（600 -> 700），非流式期间允许正常更新放大
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 700,
      tailContentHeight: 200,
      currentSpacer: 400,
      isNewTurn: false,
      isStreaming: false,
    });
    expect(result).toBe(500);
  });

  it("微小亚像素波动（< 2px）自动吸附，避免抖动和无谓重排", () => {
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 600,
      tailContentHeight: 201,
      currentSpacer: 400, // 600 - 201 = 399, diff = 1 < 2
      isNewTurn: false,
      isStreaming: false,
    });
    expect(result).toBe(400);
  });

  it("归零精确到位，即使差值仅为 1px 也必须精确归零", () => {
    const result = resolveNextTailSpacerMinHeight({
      targetHeight: 600,
      tailContentHeight: 600,
      currentSpacer: 1, // diff = 1, 但 next === 0
      isNewTurn: false,
      isStreaming: true,
    });
    expect(result).toBe(0);
  });
});
