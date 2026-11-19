<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">首页磁贴区（追问入口 + 队员状态表）</h3>
        <p class="text-sm text-base-content/70">
          首页只留一个「追问」入口磁贴，追问会话本身不铺卡；队员用一块状态表占磁贴墙的 2×2 格，内部每行一个队员、可见两行，多的在里面滚动。点队员那一行打开该队员会话（demo 里原地丢弃）。
        </p>
      </div>
      <ChatHomePanelSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 chat-home 块。
import ChatHomePanelSource from "../../chat/components/ChatHomePanelDemo.vue";
</script>
