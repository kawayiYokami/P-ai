<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { TerminalApprovalRequestPayload } from "../composables/use-terminal-approval";
import TerminalApprovalImpactPanel from "./TerminalApprovalImpactPanel.vue";
import TerminalApprovalPatchSample from "./TerminalApprovalPatchSample.vue";

const props = withDefaults(defineProps<{
  open: boolean;
  payload: TerminalApprovalRequestPayload | null;
  resolving: boolean;
  queueLength?: number;
}>(), {
  queueLength: 0,
});

const emit = defineEmits<{
  approve: [];
  deny: [];
  close: [];
}>();

const { t } = useI18n();

type TerminalApprovalImpactItem = {
  path: string;
  adds: number;
  removes: number;
  kind: "update" | "add" | "delete" | "other";
};

function splitTerminalApprovalPatches(raw: string): string[][] {
  const normalized = String(raw || "").replace(/\r/g, "");
  const lines = normalized.split("\n");
  if (!normalized.trim()) return [[]];

  const patches: string[][] = [];
  let currentPatch: string[] = [];
  let inPatchBlock = false;

  for (const rawLine of lines) {
    const line = String(rawLine || "");
    const trimmedLine = line.trim();
    if (trimmedLine.startsWith("*** Begin Patch")) {
      if (currentPatch.length > 0) {
        patches.push(currentPatch);
      }
      currentPatch = [line];
      inPatchBlock = true;
      continue;
    }

    if (inPatchBlock && trimmedLine.startsWith("*** End Patch")) {
      currentPatch.push(line);
      patches.push(currentPatch);
      currentPatch = [];
      inPatchBlock = false;
      continue;
    }

    currentPatch.push(line);
  }

  if (currentPatch.length > 0) {
    patches.push(currentPatch);
  }

  return patches.length > 0 ? patches : [lines];
}

function getTerminalApprovalPayloadText(payload: TerminalApprovalRequestPayload | null): string {
  return String(payload?.callPreview || payload?.command || payload?.summary || payload?.message || "").replace(/\r/g, "");
}

function getTerminalApprovalPatchPath(lines: string[]): string {
  for (const rawLine of lines) {
    const line = String(rawLine || "").trim();
    if (line.startsWith("*** Update File:")) {
      return line.replace("*** Update File:", "").trim();
    }
    if (line.startsWith("*** Add File:")) {
      return line.replace("*** Add File:", "").trim();
    }
    if (line.startsWith("*** Delete File:")) {
      return line.replace("*** Delete File:", "").trim();
    }
  }
  return "";
}

function getTerminalApprovalPatchKind(lines: string[]): "update" | "add" | "delete" | "other" {
  for (const rawLine of lines) {
    const line = String(rawLine || "").trim();
    if (line.startsWith("*** Update File:")) return "update";
    if (line.startsWith("*** Add File:")) return "add";
    if (line.startsWith("*** Delete File:")) return "delete";
  }
  return "other";
}

function countTerminalApprovalPatchDelta(lines: string[]) {
  let adds = 0;
  let removes = 0;
  for (const rawLine of lines) {
    const line = String(rawLine || "");
    if (line.startsWith("+") && !line.startsWith("+++")) {
      adds += 1;
      continue;
    }
    if (line.startsWith("-") && !line.startsWith("---")) {
      removes += 1;
    }
  }
  return { adds, removes };
}

const terminalApprovalDialogTitle = computed(() => {
  return String(props.payload?.title || "终端审批");
});

const terminalApprovalPatchBlocks = computed(() => {
  const raw = getTerminalApprovalPayloadText(props.payload);
  if (!raw.trim()) return [[]];
  return splitTerminalApprovalPatches(raw);
});

const terminalApprovalPreviewLines = computed(() => {
  const raw = getTerminalApprovalPayloadText(props.payload);
  if (!raw.trim()) return [];
  return raw.split("\n");
});

const terminalApprovalImpactSummary = computed<TerminalApprovalImpactItem[]>(() => {
  const patchItems = terminalApprovalPatchBlocks.value
    .map((lines) => {
      const path = getTerminalApprovalPatchPath(lines);
      if (!path) return null;
      return {
        path,
        kind: getTerminalApprovalPatchKind(lines),
        ...countTerminalApprovalPatchDelta(lines),
      };
    })
    .filter((item): item is TerminalApprovalImpactItem => !!item);
  if (patchItems.length > 0) return patchItems;

  const current = props.payload;
  if (!current) return [];
  const paths = Array.from(
    new Set([
      ...(Array.isArray(current.targetPaths) ? current.targetPaths : []),
      ...(Array.isArray(current.existingPaths) ? current.existingPaths : []),
      String(current.requestedPath || "").trim(),
    ].filter(Boolean)),
  );
  return paths.map((path) => ({
    path,
    adds: 0,
    removes: 0,
    kind: "other" as const,
  }));
});

const terminalApprovalCurrentPatchIndex = ref(0);

const terminalApprovalCurrentPatchLines = computed(() => {
  const blocks = terminalApprovalPatchBlocks.value;
  if (blocks.length === 0) return [];
  const maxIndex = blocks.length - 1;
  const safeIndex = Math.min(Math.max(terminalApprovalCurrentPatchIndex.value, 0), maxIndex);
  return blocks[safeIndex] || [];
});

