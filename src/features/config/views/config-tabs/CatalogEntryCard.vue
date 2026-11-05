<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ComponentPublicInstance } from "vue";
import { ArrowDown } from "@lucide/vue";
import type { CatalogEntry } from "../../../../types/app";

/**
 * 能力商店的一张条目卡：图标 + 名称 + 来源 + 两行说明 + 一个动作。
 * 只负责呈现，装没装、能不能装、点下去做什么都由调用方决定。
 */
const props = defineProps<{
  entry: CatalogEntry;
  /** 已解析出的图标地址（data URL）；空串表示还没取到或取不到。 */
  iconUrl: string;
  installBusy?: boolean;
  toggleBusy?: boolean;
  /** 图标元素的函数 ref，由调用方交给 IntersectionObserver。 */
  iconRef?: (el: Element | ComponentPublicInstance | null) => void;
}>();

const emit = defineEmits<{
  (event: "open"): void;
  (event: "install"): void;
  (event: "toggle"): void;
  (event: "iconError"): void;
}>();

const { t } = useI18n();

function iconInitial(entry: CatalogEntry): string {
  const source = String(entry.name || entry.id || "").trim();
  return source ? source.slice(0, 1).toUpperCase() : "?";
}

/** 下载量：一万以上折成 K，百万以上折成 M，与来源站自己的写法一致。 */
function formatInstalls(value: number): string {
  if (!value || value <= 0) return "";
  if (value >= 1_000_000) {
    const millions = value / 1_000_000;
    return `${Number.isInteger(millions) ? millions : millions.toFixed(1)}M`;
  }
  if (value >= 1_000) return `${Math.round(value / 1_000)}K`;
  return value.toLocaleString();
}
</script>

<template>
  <div
    class="group flex cursor-pointer flex-col gap-3 rounded-box border border-base-300 bg-base-100 p-4 transition-colors hover:border-base-content/25"
    @click="emit('open')"
  >
    <div class="flex items-center gap-3">
      <span
        :ref="props.iconRef"
        :data-icon="props.entry.icon"
        class="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-base-200 text-sm font-semibold text-base-content/50"
      >
        <img
          v-if="props.iconUrl"
          :src="props.iconUrl"
          alt=""
          class="h-full w-full object-cover"
          @error="emit('iconError')"
        />
        <span v-else>{{ iconInitial(props.entry) }}</span>
      </span>
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-semibold text-base-content" :title="props.entry.name">
          {{ props.entry.name }}
        </div>
        <div
          v-if="props.entry.author"
          class="mt-0.5 truncate text-caption text-base-content/50"
          :title="props.entry.author"
        >
          {{ props.entry.author }}
        </div>
      </div>
    </div>

    <p
      class="line-clamp-2 min-h-[2.5rem] text-xs leading-relaxed"
      :class="props.entry.description ? 'text-base-content/70' : 'text-base-content/40 italic'"
    >
      {{ props.entry.description || t("config.catalog.noDescription") }}
    </p>

    <div class="mt-auto flex items-center justify-between gap-2" @click.stop>
      <span v-if="formatInstalls(props.entry.popularity)" class="flex items-center gap-1 text-caption text-base-content/50">
        <ArrowDown class="h-3 w-3" />
        <span>{{ formatInstalls(props.entry.popularity) }}</span>
      </span>
      <span v-else></span>
      <div class="flex items-center gap-2">
        <template v-if="props.entry.installed">
          <span class="text-caption opacity-60">
            {{ props.entry.enabled ? t("config.catalog.enabled") : t("config.catalog.disabled") }}
          </span>
          <input
            type="checkbox"
            class="toggle toggle-xs toggle-primary"
            :checked="props.entry.enabled"
            :disabled="props.toggleBusy"
            @change="emit('toggle')"
          />
        </template>
        <button
          v-else
          class="btn btn-sm h-8 min-h-[2rem] border-base-content/25 bg-transparent px-3 text-caption font-normal text-base-content/80 hover:border-base-content/40 hover:bg-base-200"
          :class="props.entry.installReady ? '' : 'opacity-50'"
          type="button"
          :disabled="!props.entry.installReady || props.installBusy"
          :title="props.entry.installReady ? '' : t('config.catalog.notInstallable')"
          @click="emit('install')"
        >
          <span v-if="props.installBusy" class="loading loading-spinner loading-xs"></span>
          <span>{{ t("config.catalog.install") }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
