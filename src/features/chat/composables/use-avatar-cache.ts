import { ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { PersonaProfile } from "../../../types/app";

type UseAvatarCacheOptions = {
  personas: Ref<PersonaProfile[]>;
};

export function useAvatarCache(options: UseAvatarCacheOptions) {
  const avatarDataUrlCache = ref<Record<string, string>>({});

  function avatarCacheKey(path?: string, updatedAt?: string): string {
    if (!path) return "";
    return `${path}|${updatedAt || ""}`;
  }

  function resolveAvatarUrl(path?: string, updatedAt?: string): string {
    const key = avatarCacheKey(path, updatedAt);
    if (!key) return "";
    return avatarDataUrlCache.value[key] || "";
  }

  async function ensureAvatarCached(path?: string, updatedAt?: string) {
    const key = avatarCacheKey(path, updatedAt);
    if (!key || avatarDataUrlCache.value[key]) return;
    try {
      const result = await invokeTauri<{ dataUrl: string }>("read_avatar_data_url", {
        input: { path },
      });
      avatarDataUrlCache.value = {
        ...avatarDataUrlCache.value,
        [key]: result.dataUrl || "",
      };
    } catch {
      // ignore avatar load failures, fallback to initial avatar.
    }
  }

  async function preloadPersonaAvatars() {
    const tasks: Promise<void>[] = [];
    for (const p of options.personas.value) {
      if (!p.avatarPath) continue;
      tasks.push(ensureAvatarCached(p.avatarPath, p.avatarUpdatedAt));
    }
    await Promise.all(tasks);
  }

  return {
    resolveAvatarUrl,
    ensureAvatarCached,
    preloadPersonaAvatars,
  };
}

