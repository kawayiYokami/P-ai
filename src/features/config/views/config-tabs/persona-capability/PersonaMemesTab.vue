<script setup lang="ts">
import { ref, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, X } from "@lucide/vue";
import ConfigCard from "../../../components/ConfigCard.vue";
import { invokeTauri } from "../../../../../services/tauri-api";
import type { PersonaProfile } from "../../../../../types/app";

type MemeItem = {
  name: string;
  fileName: string;
  relativePath: string;
  dataUrl: string;
};

const props = defineProps<{
  persona: PersonaProfile;
}>();

const { t } = useI18n();

const items = ref<MemeItem[]>([]);
const loading = ref(false);
const loadError = ref("");
const busy = ref(false);
const statusText = ref("");
const statusError = ref(false);
const isDragging = ref(false);
const fileInput = ref<HTMLInputElement | null>(null);
const renamingName = ref("");
const renameDraft = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

function setStatus(text: string, isError = false) {
  statusText.value = text;
  statusError.value = isError;
}

async function loadMemes() {
  const agentId = props.persona?.id;
  if (!agentId) return;
  loading.value = true;
  loadError.value = "";
  try {
    items.value = await invokeTauri<MemeItem[]>("list_agent_memes", {
      input: { agentId },
    });
  } catch (error) {
    loadError.value = String((error as Error)?.message || error);
    items.value = [];
  } finally {
    loading.value = false;
  }
}

function pickImage() {
  if (busy.value || loading.value) return;
  fileInput.value?.click();
}

async function uploadFiles(files: FileList | File[]) {
  const validFiles = Array.from(files).filter(
    (file) => file.type.startsWith("image/") || /\.(png|jpe?g|gif|webp|bmp|ico|svg)$/i.test(file.name),
  );
  if (validFiles.length === 0) return;

  busy.value = true;
  let addedCount = 0;
  let lastItemName = "";
  try {
    for (const file of validFiles) {
      const bytesBase64 = await readFileAsBase64(file);
      const item = await invokeTauri<MemeItem>("add_agent_meme", {
        input: {
          agentId: props.persona.id,
          fileName: file.name,
          bytesBase64,
        },
      });
      addedCount++;
      lastItemName = item.name;
    }
    if (addedCount === 1) {
      setStatus(t("config.persona.memes.added", { name: lastItemName }));
    } else {
      setStatus(t("config.persona.memes.batchAdded", { count: addedCount }));
    }
    await loadMemes();
  } catch (error) {
    setStatus(String((error as Error)?.message || error), true);
  } finally {
    busy.value = false;
  }
}

async function onFilePicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = input.files;
  if (!files || files.length === 0) return;
  await uploadFiles(files);
  input.value = "";
}

function onDrop(event: DragEvent) {
  isDragging.value = false;
  const files = event.dataTransfer?.files;
  if (files && files.length > 0) {
    void uploadFiles(files);
  }
}

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error || new Error("read file failed"));
    reader.readAsDataURL(file);
  });
}

async function deleteItem(item: MemeItem) {
  if (!window.confirm(t("config.persona.memes.confirmDelete", { name: item.name }))) return;
  busy.value = true;
  try {
    await invokeTauri("delete_agent_meme", {
      input: { agentId: props.persona.id, name: item.name },
    });
    setStatus(t("config.persona.memes.deleted", { name: item.name }));
    await loadMemes();
  } catch (error) {
    setStatus(String((error as Error)?.message || error), true);
  } finally {
    busy.value = false;
  }
}

function startRename(item: MemeItem) {
  renamingName.value = item.name;
  renameDraft.value = item.name;
  void nextTick(() => {
    renameInputRef.value?.focus();
    renameInputRef.value?.select();
  });
}

function cancelRename() {
  renamingName.value = "";
  renameDraft.value = "";
}

