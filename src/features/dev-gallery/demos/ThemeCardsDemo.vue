<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">主题预设卡片（AutoThemeGrid）</h3>
        <p class="text-sm text-base-content/70">每张卡片在品牌色（主色/次色/强调/中性）之下列出四大状态色（成功/警告/错误/信息），用来判断该主题的状态色是否够醒目。卡片用真实 DaisyUI 主题渲染。</p>
      </div>
      <AutoThemeGrid
        :light-themes="lightThemes"
        :dark-themes="darkThemes"
        auto-light-theme="light"
        auto-dark-theme="dark"
        :light-custom-tokens="lightTokens"
        :dark-custom-tokens="darkTokens"
        @select-light="() => {}"
        @select-dark="() => {}"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import AutoThemeGrid from "../../config/components/AutoThemeGrid.vue";
import {
  generateGeneratedThemeTokens,
  getGeneratedThemeDefaultControls,
} from "../../shell/theme/theme-generator";

const lightThemes = ["light", "cupcake", "corporate", "lofi", "fantasy", "autumn"];
const darkThemes = ["dark", "crimson", "forest", "dracula", "business", "dim"];
const lightTokens = generateGeneratedThemeTokens(getGeneratedThemeDefaultControls("light"));
const darkTokens = generateGeneratedThemeTokens(getGeneratedThemeDefaultControls("dark"));
</script>
