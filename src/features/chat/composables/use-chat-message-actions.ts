import { ref } from "vue";
import type { ChatMessageBlock } from "../../../types/app";
import { stripToolcallMarkers } from "../../../utils/chat-message-semantics";
import { readTransportLocalBinaryFile, resolveLocalFileUrl, writeTransportClipboardText } from "../../../services/tauri-api";

export function useChatMessageActions() {
  const playingAudioId = ref("");
  let activeAudio: HTMLAudioElement | null = null;

  async function copyMessage(block: ChatMessageBlock): Promise<boolean> {
    // 纯附件消息 text 为空，回退到附件文件名，与批量复制的 attachmentList 同口径，避免点复制弹失败。
    const copyText = stripToolcallMarkers(block.text || "").trim()
      || (block.attachmentFiles || []).map((item) => String(item.fileName || "").trim()).filter(Boolean).join("、");
    if (!copyText) return false;
    try {
      await writeTransportClipboardText(copyText);
      return true;
    } catch {
      // Ignore clipboard failures to avoid interrupting chat flow.
      return false;
    }
  }

  async function buildAudioSource(audio: { mime: string; bytesBase64?: string; mediaRef?: string }): Promise<string> {
    const bytesBase64 = String(audio.bytesBase64 || "").trim();
    if (bytesBase64) return `data:${audio.mime};base64,${bytesBase64}`;
    const mediaRef = String(audio.mediaRef || "").trim();
    if (!mediaRef || mediaRef.startsWith("@media:") || mediaRef.startsWith("@download:")) return "";
    try {
      const result = await readTransportLocalBinaryFile<{ mime?: string; bytesBase64?: string }>({ path: mediaRef });
      const encoded = String(result?.bytesBase64 || "").trim();
      const mime = String(result?.mime || audio.mime || "audio/mpeg").trim();
      if (encoded) return `data:${mime};base64,${encoded}`;
    } catch (error) {
      const fallback = resolveLocalFileUrl(mediaRef);
      if (fallback) return fallback;
      console.warn("[聊天音频] 读取失败", { path: mediaRef, error });
      return "";
    }
    return resolveLocalFileUrl(mediaRef);
  }

  function stopAudioPlayback() {
    if (activeAudio) {
      activeAudio.pause();
      activeAudio.currentTime = 0;
      activeAudio = null;
    }
    playingAudioId.value = "";
  }

  async function toggleAudioPlayback(id: string, audio: { mime: string; bytesBase64?: string; mediaRef?: string }) {
    if (playingAudioId.value === id && activeAudio) {
      stopAudioPlayback();
      return;
    }
    stopAudioPlayback();
    const source = await buildAudioSource(audio);
    if (!source) return;
    const player = new Audio(source);
    activeAudio = player;
    playingAudioId.value = id;
    player.onended = () => {
      if (activeAudio === player) {
        activeAudio = null;
        playingAudioId.value = "";
      }
    };
    void player.play().catch(() => {
      if (activeAudio === player) {
        activeAudio = null;
        playingAudioId.value = "";
      }
    });
  }

  return {
    playingAudioId,
    copyMessage,
    stopAudioPlayback,
    toggleAudioPlayback,
  };
}
