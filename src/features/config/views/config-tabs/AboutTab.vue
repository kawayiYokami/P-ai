<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("about.version") }}</h3>
        <p class="text-sm mb-3">{{ `P-ai v${appVersion}` }}</p>
        <div class="mb-3 space-y-2">
          <div class="text-xs font-medium text-base-content/70">{{ t("about.updateMethod") }}</div>
          <div class="tabs tabs-box bg-base-200 p-1">
            <button
              v-for="option in updateMethodOptions"
              :key="option.value"
              type="button"
              class="tab flex-1 rounded-btn"
              :class="normalizedGithubUpdateMethod === option.value ? 'tab-active' : ''"
              @click="setGithubUpdateMethod(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
        <button
          class="btn btn-sm"
          :disabled="checkingUpdate"
          @click="handleCheckUpdate"
        >{{ checkingUpdate ? t("common.loading") : t("about.checkUpdate") }}</button>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("about.repository") }}</h3>
        <button
          class="btn"
          @click="openRepository"
        >{{ t("about.repository") }}</button>
      </div>
    </div>
  </div>

  <dialog class="modal" :class="{ 'modal-open': updateDialogOpen }">
    <div class="modal-box">
      <h3 class="font-bold text-lg">{{ updateDialogTitle }}</h3>
      <pre class="mt-2 whitespace-pre-wrap text-sm">{{ updateDialogBody }}</pre>
      <div class="modal-action">
        <button
          v-if="updateDialogReleaseUrl"
          class="btn"
          @click="openUpdateRelease"
        >打开 Releases</button>
        <button class="btn" @click="closeUpdateDialog">知道了</button>
      </div>
    </div>
  </dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import type { GithubUpdateMethod } from "../../../../types/app";

const props = defineProps<{
  githubUpdateMethod: GithubUpdateMethod;
  checkingUpdate: boolean;
}>();

const emit = defineEmits<{
  (e: "update:githubUpdateMethod", value: GithubUpdateMethod): void;
  (e: "checkUpdate"): void;
}>();

const { t } = useI18n();

const updateDialogOpen = ref(false);
const updateDialogTitle = ref("检查更新");
const updateDialogBody = ref("");
const updateDialogReleaseUrl = ref("");
const appVersion = ref("...");
const updateMethodOptions = computed<Array<{ value: GithubUpdateMethod; label: string }>>(() => [
  { value: "auto", label: t("about.updateMethodAuto") },
  { value: "direct", label: t("about.updateMethodDirect") },
  { value: "proxy", label: t("about.updateMethodProxy") },
]);
const normalizedGithubUpdateMethod = computed<GithubUpdateMethod>(() => {
  const value = props.githubUpdateMethod;
  return value === "direct" || value === "proxy" ? value : "auto";
});

onMounted(async () => {
  try {
    appVersion.value = await invokeTauri<string>("get_app_version");
  } catch (error) {
    console.warn("[AboutTab] load app version failed:", error);
    appVersion.value = "unknown";
  }
});

async function openRepository() {
  try {
    const url = await invokeTauri<string>("get_project_repository_url");
    void invokeTauri("open_external_url", { url });
  } catch (error) {
    console.warn("[AboutTab] resolve project repository failed:", error);
  }
}

function handleCheckUpdate() {
  emit("checkUpdate");
}

function setGithubUpdateMethod(value: GithubUpdateMethod) {
  emit("update:githubUpdateMethod", value);
}

function openUpdateRelease() {
  if (updateDialogReleaseUrl.value) {
    void invokeTauri("open_external_url", { url: updateDialogReleaseUrl.value });
  }
}

function closeUpdateDialog() {
  updateDialogOpen.value = false;
}

function showUpdateDialog(text: string, releaseUrl?: string) {
  updateDialogTitle.value = t("about.checkUpdate");
  updateDialogBody.value = text;
  updateDialogReleaseUrl.value = releaseUrl || "";
  updateDialogOpen.value = true;
}

defineExpose({
  showUpdateDialog,
});
</script>


