<template>
  <button
    type="button"
    class="btn btn-sm min-h-[2.25rem] bg-base-100 gap-1.5 px-3"
    :title="t('config.remoteIm.contactBehavior')"
    :disabled="!channel"
    @click="openModal"
  >
    <SlidersHorizontal class="h-4 w-4" />
    <span>{{ t('config.remoteIm.contactBehavior') }}</span>
  </button>

  <dialog
    ref="dialogRef"
    class="modal"
    @close="onDialogClose"
    @cancel.prevent="onDialogClose"
  >
    <div class="modal-box flex w-full max-w-3xl max-h-[85vh] min-w-0 flex-col overflow-hidden p-0">
        <header class="flex shrink-0 items-start justify-between gap-4 border-b border-base-300 px-5 py-4">
          <div class="min-w-0">
            <h3 class="font-semibold">{{ t('config.remoteIm.channelBehaviorSettings') }}</h3>
            <p class="mt-1 text-xs text-base-content/60">{{ t('config.remoteIm.channelBehaviorSettingsHint') }}</p>
          </div>
          <button type="button" class="btn btn-circle btn-sm btn-ghost" :title="t('common.close')" @click="closeModal">×</button>
        </header>

        <div class="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto px-5 py-4 space-y-6">
          <div v-if="error" class="alert alert-error py-2 text-xs break-all whitespace-pre-wrap" style="overflow-wrap:anywhere">{{ error }}</div>
          <p v-if="validationError" class="alert alert-warning py-2 text-xs break-all whitespace-pre-wrap" style="overflow-wrap:anywhere">{{ validationError }}</p>

          <!-- 消息策略 -->
          <section class="space-y-4 min-w-0">
            <h4 class="text-sm font-semibold">{{ t('config.remoteIm.channelBehaviorMessagePolicySection') }}</h4>
            <label class="form-control w-full min-w-0">
              <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.responseGuidance') }}</span></div>
              <textarea v-model="draft.responseGuidance" :placeholder="t('config.remoteIm.responseGuidancePlaceholder')" class="textarea textarea-bordered textarea-sm min-h-28 w-full break-all" style="overflow-wrap:anywhere; white-space:pre-wrap"></textarea>
              <div class="py-1"><span class="text-xs text-base-content/50 break-all" style="overflow-wrap:anywhere">{{ t('config.remoteIm.responseGuidanceHint') }}</span></div>
            </label>
            <label class="form-control w-full min-w-0">
              <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.blockedMessagePrefixes') }}</span></div>
              <input v-model="draft.blockedMessagePrefixesText" :placeholder="t('config.remoteIm.blockedMessagePrefixesPlaceholder')" type="text" class="input input-bordered input-sm w-full min-w-0" />
              <div class="py-1"><span class="text-xs text-base-content/50 break-all" style="overflow-wrap:anywhere">{{ t('config.remoteIm.blockedMessagePrefixesHint') }}</span></div>
            </label>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.muteKeywords') }}</span></div>
                <input v-model="draft.muteKeywordsText" :placeholder="t('config.remoteIm.muteKeywordsPlaceholder')" type="text" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.unmuteKeywords') }}</span></div>
                <input v-model="draft.unmuteKeywordsText" :placeholder="t('config.remoteIm.unmuteKeywordsPlaceholder')" type="text" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.muteDuration') }}</span></div>
                <input v-model.number="draft.muteDurationSeconds" type="number" min="0" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.patienceExit') }}</span></div>
                <input v-model.number="draft.patienceSeconds" type="number" min="0" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
          </section>

          <div class="divider my-0"></div>

          <!-- 看群频率 -->
          <section class="space-y-4 min-w-0">
            <h4 class="text-sm font-semibold">{{ t('config.remoteIm.channelBehaviorViewFrequencySection') }}</h4>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.assistantDebounceSeconds') }}</span></div>
                <input v-model.number="draft.pacing.assistantDebounceSeconds" type="number" min="1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.secretaryInspectionSeconds') }}</span></div>
                <input v-model.number="draft.pacing.secretaryInspectionSeconds" type="number" min="1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.replyCooldownSeconds') }}</span></div>
                <input v-model.number="draft.pacing.replyCooldownSeconds" type="number" min="0" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.inspectionJitterRatio') }}</span></div>
                <input v-model.number="draft.pacing.inspectionJitterRatio" type="number" min="0" max="1" step="0.05" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
          </section>

          <div class="divider my-0"></div>

          <!-- 能量 -->
          <section class="space-y-4 min-w-0">
            <h4 class="text-sm font-semibold">{{ t('config.remoteIm.channelBehaviorReplyEnthusiasmSection') }}</h4>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.maximumEnergy') }}</span></div>
                <input v-model.number="draft.pacing.maximumEnergy" type="number" min="0.01" step="1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.energyRecoveryPerSecond') }}</span></div>
                <input v-model.number="draft.pacing.energyRecoveryPerSecond" type="number" min="0" step="0.01" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.baseReplyEnergyCost') }}</span></div>
                <input v-model.number="draft.pacing.baseReplyEnergyCost" type="number" min="0" step="0.1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.energyCostPerCharacter') }}</span></div>
                <input v-model.number="draft.pacing.energyCostPerCharacter" type="number" min="0" step="0.01" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.positiveEnergyPhrases') }}</span></div>
                <input v-model="draft.positiveEnergyPhrasesText" type="text" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.positiveEnergyDelta') }}</span></div>
                <input v-model.number="draft.pacing.positiveEnergyDelta" type="number" min="0" step="0.1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.negativeEnergyPhrases') }}</span></div>
                <input v-model="draft.negativeEnergyPhrasesText" type="text" class="input input-bordered input-sm w-full min-w-0" />
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.negativeEnergyDelta') }}</span></div>
                <input v-model.number="draft.pacing.negativeEnergyDelta" type="number" max="0" step="0.1" class="input input-bordered input-sm w-full min-w-0" />
              </label>
            </div>
          </section>

          <div class="divider my-0"></div>

          <!-- 回复长度 -->
          <section class="space-y-4 min-w-0">
            <h4 class="text-sm font-semibold">{{ t('config.remoteIm.channelBehaviorReplyLengthSection') }}</h4>
            <label class="form-control w-full min-w-0">
              <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.focusInstructions') }}</span></div>
              <input v-model="draft.focusInstructionsText" type="text" class="input input-bordered input-sm w-full min-w-0" :placeholder="t('config.remoteIm.focusInstructions')" />
            </label>
            <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.normalReplyMaxChars') }}</span></div>
                <input v-model.number="draft.pacing.normalReplyMaxChars" type="number" min="1" class="input input-bordered input-sm w-full min-w-0" />
                <div class="py-1"><span class="text-xs text-base-content/50 break-all" style="overflow-wrap:anywhere">{{ t('config.remoteIm.normalReminderPreview', { count: draft.pacing.normalReplyMaxChars }) }}</span></div>
              </label>
              <label class="form-control w-full min-w-0">
                <div class="py-1"><span class="text-sm">{{ t('config.remoteIm.focusReplyMaxChars') }}</span></div>
                <input v-model.number="draft.pacing.focusReplyMaxChars" type="number" min="1" class="input input-bordered input-sm w-full min-w-0" />
                <div class="py-1"><span class="text-xs text-base-content/50 break-all" style="overflow-wrap:anywhere">{{ t('config.remoteIm.focusReminderPreview', { count: draft.pacing.focusReplyMaxChars }) }}</span></div>
              </label>
            </div>
          </section>
        </div>

        <footer class="flex shrink-0 flex-wrap justify-end gap-2 border-t border-base-300 px-5 py-4">
          <button type="button" class="btn btn-ghost" :disabled="saving" @click="restoreDefaults">{{ t('config.remoteIm.restoreBehaviorDefaults') }}</button>
          <button type="button" class="btn btn-ghost" :disabled="saving || !savedSnapshot" @click="restoreSaved">{{ t('config.remoteIm.restoreBehaviorSaved') }}</button>
          <button type="button" class="btn btn-primary" :disabled="saving || !dirty || !!validationError" @click="save">
            <span v-if="saving" class="loading loading-spinner loading-xs"></span>
            <Save v-else class="h-3.5 w-3.5" />{{ t('common.save') }}
          </button>
        </footer>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Save, SlidersHorizontal } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../../services/tauri-api";
