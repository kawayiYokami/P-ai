<template>
  <!--
    首页磁贴墙里的一块：每行一个队员，像联系人列表那样一行一个。
    跨列写在这里没有意义——块本身是磁贴墙的格子，跨格由墙那侧决定。
  -->
  <div class="team-member-tile-block">
    <div class="team-member-tile-grid">
      <div
        v-for="member in members"
        :key="memberId(member)"
        class="flex h-full min-w-0 cursor-pointer items-center gap-2.5 overflow-hidden rounded-field px-2 py-1.5 transition-colors hover:bg-base-200/60 active:scale-[0.99] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary/50"
        role="button"
        tabindex="0"
        @click="selectMember(member)"
        @keydown.enter.prevent="selectMember(member)"
        @keydown.space.prevent="selectMember(member)"
      >
        <!--
          头像三级兜底：有缓存头像用图，无人格头像时用名字首字，两者都没有才退回图标。
          首字规则与招募区 agentInitials 一致（英文取前两位大写、中文取首字）。
          这里不做「按人格名反查头像」——那需要 chatPersonaNameMap，属于传数据一侧的职责。
        -->
        <div class="avatar shrink-0">
          <div class="h-5 w-5 rounded-full">
            <img
              v-if="avatarUrl(member)"
              :src="avatarUrl(member)"
              :alt="memberLabel(member)"
              class="h-5 w-5 rounded-full object-cover"
            />
            <div
              v-else-if="memberInitials(member)"
              class="flex h-5 w-5 items-center justify-center rounded-full bg-primary/80 text-[10px] font-semibold text-primary-content"
            >
              {{ memberInitials(member) }}
            </div>
            <div
              v-else
              class="flex h-5 w-5 items-center justify-center rounded-full bg-base-200 text-base-content/50"
            >
              <UserRound class="h-3 w-3" aria-hidden="true" />
            </div>
          </div>
        </div>

        <!--
          队员名：从会话绑定的 agent 取，不读会话标题。
          标题现在归自动标题生成（和主会话一样是内容标题），拿它当名字会让队员名随话题变。
        -->
        <div class="max-w-[40%] shrink-0 truncate text-sm text-base-content/85">
          {{ memberLabel(member) }}
        </div>
        <!--
          有在办议题时写一整句「正在处理 #3 标题」：说的是他手上这件事。
          一人最多一条在办，所以永远只有一条，不需要轮播或「+N」。
          吃满剩余宽度并单行截断，窄屏下先截这句，不撑破磁贴。
        -->
        <div
          v-if="activeIssue(member)"
          class="min-w-0 flex-1 truncate text-xs text-base-content/50"
          :title="workingOnLabel(member)"
        >
          {{ workingOnLabel(member) }}
        </div>
        <!-- 没有在办议题时不硬凑一句话，让空位把右侧状态顶到边上 -->
        <div v-else class="min-w-0 flex-1"></div>
        <!--
          运行时三态只在没有在办议题时出现：有议题时这格让给那句「正在处理」，
          议题状态也不再单独占一只徽章。忙碌 / 空闲 / 出错说的是他此刻在不在跑，
          与议题是两件事，不合并成一个字段。
        -->
        <span
          v-if="!activeIssue(member)"
          class="badge badge-xs shrink-0"
          :class="memberStateChip(member)"
        >
          {{ memberStateLabel(member) }}
        </span>
        <!-- 最右边永远是转圈：调度进行中就把这圈转起来，不被别的元素挤占 -->
        <span
          v-if="memberBusy(member)"
          class="loading loading-spinner loading-xs shrink-0 text-base-content/60"
          :aria-label="t('chat.teamMember.busy')"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { UserRound } from "@lucide/vue";
import { conversationRuntimeBusy } from "../utils/conversation-item-display";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import type { TeamMemberActiveIssue, TeamMemberTileItem } from "./team-member-tile";

const props = withDefaults(defineProps<{
  members?: TeamMemberTileItem[];
  avatarUrlMap?: Record<string, string>;
}>(), {
  members: () => [],
  avatarUrlMap: () => ({}),
});

const { t, locale } = useI18n();

/**
 * 点一格就打开这名队员的会话。
 * 交互留在组件里、只抛事件，打开哪个面板、激活哪个会话由传数据一侧决定。
 */
const emit = defineEmits<{
  (e: "selectMember", conversationId: string): void;
}>();

function selectMember(member: TeamMemberTileItem) {
  const id = memberId(member);
  if (!id) return;
  emit("selectMember", id);
}

function memberId(member: TeamMemberTileItem): string {
  return String(member.conversationId || "").trim();
}

