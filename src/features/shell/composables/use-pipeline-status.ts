import { ref, onMounted, onUnmounted, type Ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * 三色语义（Anton 设计）：
 *   🟢 green  = idle，系统正常
 *   🟡 yellow = busy（处理中 + 脉冲）/ warning（有过异常或有新版本就绪，无脉冲）
 *   🔴 red    = error，故障（请求失败等），5 秒后自动恢复为 warning
 */
export type PipelineStatus = "idle" | "busy" | "warning" | "error";

export interface PipelineStageEvent {
  stage: string;
  label: string;
  elapsed_ms: number;
  conversation_id: string;
  status: "idle" | "busy" | "error";
  request_id?: string;
}

export interface PipelineState {
  status: Ref<PipelineStatus>;
  label: Ref<string>;
  elapsedMs: Ref<number>;
  isBusy: Ref<boolean>;
}

export function usePipelineStatus(): PipelineState {
  const status = ref<PipelineStatus>("idle");
  const label = ref("");
  const elapsedMs = ref(0);
  const isBusy = ref(false);

  let hasWarning = false;
  let latestRequestId: string | undefined;
  let unlisten: UnlistenFn | null = null;
  let errorAutoRecoverTimer: ReturnType<typeof setTimeout> | null = null;

  onMounted(async () => {
    unlisten = await listen<PipelineStageEvent>("pipeline_stage", (event) => {
      const payload = event.payload;
      // Ignore events from stale requests — only accept the latest request_id
      if (payload.request_id) {
        if (latestRequestId && payload.request_id !== latestRequestId && payload.status !== "busy") {
          return;
        }
        latestRequestId = payload.request_id;
      }
      label.value = payload.label;
      elapsedMs.value = payload.elapsed_ms;

      if (errorAutoRecoverTimer) {
        clearTimeout(errorAutoRecoverTimer);
        errorAutoRecoverTimer = null;
      }

      if (payload.status === "error") {
        status.value = "error";
        isBusy.value = false;
        hasWarning = true;
        errorAutoRecoverTimer = setTimeout(() => {
          if (status.value === "error") {
            status.value = "warning";
            label.value = "";
          }
          errorAutoRecoverTimer = null;
        }, 5000);
      } else if (payload.status === "busy") {
        status.value = "busy";
        isBusy.value = true;
      } else {
        if (hasWarning) {
          status.value = "warning";
        } else {
          status.value = "idle";
        }
        isBusy.value = false;
      }
    });
  });

  onUnmounted(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    if (errorAutoRecoverTimer) {
      clearTimeout(errorAutoRecoverTimer);
      errorAutoRecoverTimer = null;
    }
  });

  return { status, label, elapsedMs, isBusy };
}