import type { RemoteImChannelBehaviorSettings, RemoteImChannelConfig, RemoteImGroupReplyPacing } from "../../../../../types/app";
import {
  cloneChannelBehaviorSettings,
  DEFAULT_REMOTE_IM_CHANNEL_BEHAVIOR_SETTINGS,
  normalizeGroupReplyPacing,
  parseSpaceSeparatedList,
} from "./helpers";

const props = defineProps<{
  channel: RemoteImChannelConfig | null;
  saveConfigAction: () => Promise<boolean> | boolean;
  setStatusAction: (text: string) => void;
}>();
const { t } = useI18n();

type Draft = {
  responseGuidance: string;
  blockedMessagePrefixesText: string;
  muteKeywordsText: string;
  unmuteKeywordsText: string;
  patienceSeconds: number;
  muteDurationSeconds: number;
  positiveEnergyPhrasesText: string;
  negativeEnergyPhrasesText: string;
  focusInstructionsText: string;
  pacing: RemoteImGroupReplyPacing;
};

function draftFromSettings(value?: Partial<RemoteImChannelBehaviorSettings> | null): Draft {
  const settings = cloneChannelBehaviorSettings(value);
  const pacing = normalizeGroupReplyPacing(settings.groupReplyPacing);
  return {
    responseGuidance: settings.responseGuidance,
    blockedMessagePrefixesText: settings.blockedMessagePrefixes.join(" "),
    muteKeywordsText: settings.muteKeywords.join(" "),
    unmuteKeywordsText: settings.unmuteKeywords.join(" "),
    patienceSeconds: settings.patienceSeconds,
    muteDurationSeconds: settings.muteDurationSeconds,
    positiveEnergyPhrasesText: pacing.positiveEnergyPhrases.join(" "),
    negativeEnergyPhrasesText: pacing.negativeEnergyPhrases.join(" "),
    focusInstructionsText: pacing.focusInstructions.join(" "),
    pacing,
  };
}

