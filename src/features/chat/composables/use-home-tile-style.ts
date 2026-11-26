import { ref } from "vue";

/**
 * 磁贴墙的呈现样式，两态只差底色与悬停色，形状（无缝、直角、格线）不变。
 *
 * 两个颜色角色固定是「背景色」和「对比色」：
 * - `tile`（磁贴）：卡面用对比色，与背景色分开，卡之间靠色差成块
 * - `grid`（网格）：卡面用背景色，与墙面同色，只靠格线分区，悬停才浮到对比色
 */
export type HomeTileStyle = "tile" | "grid";

const HOME_TILE_STYLE_STORAGE_KEY = "easy_call.home_tile_style.v1";

function normalizeHomeTileStyle(value: unknown): HomeTileStyle {
  return String(value || "").trim() === "grid" ? "grid" : "tile";
}

function readHomeTileStyle(): HomeTileStyle {
  if (typeof window === "undefined") return "tile";
  return normalizeHomeTileStyle(window.localStorage.getItem(HOME_TILE_STYLE_STORAGE_KEY));
}

/** 模块级单例：磁贴墙在主窗口与侧边栏各挂一份时，切换要 global 一致 */
const homeTileStyle = ref<HomeTileStyle>(readHomeTileStyle());

export function useHomeTileStyle() {
  function setHomeTileStyle(next: HomeTileStyle) {
    const normalized = normalizeHomeTileStyle(next);
    if (homeTileStyle.value === normalized) return;
    homeTileStyle.value = normalized;
    if (typeof window === "undefined") return;
    window.localStorage.setItem(HOME_TILE_STYLE_STORAGE_KEY, normalized);
  }

  return { homeTileStyle, setHomeTileStyle };
}
