<template>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.language.label") }}</span></div>
    <select :value="uiLanguage" class="select select-bordered select-sm" @change="$emit('update:uiLanguage', ($event.target as HTMLSelectElement).value)">
      <option v-for="item in localeOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
    </select>
  </label>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.label") }}</span></div>
    <div class="flex items-center gap-2">
      <input :value="config.hotkey" class="input input-bordered input-sm flex-1" placeholder="Alt+·" readonly />
      <button
        class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200"
        :class="{ 'btn-primary': hotkeyCapturing }"
        @click="toggleHotkeyCapture"
      >
        {{ hotkeyCapturing ? t("config.hotkey.recording") : t("config.hotkey.recordButton") }}
      </button>
    </div>
    <div class="label py-1">
      <span class="label-text-alt text-[11px] opacity-70">{{ hotkeyCaptureHint }}</span>
    </div>
    <div v-if="hotkeyRecordConflict" class="text-xs text-error mt-1">{{ t("config.hotkey.conflict") }}</div>
  </label>
  <div class="grid grid-cols-1 gap-2">
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.recordKey") }}</span></div>
      <div class="flex items-center gap-1">
        <input :value="config.recordHotkey" class="input input-bordered input-sm flex-1" readonly />
        <button
          class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 shrink-0"
          :class="{ 'btn-primary': recordHotkeyCapturing }"
          @click="toggleRecordHotkeyCapture"
        >
          {{ recordHotkeyCapturing ? t("config.hotkey.recording") : t("config.hotkey.recordButton") }}
        </button>
      </div>
    </label>
    <div class="grid grid-cols-2 gap-2">
      <label class="form-control min-w-0">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.minRecordSeconds") }}</span></div>
        <input v-model.number="config.minRecordSeconds" type="number" min="1" max="30" class="input input-bordered input-sm w-full" />
      </label>
      <label class="form-control min-w-0">
        <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.maxRecordSeconds") }}</span></div>
        <input v-model.number="config.maxRecordSeconds" type="number" min="1" max="600" class="input input-bordered input-sm w-full" />
      </label>
    </div>
  </div>
  <div class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.recordTest") }}</span></div>
    <div class="flex flex-wrap items-center gap-2">
      <button
        class="btn btn-sm btn-ghost bg-base-100 shrink-0"
        :class="{ 'btn-error text-error-content': hotkeyTestRecording }"
        :title="hotkeyTestRecording ? t('config.hotkey.releaseToStop') : t('config.hotkey.holdToRecord')"
        @mousedown.prevent="$emit('startHotkeyRecordTest')"
        @mouseup.prevent="$emit('stopHotkeyRecordTest')"
        @mouseleave.prevent="hotkeyTestRecording && $emit('stopHotkeyRecordTest')"
        @touchstart.prevent="$emit('startHotkeyRecordTest')"
        @touchend.prevent="$emit('stopHotkeyRecordTest')"
      >
        {{ hotkeyTestRecording ? t("chat.recording", { seconds: Math.max(1, Math.round(hotkeyTestRecordingMs / 1000)) }) : t("config.hotkey.holdRecordButton") }}
      </button>
      <button
        class="btn btn-sm btn-ghost bg-base-100 shrink-0"
        :disabled="!hotkeyTestAudioReady"
        @click="$emit('playHotkeyRecordTest')"
      >
        {{ t("config.hotkey.playRecord") }}
      </button>
    </div>
  </div>
  <div class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.hotkey.theme") }}</span></div>
    <button class="btn btn-sm btn-ghost bg-base-100 w-full flex items-center justify-center gap-2" @click="$emit('toggleTheme')">
      <Sun v-if="currentTheme === 'light'" class="h-4 w-4" />
      <Moon v-else class="h-4 w-4" />
      <span>{{ currentTheme === "light" ? t("config.hotkey.lightTheme") : t("config.hotkey.darkTheme") }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Moon, Sun } from "lucide-vue-next";
import type { AppConfig } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  uiLanguage: "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "ja-JP" | "ko-KR"; label: string }>;
  currentTheme: "light" | "forest";
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
}>();

const emit = defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "toggleTheme"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
  (e: "captureHotkey", value: string): void;
}>();

const { t } = useI18n();

const hotkeyCapturing = ref(false);
const hotkeyCaptureHint = ref(t("config.hotkey.captureDefaultHint"));
let hotkeyCaptureHandler: ((event: KeyboardEvent) => void) | null = null;
const recordHotkeyCapturing = ref(false);
let recordHotkeyCaptureHandler: ((event: KeyboardEvent) => void) | null = null;

const hotkeyRecordConflict = computed(() => {
  const hotkey = String(props.config.hotkey || "").trim().toUpperCase();
  const recordHotkey = String(props.config.recordHotkey || "").trim().toUpperCase();
  if (!hotkey || !recordHotkey) return false;
  return hotkey === recordHotkey;
});

