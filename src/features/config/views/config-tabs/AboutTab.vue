<template>
  <div class="grid gap-3">
    <ConfigCard :title="t('about.version')">
      <template #actions>
        <button
          class="btn btn-sm btn-primary text-primary-content"
          @click="openRepository"
        >{{ t("about.repository") }}</button>
        <button
          class="btn btn-sm btn-secondary text-secondary-content"
          :disabled="checkingUpdate"
          @click="handleCheckUpdate"
        >{{ checkingUpdate ? t("common.loading") : t("about.checkUpdate") }}</button>
      </template>
      <div class="py-3">
        <div class="mb-3 flex items-center justify-between gap-2">
          <div class="text-xs font-medium text-base-content/70">{{ t("about.updateMethod") }}</div>
          <div class="tabs tabs-box bg-base-200 p-1">
            <button
              v-for="option in updateMethodOptions"
              :key="option.value"
              type="button"
              class="tab rounded-btn"
              :class="normalizedGithubUpdateMethod === option.value ? 'tab-active' : ''"
              @click="setGithubUpdateMethod(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
        <p class="text-sm">{{ `P-ai v${appVersion}` }}</p>
      </div>
    </ConfigCard>

    <ConfigCard :title="t('about.changelog')">
      <template #actions>
        <button
          class="btn btn-sm btn-ghost"
          :disabled="changelogLoading"
          @click="loadProjectChangelog(true)"
        >
          <span v-if="changelogLoading" class="loading loading-spinner loading-xs"></span>
          {{ t("common.refresh") }}
        </button>
      </template>
      <div class="py-3">
          <div class="config-changelog-markdown max-h-[60vh] overflow-auto">
            <div v-if="changelogLoading && !changelogMarkdown" class="flex min-h-0 items-center justify-center py-8 text-sm text-base-content/70">
              <span class="loading loading-spinner loading-sm mr-2"></span>
              {{ t("about.changelogLoading") }}
            </div>
            <div v-else-if="changelogError" class="rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
              {{ changelogError }}
            </div>
            <AppMarkdownRenderer
              v-else-if="changelogMarkdown"
              class="ecall-markdown-content max-w-none"
              :text="changelogMarkdown"
              :is-dark="markdownIsDark"
              variant="document"
            />
            <div v-else class="flex min-h-0 items-center justify-center py-8 text-sm text-base-content/70">
              {{ t("about.changelogEmpty") }}
            </div>
          </div>
      </div>
    </ConfigCard>
  </div>

  <dialog ref="updateDialogRef" class="modal" @close="closeUpdateDialog" @cancel.prevent="closeUpdateDialog">
    <div class="modal-box">
      <h3 class="font-bold text-lg">{{ updateDialogTitle }}</h3>
      <pre class="mt-2 whitespace-pre-wrap text-sm">{{ updateDialogBody }}</pre>
      <div class="modal-action">
        <button
          v-if="updateDialogReleaseUrl"
          class="btn"
          @click="openUpdateRelease"
        >{{ t("dialogs.update.openReleases") }}</button>
        <button class="btn" @click="closeUpdateDialog">{{ t("common.confirm") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeUpdateDialog">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri, openTransportExternalUrl } from "../../../../services/tauri-api";
import ConfigCard from "../../components/ConfigCard.vue";
import { AppMarkdownRenderer } from "../../../chat/markdown";
import { isDarkAppTheme } from "../../../shell/composables/use-app-theme";
import type { GithubUpdateMethod } from "../../../../types/app";

const props = defineProps<{
  githubUpdateMethod: GithubUpdateMethod;
  checkingUpdate: boolean;
  currentTheme: string;
}>();

const emit = defineEmits<{
  (e: "update:githubUpdateMethod", value: GithubUpdateMethod): void;
  (e: "checkUpdate"): void;
}>();

const { t } = useI18n();

const updateDialogRef = ref<HTMLDialogElement | null>(null);
const updateDialogOpen = ref(false);
const updateDialogTitle = ref("");
const updateDialogBody = ref("");
const updateDialogReleaseUrl = ref("");

function syncUpdateDialog() {
  const d = updateDialogRef.value;
  if (!d) return;
  if (updateDialogOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(updateDialogOpen, syncUpdateDialog);
watch(updateDialogRef, syncUpdateDialog);
const appVersion = ref("...");
const changelogLoading = ref(false);
const changelogError = ref("");
const changelogMarkdown = ref("");
const changelogLoaded = ref(false);
const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));
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
    console.warn("[关于] load app version failed:", error);
    appVersion.value = "unknown";
  }
  void loadProjectChangelog();
});

async function loadProjectChangelog(force = false) {
  if (changelogLoading.value) return;
  if (changelogLoaded.value && !force) return;
  changelogLoading.value = true;
  changelogError.value = "";
  try {
    changelogMarkdown.value = await invokeTauri<string>("fetch_project_changelog_markdown");
    changelogLoaded.value = true;
  } catch (error) {
    changelogError.value = String(error);
  } finally {
    changelogLoading.value = false;
  }
}

async function openRepository() {
  try {
    const url = await invokeTauri<string>("get_project_repository_url");
    void openTransportExternalUrl(url);
  } catch (error) {
    console.warn("[关于] resolve project repository failed:", error);
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
    void openTransportExternalUrl(updateDialogReleaseUrl.value);
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

<style scoped>
.config-changelog-markdown:deep(.ecall-markdown-content.prose) {
  max-width: none;
}

.config-changelog-markdown:deep(.ecall-markdown-content) {
  color: inherit;
  line-height: 1.75;
  font-size: var(--app-text-base-size);
}

.config-changelog-markdown:deep(.ecall-markdown-content :where(p,ul,ol,blockquote,pre,table,figure,.paragraph-node,.list-node,.blockquote,.table-node-wrapper,.code-block-container,._mermaid,.vmr-container)) {
  margin-top: 0.85rem;
  margin-bottom: 0.85rem;
}

.config-changelog-markdown:deep(.ecall-markdown-content :where(h1,h2,h3,h4,.heading-node)) {
  margin-top: 1.25rem;
  margin-bottom: 0.75rem;
  font-weight: 700;
}

.config-changelog-markdown:deep(.ecall-markdown-content :where(a,.link-node)) {
  color: hsl(var(--p));
  text-decoration: underline;
}

.config-changelog-markdown:deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  padding-left: 0.9rem;
  opacity: 0.9;
}

.config-changelog-markdown:deep(.ecall-markdown-content :where(:not(pre) > code,.inline-code)) {
  border: 0;
  border-radius: 0.45rem;
  padding: 0.08rem 0.35rem;
  color: var(--ecall-md-inline-code-color);
  background: transparent;
}
</style>