/**
 * 磁贴上的队员名：从会话绑定的 agent 取。
 * 会话标题现在是自动生成的内容标题，不再承载队员名；agent 名拿不到时退名册角色名，
 * 两者都没有才走会话标题链兜底（此时磁贴本来也没有可显示的名字）。
 */
function memberLabel(member: TeamMemberTileItem): string {
  const personaName = String(member.agentName || "").trim();
  if (personaName) return personaName;
  const rosterName = String(member.teamName || "").trim();
  if (rosterName) return rosterName;
  return resolveConversationDisplayTitle(
    {
      conversationId: member.conversationId,
      title: member.title,
      summaryTitle: member.summaryTitle,
      updatedAt: member.updatedAt,
      lastMessageAt: member.lastMessageAt,
      teamName: member.teamName,
      agentName: member.agentName,
    },
    { locale: locale.value, untitledLabel: t("chat.untitledConversation") },
  );
}

function avatarUrl(member: TeamMemberTileItem): string {
  const agentId = String(member.agentId || "").trim();
  if (!agentId) return "";
  return String(props.avatarUrlMap?.[agentId] || "").trim();
}

/**
 * 头像首字兜底取人名，不取会话标题：
 * 头像代表的是这个人，标题代表的是这件事，用「重」当「李工」的头像会读不出来。
 */
function memberPersonName(member: TeamMemberTileItem): string {
  return String(member.agentName || member.teamName || "").trim();
}

function memberInitials(member: TeamMemberTileItem): string {
  const name = memberPersonName(member);
  if (!name) return "";
  const firstTwo = name.slice(0, 2);
  if (/^[A-Za-z]{2}/.test(firstTwo)) {
    return firstTwo.toUpperCase();
  }
  return name.charAt(0).toUpperCase();
}

/** 这条会话是否正在跑：与列表的忙碌判断同一份口径 */
function memberBusy(member: TeamMemberTileItem): boolean {
  return conversationRuntimeBusy(member.runtimeState as Parameters<typeof conversationRuntimeBusy>[0]);
}

/** 运行时三态取色：忙 = 正在跑，空闲 = 中性，出错 = 错误色 */
const MEMBER_STATE_CHIP: Record<string, string> = {
  busy: "badge-primary",
  idle: "bg-base-content/15 text-base-content",
  error: "badge-error",
};

/** 他手上在办的那条议题；没有就是 null，磁贴退回运行时状态 */
function activeIssue(member: TeamMemberTileItem): TeamMemberActiveIssue | null {
  const issue = member.activeIssue;
  if (!issue || !String(issue.status || "").trim()) return null;
  return issue;
}

/** 议题编号那一截：`#3` */
function activeIssueNumber(member: TeamMemberTileItem): string {
  const issue = activeIssue(member);
  return issue ? `#${issue.number}` : "";
}

/**
 * 有在办议题时那一整句：「正在处理 #3 磁贴徽章口径重做」。
 * 状态词不再单独占一只徽章——一句话说清他在做什么，比拆成「标题 + 状态」两截好读。
 */
function workingOnLabel(member: TeamMemberTileItem): string {
  const issue = activeIssue(member);
  if (!issue) return "";
  const title = String(issue.title || "").trim();
  const target = title ? `${activeIssueNumber(member)} ${title}` : activeIssueNumber(member);
  return t("chat.teamMember.workingOn", { issue: target });
}

/**
 * 运行时三态。后端只给 busy / idle / error 三个值，
 * 拿不到（非队员会话或字段尚未下发）时按空闲处理，不停在一只空徽章上。
 */
function memberState(member: TeamMemberTileItem): "busy" | "idle" | "error" {
  const raw = String(member.memberState || "").trim();
  return raw === "busy" || raw === "error" ? raw : "idle";
}

function memberStateChip(member: TeamMemberTileItem): string {
  return MEMBER_STATE_CHIP[memberState(member)];
}

function memberStateLabel(member: TeamMemberTileItem): string {
  return t(`chat.teamMember.state.${memberState(member)}`);
}
</script>

<style scoped>
/* 块自身不做跨格：占几格由磁贴墙那侧决定，这里只管内部排布 */
.team-member-tile-block {
  min-width: 0;
}

/*
  每行一个队员：联系人列表那种简洁行，不再给每行套卡片壳
  （卡片里套卡片是列表分组最差的做法，分组靠行距与 hover 底色就够）。
  行高与间距取磁贴墙给的变量，行高固定后内容多的行也不会把整行撑高，
  滚动区算出来的高度才作数。
*/
.team-member-tile-grid {
  display: grid;
  gap: var(--ecall-team-member-gap, 0.5rem);
  grid-template-columns: minmax(0, 1fr);
  grid-auto-rows: var(--ecall-team-member-row, 32px);
}
</style>
