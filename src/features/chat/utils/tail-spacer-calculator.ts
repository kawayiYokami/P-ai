/**
 * 尾部弹性留白高度计算器：
 *
 * 核心守卫规则：
 * 1. 新回合（新用户消息 / 切换会话）允许正常展开留白（甚至变大），让新消息能顶格对齐。
 * 2. 同一回合流式期间（isStreaming = true 且非新回合）：正文增长已由 absorbContentGrowthIntoTailSpacer
 *    在 ResizeObserver 中逐帧精确扣除，此时留白高度只能保持现值（禁止 watcher 重复总量扣减导致双重扣减、
 *    禁止 scrollHeight 瞬间缩水导致浏览器强制下拽 scrollTop，同时也杜绝 Virtua 滞后缓存反向弹大）。
 * 3. 流式结束后，允许根据真实正文高度做最终的结算校准。
 * 4. 归零必须精确到位；微小亚像素波动（< 2px）吸附避免无谓的样式回写与 Layout。
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

  // 同一回合流式期间：留白由 absorb 增量式精准扣减，计算器在此期间坚决不二次重写，避免双重扣减与活塞振荡
  if (!isNewTurn && isStreaming) {
    return currentSpacer;
  }

  const diff = Math.abs(currentSpacer - next);
  if (next === 0 ? currentSpacer !== 0 : diff >= 2) {
    return next;
  }
  return currentSpacer;
}
