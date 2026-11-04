/**
 * 尾部弹性留白高度计算器：
 *
 * 核心守卫规则：
 * 1. 新回合（新用户消息 / 切换会话）允许正常展开留白（甚至变大），让新消息能顶格对齐。
 * 2. 同一回合流式期间（isStreaming = true 且非新回合），正文单调变长，留白只能单调缩小或归零。
 *    绝不允许因为 Virtua 尺寸缓存滞后，把已被 absorb 扣减掉的留白反复反弹写大（消灭活塞式振荡）。
 * 3. 归零必须精确到位；微小亚像素波动（< 2px）吸附避免无谓的样式回写与 Layout。
 */
export function resolveNextTailSpacerMinHeight(params: {
  targetHeight: number;
  tailContentHeight: number;
  currentSpacer: number;
  isNewTurn: boolean;
  isStreaming: boolean;
}): number {
  const { targetHeight, tailContentHeight, currentSpacer, isNewTurn, isStreaming } = params;
  const next = Math.max(0, targetHeight - tailContentHeight);

  // 同一回合流式期间：留白已经被 absorb 实时扣减，只能单调收缩或归零，禁止滞后缓存反向弹大
  if (!isNewTurn && isStreaming && next > currentSpacer) {
    return currentSpacer;
  }

  const diff = Math.abs(currentSpacer - next);
  if (next === 0 ? currentSpacer !== 0 : diff >= 2) {
    return next;
  }
  return currentSpacer;
}
