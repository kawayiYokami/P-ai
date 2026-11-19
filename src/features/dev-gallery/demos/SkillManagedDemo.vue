<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">技能面板（托管技能禁用态）</h3>
        <p class="text-sm text-base-content/70">
          渲染真实 SkillTab，喂两个托管技能（persona-craft / pai-guide）与一个普通内置技能、一个自定义技能。托管技能标「系统托管」，开关是禁用态，详情页写着一句为什么关不掉；开关与保存在这张 demo 里不写库。
        </p>
      </div>
      <SkillTabSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 skill-managed 块。
import SkillTabSource from "../../config/components/SkillTabDemo.vue";
</script>
