<template>
  <ul v-if="stages.length" class="timeline timeline-vertical timeline-compact w-full">
    <li v-for="(item, index) in stages" :key="`${index}:${item.stage}:${item.elapsedMs}`" class="min-w-0">
      <hr v-if="index !== 0" />
      <div class="timeline-middle">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-5 w-5 shrink-0" :class="iconColorClass(item.detail)">
          <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 10-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z" clip-rule="evenodd" />
        </svg>
      </div>
      <div class="timeline-end timeline-box min-w-0 w-full">
        <button type="button" class="flex w-full min-w-0 items-center gap-1.5 text-left" @click="toggleCollapsed(itemKey(item, index))">
          <span class="shrink-0 text-xs font-medium text-base-content/80">{{ phaseLabel(item.stage) }}</span>
          <span v-if="titleInlineSummary(item)" class="min-w-0 flex-1 truncate text-xs text-base-content/55">{{ titleInlineSummary(item) }}</span>
          <span v-else class="flex-1"></span>
          <span class="shrink-0 text-caption tabular-nums text-base-content/40">+{{ formatElapsedShort(item.elapsedMs) }}</span>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5 shrink-0 text-base-content/25 transition-transform duration-200" :class="isCollapsed(itemKey(item, index)) ? '' : 'rotate-90'"><path fill-rule="evenodd" d="M7.21 14.78a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd" /></svg>
        </button>
        <Transition :css="false" @enter="animateEnter" @leave="animateLeave" @enter-cancelled="cleanupAnimation" @leave-cancelled="cleanupAnimation">
          <div v-if="!isCollapsed(itemKey(item, index))" class="mt-2 min-w-0 w-full space-y-1.5 border-t border-base-300 pt-2">
            <div v-if="eventSummaryLine(item)" class="whitespace-normal break-words text-xs leading-5 text-base-content/55">{{ eventSummaryLine(item) }}</div>
            <div v-if="eventBodyText(item)" class="whitespace-pre-wrap break-words break-all text-xs leading-5 text-base-content/70">{{ eventBodyText(item) }}</div>
            <div v-if="eventErrorText(item)" class="whitespace-pre-wrap break-words break-all text-caption leading-4 text-error/80">{{ eventErrorText(item) }}</div>
            <div v-if="!eventSummaryLine(item) && !eventBodyText(item) && !eventErrorText(item)" class="text-xs text-base-content/30">—</div>
          </div>
        </Transition>
      </div>
      <hr v-if="index !== stages.length - 1" />
    </li>
  </ul>
  <div v-else class="text-xs text-base-content/30">—</div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { LlmRoundLogStage } from "../../../types/app";

const props = defineProps<{ stages: LlmRoundLogStage[] }>();
const { t } = useI18n();

const collapsedIds = ref<Set<string>>(new Set());
function itemKey(item: LlmRoundLogStage, index: number) {
  return `${index}:${item.stage}:${item.elapsedMs}`;
}
function isCollapsed(key: string) { return collapsedIds.value.has(key); }
function toggleCollapsed(key: string) {
  const next = new Set(collapsedIds.value);
  if (next.has(key)) next.delete(key); else next.add(key);
  collapsedIds.value = next;
}
function syncCollapsed() {
  const all = props.stages.map((item, index) => itemKey(item, index));
  if (all.length === 0) { collapsedIds.value = new Set(); return; }
  collapsedIds.value = new Set(all);
}
syncCollapsed();
watch(() => props.stages.length, syncCollapsed);