function settingsFromDraft(value: Draft): RemoteImChannelBehaviorSettings {
  return {
    responseGuidance: String(value.responseGuidance || "").trim(),
    blockedMessagePrefixes: parseSpaceSeparatedList(value.blockedMessagePrefixesText),
    muteKeywords: parseSpaceSeparatedList(value.muteKeywordsText),
    unmuteKeywords: parseSpaceSeparatedList(value.unmuteKeywordsText),
    patienceSeconds: Math.max(0, Math.floor(Number(value.patienceSeconds) || 0)),
    muteDurationSeconds: Math.max(0, Math.floor(Number(value.muteDurationSeconds) || 0)),
    groupReplyPacing: {
      ...value.pacing,
      positiveEnergyPhrases: parseSpaceSeparatedList(value.positiveEnergyPhrasesText),
      negativeEnergyPhrases: parseSpaceSeparatedList(value.negativeEnergyPhrasesText),
      focusInstructions: parseSpaceSeparatedList(value.focusInstructionsText),
    },
  };
}

const dialogRef = ref<HTMLDialogElement | null>(null);
const open = ref(false);

function onDialogClose() {
  if (saving.value) {
    const d = dialogRef.value;
    if (d && !d.open && open.value) d.showModal();
    return;
  }
  closeModal();
}