async function submitRename(item: MemeItem) {
  const next = renameDraft.value.trim();
  if (!next || next === item.name) {
    cancelRename();
    return;
  }
  busy.value = true;
  try {
    await invokeTauri("rename_agent_meme", {
      input: { agentId: props.persona.id, name: item.name, newName: next },
    });
    setStatus(t("config.persona.memes.renamed", { from: item.name, to: next }));
    cancelRename();
    await loadMemes();
  } catch (error) {
    setStatus(String((error as Error)?.message || error), true);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.persona?.id,
  () => {
    statusText.value = "";
    statusError.value = false;
    cancelRename();
    void loadMemes();
  },
  { immediate: true },
);
</script>

<template>
  <div class="grid gap-5 min-w-0 max-w-full">
    <ConfigCard :title="t('config.persona.memes.title')" flush>
      <template #actions>
        <span v-if="items.length > 0" class="text-caption text-base-content/50">
          {{ t("config.persona.memes.summary", { count: items.length }) }}
        </span>
      </template>

      <div
        class="p-4 sm:p-5 min-w-0 transition-colors"
        :class="isDragging ? 'bg-primary/5' : ''"
        @dragover.prevent="isDragging = true"
        @dragleave.prevent="isDragging = false"
        @drop.prevent="onDrop"
      >
        <!-- 状态反馈条 -->
        <div
          v-if="statusText"
          class="mb-3 flex items-center justify-between gap-2 px-3 py-1.5 rounded-lg text-caption"
          :class="statusError ? 'bg-error/10 text-error' : 'bg-success/10 text-success'"
        >
          <span class="truncate">{{ statusText }}</span>
          <button
            type="button"
            class="btn btn-ghost btn-xs btn-circle shrink-0"
            @click="statusText = ''"
          >
            <X class="h-3 w-3" />
          </button>
        </div>

        <!-- 读取状态 -->
        <div v-if="loading" class="py-12 text-center text-caption text-base-content/50">
          {{ t("config.persona.memes.loading") }}
        </div>

        <!-- 错误状态 -->
        <div v-else-if="loadError" class="py-8 text-center text-caption text-error">
          {{ loadError }}
        </div>

        <!-- 纯净表情网格：首格为 [+] 添加，后跟纯净表情贴纸 -->
        <div
          v-else
          class="grid grid-cols-[repeat(auto-fill,minmax(76px,1fr))] gap-3 sm:gap-4 min-w-0"
        >
          <!-- [+] 虚线添加格（参考主流聊天应用贴纸库首格） -->
          <button
            type="button"
            class="group flex flex-col items-center gap-1.5 cursor-pointer focus:outline-hidden"
            :disabled="busy"
            @click="pickImage"
          >
            <div
              class="w-full aspect-square rounded-2xl border-2 border-dashed border-base-300 group-hover:border-primary group-hover:bg-primary/5 flex items-center justify-center transition-colors"
            >
              <Plus class="h-7 w-7 text-base-content/30 group-hover:text-primary transition-colors" />
            </div>
          </button>

          <!-- 表情贴纸项 -->
          <div
            v-for="item in items"
            :key="item.name"
            class="group relative flex flex-col items-center gap-1 min-w-0 select-none"
          >
            <!-- 贴纸图像区 -->
            <div class="relative w-full aspect-square flex items-center justify-center">
              <img
                :src="item.dataUrl"
                :alt="item.name"
                :title="item.fileName"
                class="max-h-full max-w-full object-contain pointer-events-none rounded"
              />

              <!-- 右上角轻量删除按钮（悬停浮现，不占版面） -->
              <button
                type="button"
                class="btn btn-circle btn-xs h-5 w-5 min-h-5 absolute -top-1 -right-1 bg-base-100/95 hover:bg-error hover:text-white border border-base-300 shadow-xs opacity-0 group-hover:opacity-100 transition-opacity"
                :title="t('config.persona.memes.delete')"
                :disabled="busy"
                @click.stop="deleteItem(item)"
              >
                <X class="h-3 w-3" />
              </button>
            </div>

            <!-- 表情名（点击直接内联编辑，绝不多余堆砌按钮） -->
            <div class="w-full min-w-0">
              <input
                v-if="renamingName === item.name"
                ref="renameInputRef"
                v-model="renameDraft"
                class="input input-xs input-bordered w-full text-center text-caption px-1 h-5 min-h-5"
                :disabled="busy"
                @blur="submitRename(item)"
                @keydown.enter.prevent="submitRename(item)"
                @keydown.esc.prevent="cancelRename"
              />
              <div
                v-else
                class="text-caption text-base-content/75 hover:text-primary text-center truncate cursor-pointer px-0.5 leading-tight"
                :title="`${item.name} (${t('config.persona.memes.clickToRename')})`"
                @click="startRename(item)"
              >
                {{ item.name }}
              </div>
            </div>
          </div>
        </div>

        <div class="text-caption text-base-content/40 leading-relaxed pt-3">
          {{ t("config.persona.memes.hint") }}
        </div>
      </div>
    </ConfigCard>

    <input
      ref="fileInput"
      type="file"
      accept="image/*"
      multiple
      class="hidden"
      @change="onFilePicked"
    />
  </div>
</template>
