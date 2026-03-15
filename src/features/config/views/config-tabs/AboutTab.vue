<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("about.version") }}</h3>
        <p class="text-sm mb-3">{{ `P-ai v${appVersion}` }}</p>
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
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";

defineProps<{
  checkingUpdate: boolean;
}>();

const emit = defineEmits<{
  (e: "checkUpdate"): void;
}>();

const { t } = useI18n();

const updateDialogOpen = ref(false);
const updateDialogTitle = ref("检查更新");
const updateDialogBody = ref("");
const updateDialogReleaseUrl = ref("");
const appVersion = ref("...");

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