function syncDialog() {
  const d = dialogRef.value;
  if (!d) return;
  if (open.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(open, syncDialog);
watch(dialogRef, syncDialog);

const draft = ref<Draft>(draftFromSettings());
const savedSnapshot = ref("");
const editingChannelId = ref("");
const saving = ref(false);
const error = ref("");
const draftSnapshot = computed(() => JSON.stringify(draft.value));
const dirty = computed(() => !!savedSnapshot.value && draftSnapshot.value !== savedSnapshot.value);
const validationError = computed(() => {
  const common = [draft.value.patienceSeconds, draft.value.muteDurationSeconds];
  if (common.some((value) => !Number.isFinite(Number(value)))) return t("config.remoteIm.behaviorFiniteNumberError");
  const p = draft.value.pacing;
  const group = [p.assistantDebounceSeconds, p.secretaryInspectionSeconds, p.replyCooldownSeconds, p.inspectionJitterRatio, p.maximumEnergy, p.baseReplyEnergyCost, p.energyCostPerCharacter, p.energyRecoveryPerSecond, p.positiveEnergyDelta, p.negativeEnergyDelta, p.normalReplyMaxChars, p.focusReplyMaxChars];
  if (group.some((value) => !Number.isFinite(Number(value)))) return t("config.remoteIm.behaviorFiniteNumberError");
  if (p.assistantDebounceSeconds < 1 || p.secretaryInspectionSeconds < 1) return t("config.remoteIm.behaviorPeriodError");
  if (p.inspectionJitterRatio < 0 || p.inspectionJitterRatio > 1) return t("config.remoteIm.behaviorJitterError");
  if (p.maximumEnergy <= 0 || p.baseReplyEnergyCost < 0 || p.energyCostPerCharacter < 0 || p.energyRecoveryPerSecond < 0 || p.positiveEnergyDelta < 0 || p.negativeEnergyDelta > 0) return t("config.remoteIm.behaviorEnergyError");
  if (p.normalReplyMaxChars < 1 || p.focusReplyMaxChars < p.normalReplyMaxChars) return t("config.remoteIm.behaviorLengthError");
  return "";
});

function openModal() {
  if (!props.channel) return;
  draft.value = draftFromSettings(props.channel.behaviorSettings);
  savedSnapshot.value = JSON.stringify(draft.value);
  editingChannelId.value = props.channel.id;
  error.value = "";
  open.value = true;
}

function closeModal() {
  if (!saving.value) open.value = false;
}

async function restoreDefaults() {
  const next = draftFromSettings(DEFAULT_REMOTE_IM_CHANNEL_BEHAVIOR_SETTINGS);
  try {
    next.responseGuidance = await invokeTauri<string>("remote_im_get_default_group_response_guidance");
  } catch (restoreError) {
    console.warn("[远程IM] 渠道默认应答规则读取失败，保留其他默认值:", restoreError);
  }
  draft.value = next;
  error.value = "";
}

function restoreSaved() {
  if (!savedSnapshot.value) return;
  try {
    draft.value = JSON.parse(savedSnapshot.value) as Draft;
    error.value = "";
  } catch {
    draft.value = draftFromSettings();
    error.value = "";
  }
}

async function save() {
  if (saving.value || !dirty.value || validationError.value) return;
  const channel = props.channel;
  if (!channel || channel.id !== editingChannelId.value) {
    error.value = t("config.remoteIm.channelBehaviorChannelChanged");
    return;
  }
  saving.value = true;
  error.value = "";
  const submittedSnapshot = draftSnapshot.value;
  const previous = channel.behaviorSettings
    ? cloneChannelBehaviorSettings(channel.behaviorSettings)
    : undefined;
  const next = settingsFromDraft(draft.value);
  channel.behaviorSettings = next;
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      channel.behaviorSettings = previous;
      error.value = t("config.remoteIm.channelBehaviorSaveFailed");
      return;
    }
    savedSnapshot.value = JSON.stringify(draftFromSettings(next));
    if (draftSnapshot.value === submittedSnapshot) draft.value = draftFromSettings(next);
    props.setStatusAction(t("config.remoteIm.channelBehaviorSaved"));
    try {
      await invokeTauri("remote_im_reconfigure_channel_behavior", { channelId: channel.id });
    } catch (reconfigureError) {
      console.warn("[远程IM] channel behavior reconfigure deferred:", reconfigureError);
      props.setStatusAction(t("config.remoteIm.channelBehaviorSavedReconfigureDeferred"));
    }
  } catch (saveError) {
    channel.behaviorSettings = previous;
    error.value = String(saveError);
  } finally {
    saving.value = false;
  }
}
</script>