function isModifierKey(code: string): boolean {
  return code === "AltLeft"
    || code === "AltRight"
    || code === "ControlLeft"
    || code === "ControlRight"
    || code === "ShiftLeft"
    || code === "ShiftRight"
    || code === "MetaLeft"
    || code === "MetaRight";
}

function mainKeyFromEvent(event: KeyboardEvent): string {
  const code = event.code || "";
  if (code === "Backquote") return "·";
  if (code.startsWith("Key") && code.length === 4) return code.slice(3).toUpperCase();
  if (code.startsWith("Digit") && code.length === 6) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  if (code === "Minus") return "-";
  if (code === "Equal") return "=";
  if (code === "BracketLeft") return "[";
  if (code === "BracketRight") return "]";
  if (code === "Backslash") return "\\";
  if (code === "Semicolon") return ";";
  if (code === "Quote") return "'";
  if (code === "Comma") return ",";
  if (code === "Period") return ".";
  if (code === "Slash") return "/";
  if (code === "Space") return "Space";
  const key = event.key || "";
  if (key.length === 1) return key.toUpperCase();
  return key;
}

function stopHotkeyCapture() {
  hotkeyCapturing.value = false;
  if (hotkeyCaptureHandler) {
    window.removeEventListener("keydown", hotkeyCaptureHandler, true);
    hotkeyCaptureHandler = null;
  }
}

function stopRecordHotkeyCapture() {
  recordHotkeyCapturing.value = false;
  if (recordHotkeyCaptureHandler) {
    window.removeEventListener("keydown", recordHotkeyCaptureHandler, true);
    recordHotkeyCaptureHandler = null;
  }
}

function startHotkeyCapture() {
  if (hotkeyCapturing.value) return;
  hotkeyCapturing.value = true;
  hotkeyCaptureHint.value = t("config.hotkey.captureListeningHint");
  hotkeyCaptureHandler = (event: KeyboardEvent) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      hotkeyCaptureHint.value = t("config.hotkey.captureCancelledHint");
      stopHotkeyCapture();
      return;
    }

    const modifiers: string[] = [];
    if (event.ctrlKey) modifiers.push("Ctrl");
    if (event.altKey) modifiers.push("Alt");
    if (event.shiftKey) modifiers.push("Shift");
    if (event.metaKey) modifiers.push("Meta");

    if (isModifierKey(event.code)) {
      hotkeyCaptureHint.value = t("config.hotkey.captureNeedMainKeyHint");
      return;
    }
    if (modifiers.length === 0) {
      hotkeyCaptureHint.value = t("config.hotkey.captureNeedModifierHint");
      return;
    }

    const main = mainKeyFromEvent(event).trim();
    if (!main) {
      hotkeyCaptureHint.value = t("config.hotkey.captureUnrecognizedHint");
      return;
    }
    const combo = `${modifiers.join("+")}+${main}`;
    emit("captureHotkey", combo);
    hotkeyCaptureHint.value = t("config.hotkey.captureCapturedHint", { combo });
    stopHotkeyCapture();
  };
  window.addEventListener("keydown", hotkeyCaptureHandler, true);
}

function startRecordHotkeyCapture() {
  if (recordHotkeyCapturing.value) return;
  recordHotkeyCapturing.value = true;
  recordHotkeyCaptureHandler = (event: KeyboardEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      stopRecordHotkeyCapture();
      return;
    }

    const modifiers: string[] = [];
    if (event.ctrlKey) modifiers.push("Ctrl");
    if (event.altKey) modifiers.push("Alt");
    if (event.shiftKey) modifiers.push("Shift");
    if (event.metaKey) modifiers.push("Meta");

    if (isModifierKey(event.code)) {
      const modifierOnly = modifiers[0];
      if (modifiers.length === 1 && modifierOnly) {
        props.config.recordHotkey = modifierOnly;
        stopRecordHotkeyCapture();
      }
      return;
    }

    const main = mainKeyFromEvent(event).trim();
    if (!main) return;
    props.config.recordHotkey = modifiers.length > 0 ? `${modifiers.join("+")}+${main}` : main;
    stopRecordHotkeyCapture();
  };
  window.addEventListener("keydown", recordHotkeyCaptureHandler, true);
}

function toggleHotkeyCapture() {
  if (hotkeyCapturing.value) {
    hotkeyCaptureHint.value = t("config.hotkey.captureCancelledHint");
    stopHotkeyCapture();
    return;
  }
  startHotkeyCapture();
}

function toggleRecordHotkeyCapture() {
  if (recordHotkeyCapturing.value) {
    stopRecordHotkeyCapture();
    return;
  }
  startRecordHotkeyCapture();
}

onBeforeUnmount(() => {
  stopHotkeyCapture();
  stopRecordHotkeyCapture();
});
</script>
