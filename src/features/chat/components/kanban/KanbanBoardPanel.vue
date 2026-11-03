<template>
  <KanbanBoard
    class="h-full w-full"
    :issues="issues"
    :loading="loading"
    :error-text="errorText"
    :approving-number="approvingNumber"
    :leader-name="leaderName"
    :leader-avatar-url="leaderAvatarUrl"
    :avatar-url-map="avatarUrlMap"
    :persona-avatar-url-map="personaAvatarUrlMap"
    :persona-name-map="personaNameMap"
    :team-members="teamMembers"
    :show-back="true"
    @back="emit('back')"
    @refresh="emit('refresh')"
    @approve="emit('approve', $event)"
    @open-plan="emit('openPlan', $event)"
    @reference="emit('reference', $event)"
  />
</template>

<script setup lang="ts">
import KanbanBoard from "./KanbanBoard.vue";
import type { KanbanIssue } from "./kanban-types";

withDefaults(defineProps<{
  issues: KanbanIssue[];
  loading?: boolean;
  errorText?: string;
  approvingNumber?: number | null;
  leaderName?: string;
  leaderAvatarUrl?: string;
  avatarUrlMap?: Record<string, string>;
  personaAvatarUrlMap?: Record<string, string>;
  personaNameMap?: Record<string, string>;
  teamMembers?: Array<{ teamName?: string; agentId?: string; agentName?: string }>;
}>(), {
  loading: false,
  errorText: "",
  approvingNumber: null,
  leaderName: "",
  leaderAvatarUrl: "",
  avatarUrlMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  personaNameMap: () => ({}),
  teamMembers: () => [],
});

const emit = defineEmits<{
  (e: "back"): void;
  (e: "refresh"): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "reference", issue: KanbanIssue): void;
}>();
</script>