const stageLabelKeys: Record<string, string> = {
  "send_chat_message_inner.start": "config.logs.stages.chatStart",
  "runtime_and_session_ready": "config.logs.stages.runtimeReady",
  "run.begin": "config.logs.stages.runBegin",
  "attachments_processed": "config.logs.stages.attachmentsProcessed",
  "prepare_context.begin": "config.logs.stages.prepareBegin",
  "prepare_context.conversation_lock_wait_done": "config.logs.stages.conversationLockDone",
  "prepare_context.skill_snapshot_ready": "config.logs.stages.skillReady",
  "prepare_context.workspace_agents_ready": "config.logs.stages.workspaceReady",
  "prepare_context.todo_guide_ready": "config.logs.stages.todoGuideReady",
  "prepare_context.im_runtime_ready": "config.logs.stages.imReady",
  "prepare_context.task_board_ready": "config.logs.stages.taskBoardReady",
  "prepare_context.todo_board_ready": "config.logs.stages.todoBoardReady",
  "prepare_context.attachment_hints_ready": "config.logs.stages.attachmentHintsReady",
  "prepare_context.overrides_built": "config.logs.stages.overridesBuilt",
  "prepare_context.terminal_block_ready": "config.logs.stages.terminalBlockReady",
  "prepare_context.prompt_build_begin": "config.logs.stages.promptBuildBegin",
  "prepare_context.prompt_fixed_system_ready": "config.logs.stages.promptFixedSystemReady",
  "prepare_context.prompt_conversation_payload_ready": "config.logs.stages.promptConversationReady",
  "prepare_context.prompt_system_cache_hit": "config.logs.stages.promptCacheHit",
  "prepare_context.prompt_system_cache_rebuilt": "config.logs.stages.promptCacheRebuilt",
  "prepare_context.prompt_system_finalize_ready": "config.logs.stages.promptFinalizeReady",
  "prepare_context.prompt_built": "config.logs.stages.promptBuilt",
  "prepare_context.prompt_tokens_estimated": "config.logs.stages.promptTokensEstimated",
  "prepare_context.done": "config.logs.stages.prepareDone",
  "pre_send_archive_checked": "config.logs.stages.archiveChecked",
  "prompt_ready": "config.logs.stages.promptReady",
  "model_reply_ready": "config.logs.stages.modelReplyReady",
  "assistant_final_append.start": "config.logs.stages.finalAppendStart",
  "assistant_final_append.finish": "config.logs.stages.finalAppendFinish",
  "assistant_message_persist_scheduled": "config.logs.stages.persistScheduled",
  "send_chat_message_inner.finish": "config.logs.stages.chatFinish",
  "model_round_total": "config.logs.stages.modelRoundTotal",
  "dispatch_start": "config.logs.stages.dispatchStart",
  "dispatch_end": "config.logs.stages.dispatchEnd",
  "model_round_start": "config.logs.stages.modelRoundStart",
  "model_round_end": "config.logs.stages.modelRoundEnd",
  "tool_call": "config.logs.stages.toolCall",
  "tool_result": "config.logs.stages.toolResult",
  "compaction_start": "config.logs.stages.compactionStart",
  "compaction_end": "config.logs.stages.compactionEnd",
};
function phaseLabel(phase: string) {
  const key = stageLabelKeys[phase];
  if (key) return t(key);
  if (phase.startsWith("model_request.start")) return t("config.logs.stages.modelRequestStart");
  if (phase.startsWith("model_request.finish")) return t("config.logs.stages.modelRequestFinish");
  return phase;
}
function normalizeDetail(detail: unknown) { return (detail ?? {}) as Record<string, unknown>; }
function iconColorClass(detail: unknown) {
  const d = normalizeDetail(detail);
  if (d.error) return "text-error";
  if (d.isError === true) return "text-error";
  if (d.hasError === true) return "text-error";
  if ((d as Record<string, unknown>).success === false) return "text-error";
  if ((d as Record<string, unknown>).success === true) return "text-success";
  return "text-primary";
}
function titleInlineSummary(item: LlmRoundLogStage) {
  const d = normalizeDetail((item as unknown as { detail?: unknown }).detail);
  if (item.stage === "model_round_start") { if (d.modelName) return String(d.modelName); if (d.attempt != null) return t("config.logs.attemptLabel", { count: Number(d.attempt) }); return ""; }
  if (item.stage === "model_round_end") { if (d.modelName) return String(d.modelName); return ""; }
  if (item.stage === "tool_call" || item.stage === "tool_result") { if (d.toolName) return String(d.toolName); return ""; }
  if (item.stage === "dispatch_start") return d.traceId ? String(d.traceId).slice(0, 8) : "";
  if (item.stage === "dispatch_end") { if (d.assistantTextLength != null) return t("config.logs.timelineTextChars", { count: Number(d.assistantTextLength) }); return ""; }
  if (item.stage === "compaction_start" || item.stage === "compaction_end") { if (d.reason) return String(d.reason); return ""; }
  return "";
}
function formatTimeDelta(ms: number): string { const rounded = Math.max(0, Math.round(ms)); if (rounded >= 100) return t("config.logs.seconds", { value: (rounded / 1000).toFixed(1) }); return t("config.logs.ms", { ms: rounded }); }
function eventSummaryLine(item: LlmRoundLogStage) {
  const d = normalizeDetail((item as unknown as { detail?: unknown }).detail);
  if (item.stage === "model_round_start") { const parts: string[] = []; if (d.modelName) parts.push(String(d.modelName)); if (d.attempt != null) parts.push(t("config.logs.attemptLabel", { count: Number(d.attempt) })); if (d.providerName) parts.push(String(d.providerName)); if (item.sincePrevMs != null) parts.push(`+${formatTimeDelta(Number(item.sincePrevMs))}`); return parts.join(" · "); }
  if (item.stage === "model_round_end") { const parts: string[] = []; if (d.modelName) parts.push(String(d.modelName)); if ((d as Record<string, unknown>).elapsedMs != null) parts.push(t("config.logs.ms", { ms: Math.max(0, Math.round(Number((d as Record<string, unknown>).elapsedMs))) })); else if (item.elapsedMs != null) parts.push(t("config.logs.ms", { ms: Math.max(0, Math.round(Number(item.elapsedMs))) })); if (d.assistantTextLength != null) parts.push(t("config.logs.timelineTextChars", { count: Number(d.assistantTextLength) })); if (d.reasoningLength != null && Number(d.reasoningLength) > 0) parts.push(t("config.logs.timelineReasoningChars", { count: Number(d.reasoningLength) })); if (d.toolCallCount != null) parts.push(Number(d.toolCallCount) > 0 ? t("config.logs.timelineToolCount", { count: Number(d.toolCallCount) }) : t("config.logs.timelineNoTools")); if (d.hasError) parts.push(t("config.logs.timelineAbnormal")); if (item.sincePrevMs != null) parts.push(`+${formatTimeDelta(Number(item.sincePrevMs))}`); return parts.join(" · "); }
  if (item.stage === "tool_call") { const parts: string[] = []; if (d.toolName) parts.push(String(d.toolName)); if (d.argLength != null) parts.push(t("config.logs.timelineCharCount", { count: Number(d.argLength) })); if (item.sincePrevMs != null) parts.push(`+${formatTimeDelta(Number(item.sincePrevMs))}`); return parts.join(" · "); }
  if (item.stage === "tool_result") { const parts: string[] = []; if (d.toolName) parts.push(String(d.toolName)); if (d.isError === true) parts.push(t("config.logs.timelineFailed")); else if (d.isError === false) parts.push(t("config.logs.timelineSuccess")); if (d.textLength != null) parts.push(t("config.logs.timelineCharCount", { count: Number(d.textLength) })); if (item.sincePrevMs != null) parts.push(`+${formatTimeDelta(Number(item.sincePrevMs))}`); return parts.join(" · "); }
  if (item.stage === "compaction_start" || item.stage === "compaction_end") { const parts: string[] = []; if (d.reason) parts.push(String(d.reason)); if (item.sincePrevMs != null) parts.push(`+${formatTimeDelta(Number(item.sincePrevMs))}`); return parts.join(" · "); }
  if (item.stage === "dispatch_start") { const v = d.traceId ? String(d.traceId) : ""; return v ? `${v} · +${formatTimeDelta(Number(item.sincePrevMs))}` : `+${formatTimeDelta(Number(item.sincePrevMs))}`; }
  if (item.stage === "dispatch_end") { if (d.assistantTextLength != null) return `${t("config.logs.timelineTextChars", { count: Number(d.assistantTextLength) })} · +${formatTimeDelta(Number(item.sincePrevMs))}`; return `+${formatTimeDelta(Number(item.sincePrevMs))}`; }
  return `+${formatTimeDelta(Number(item.sincePrevMs))}`;
}
function eventBodyText(item: LlmRoundLogStage) {
  const d = normalizeDetail((item as unknown as { detail?: unknown }).detail);
  if (item.stage === "model_round_end") { const chunks: string[] = []; if (d.reasoningPreview) chunks.push(String(d.reasoningPreview).trim()); if (d.textPreview) chunks.push(String(d.textPreview).trim()); return chunks.join("\n\n"); }
  if (item.stage === "tool_call") { if (d.argPreview) return String(d.argPreview); return ""; }
  if (item.stage === "tool_result") { if (d.textPreview) return String(d.textPreview); return ""; }
  if (item.stage === "dispatch_end") { if (d.textPreview) return String(d.textPreview); return ""; }
  return "";
}
function eventErrorText(item: LlmRoundLogStage) { const d = normalizeDetail((item as unknown as { detail?: unknown }).detail); if (!d.error) return ""; return String(d.error).trim(); }
function formatElapsedShort(value: number) { if (!Number.isFinite(value) || value < 0) return "0s"; if (value < 1000) return `${value}ms`; return `${(value / 1000).toFixed(1)}s`; }
function cleanupAnimation(element: Element) { const el = element as HTMLElement; el.style.height=""; el.style.opacity=""; el.style.transform=""; el.style.overflow=""; el.style.willChange=""; el.style.transition=""; }
function animateEnter(element: Element, done: () => void) {
  const el = element as HTMLElement; cleanupAnimation(el); delete (el.dataset as Record<string,string>).ecallCollapseFinished; el.style.height="0px"; el.style.opacity="0"; el.style.transform="translateY(-6px)"; el.style.overflow="hidden"; el.style.willChange="height, opacity, transform"; void el.offsetHeight;
  const onEnd = (e: TransitionEvent) => { if (e.target!==el || e.propertyName!=="height") return; finishAnimation(el,onEnd,done); };
  el.addEventListener("transitionend", onEnd); el.style.transition=["height 180ms cubic-bezier(0.22, 1, 0.36, 1)","opacity 140ms ease-out","transform 180ms cubic-bezier(0.22, 1, 0.36, 1)"].join(", ");
  requestAnimationFrame(()=>{ el.style.height=`${el.scrollHeight}px`; el.style.opacity="1"; el.style.transform="translateY(0)"; });
  window.setTimeout(()=>finishAnimation(el,onEnd,done),400);
}
function animateLeave(element: Element, done: () => void) {
  const el = element as HTMLElement; cleanupAnimation(el); delete (el.dataset as Record<string,string>).ecallCollapseFinished; el.style.height=`${el.scrollHeight}px`; el.style.opacity="1"; el.style.transform="translateY(0)"; el.style.overflow="hidden"; el.style.willChange="height, opacity, transform"; void el.offsetHeight;
  const onEnd = (e: TransitionEvent) => { if (e.target!==el || e.propertyName!=="height") return; finishAnimation(el,onEnd,done); };
  el.addEventListener("transitionend", onEnd); el.style.transition=["height 180ms cubic-bezier(0.22, 1, 0.36, 1)","opacity 140ms ease-out","transform 180ms cubic-bezier(0.22, 1, 0.36, 1)"].join(", ");
  requestAnimationFrame(()=>{ el.style.height="0px"; el.style.opacity="0"; el.style.transform="translateY(-6px)"; });
  window.setTimeout(()=>finishAnimation(el,onEnd,done),400);
}
function finishAnimation(el: HTMLElement, onEnd: (e: TransitionEvent)=>void, done: ()=>void){ if((el.dataset as Record<string,string>).ecallCollapseFinished==="1") return; (el.dataset as Record<string,string>).ecallCollapseFinished="1"; el.removeEventListener("transitionend", onEnd); cleanupAnimation(el); done(); }
</script>
