<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">队员磁贴（首页磁贴墙占两列）</h3>
        <p class="text-sm text-base-content/70">
          一格一个队员：头像、队员名、他手上那件事。有在办议题时写「负责 #3 标题」，最右边一圈转着表示正在跑；没有议题就不硬凑句子，只显示他自己的运行时状态（忙碌 / 空闲 / 出错）。队员名取自会话绑定的 agent，会话标题交给自动标题生成。
        </p>
      </div>
      <TeamMemberTileGridSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 team-member-tiles 块。
import TeamMemberTileGridSource from "../../chat/components/TeamMemberTileGridDemo.vue";
</script>
