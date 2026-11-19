<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <TeamMemberTileGridSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 team-member-tiles 块。
import TeamMemberTileGridSource from "../../chat/components/TeamMemberTileGridDemo.vue";
</script>