const terminalApprovalPatchKinds = computed(() =>
  terminalApprovalPatchBlocks.value.map((lines) => getTerminalApprovalPatchKind(lines)),
);
const terminalApprovalReviewOpinion = computed(() => String(props.payload?.reviewOpinion || "").trim());

const terminalApprovalCurrentDiffLineCount = computed(() =>
  terminalApprovalCurrentPatchLines.value.reduce((count, rawLine) => {
    const line = String(rawLine || "");
    if (line.startsWith("+") && !line.startsWith("+++")) {
      return count + 1;
    }
    if (line.startsWith("-") && !line.startsWith("---")) {
      return count + 1;
    }
    return count;
  }, 0),
);

const terminalApprovalShowDiffOnly = computed(() =>
  String(props.payload?.approvalKind || "").trim() === "apply_patch_workspace_write",
);

const terminalApprovalShouldShowCodePreview = computed(() => {
  if (terminalApprovalShowDiffOnly.value) {
    return terminalApprovalCurrentDiffLineCount.value > 0;
  }
  return terminalApprovalPreviewLines.value.length > 0;
});

const terminalApprovalCurrentPatchLinesForSample = computed(() =>
  terminalApprovalShowDiffOnly.value ? terminalApprovalCurrentPatchLines.value : terminalApprovalPreviewLines.value,
);

const terminalApprovalPatchCount = computed(() => terminalApprovalPatchBlocks.value.length);

function handleClose() {
  emit("deny");
  emit("close");
}

function handleApprove() {
  emit("approve");
}

function goToTerminalApprovalPatch(index: number) {
  const maxIndex = Math.max(0, terminalApprovalPatchBlocks.value.length - 1);
  const nextIndex = Math.min(maxIndex, Math.max(0, Math.floor(index)));
  terminalApprovalCurrentPatchIndex.value = nextIndex;
}

function clampTerminalApprovalPatchIndex() {
  const total = terminalApprovalPatchBlocks.value.length;
  if (total <= 1) {
    terminalApprovalCurrentPatchIndex.value = 0;
    return;
  }
  const maxIndex = total - 1;
  if (terminalApprovalCurrentPatchIndex.value < 0) {
    terminalApprovalCurrentPatchIndex.value = 0;
    return;
  }
  if (terminalApprovalCurrentPatchIndex.value > maxIndex) {
    terminalApprovalCurrentPatchIndex.value = maxIndex;
  }
}

watch(
  () => props.payload?.requestId,
  () => {
    terminalApprovalCurrentPatchIndex.value = 0;
  },
);

watch(terminalApprovalPatchBlocks, clampTerminalApprovalPatchIndex);
</script>

<template>
  <dialog class="modal" :class="{ 'modal-open': open }">
    <div class="modal-box max-w-2xl">
      <h3 class="font-semibold text-base">
        {{ terminalApprovalDialogTitle }}
      </h3>
      <div v-if="props.payload?.reason" class="mt-3 rounded-box border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-base-content/80">
        {{ props.payload?.reason }}
      </div>
      <div
        v-if="terminalApprovalReviewOpinion"
        class="mt-3"
      >
        <div>{{ t("status.reviewOpinion") }}</div>
        <div class="min-h-24 max-h-56 overflow-y-auto whitespace-pre-wrap text-sm leading-7 text-base-content/80">
          {{ terminalApprovalReviewOpinion }}
        </div>
      </div>
      <TerminalApprovalImpactPanel
        :approval-kind="props.payload?.approvalKind"
        :command="props.payload?.command"
        :review-opinion="props.payload?.reviewOpinion"
        :impact-summary="terminalApprovalImpactSummary"
        :patch-kinds="terminalApprovalPatchKinds"
      />
      <div v-if="terminalApprovalShouldShowCodePreview" class="mt-4">
        <TerminalApprovalPatchSample
          :lines="terminalApprovalCurrentPatchLinesForSample"
          :diff-only="terminalApprovalShowDiffOnly"
        />
      </div>
      <div v-if="terminalApprovalPatchCount > 1" class="mt-3 flex justify-center">
        <div class="join">
          <button
            v-for="index in terminalApprovalPatchCount"
            :key="index"
            class="join-item"
            :class="[
              'btn btn-xs',
              terminalApprovalCurrentPatchIndex + 1 === index ? 'btn-active' : 'btn-ghost',
            ]"
            @click="goToTerminalApprovalPatch(index - 1)"
          >
            {{ index }}
          </button>
        </div>
      </div>
      <div v-if="(queueLength ?? 0) > 1" class="text-sm opacity-70 mt-2">
        {{ t("status.terminalApprovalQueueHint", { count: queueLength }) }}
      </div>
      <div class="modal-action justify-center">
        <button
          class="btn btn-sm btn-warning text-warning-content min-w-24"
          :disabled="resolving"
          @click="handleClose"
        >
          拒绝
        </button>
        <button
          class="btn btn-sm btn-primary text-primary-content min-w-24"
          :disabled="resolving"
          @click="handleApprove"
        >
          批准
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="handleClose">close</button>
    </form>
  </dialog>
</template>
