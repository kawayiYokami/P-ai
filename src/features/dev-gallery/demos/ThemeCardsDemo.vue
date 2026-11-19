<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
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
