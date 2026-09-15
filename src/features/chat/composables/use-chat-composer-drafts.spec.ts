import { nextTick, ref } from "vue";
import { describe, expect, it } from "vitest";
import { useChatComposerDrafts } from "./use-chat-composer-drafts";

function createComposable() {
  const activeConversationId = ref("conv-a");
  const chatInput = ref("");
  const selectedMentions = ref([] as Array<{ agentId: string; agentName: string; avatarUrl?: string }>);
  const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
  const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; path: string; mime: string }>>([]);

  useChatComposerDrafts({
    activeConversationId,
    chatInput,
    selectedMentions,
    clipboardImages,
    queuedAttachmentNotices,
  });

  return {
    activeConversationId,
    chatInput,
    selectedMentions,
    clipboardImages,
    queuedAttachmentNotices,
  };
}

describe("useChatComposerDrafts", () => {
  it("keeps savedPath-only image drafts when switching conversations", async () => {
    const state = createComposable();
    await nextTick();

    state.clipboardImages.value = [
      {
        mime: "image/png",
        bytesBase64: "",
        savedPath: "C:/workspace/downloads/pasted.png",
      },
    ];
    await nextTick();

    state.activeConversationId.value = "conv-b";
    await nextTick();
    state.activeConversationId.value = "conv-a";
    await nextTick();

    expect(state.clipboardImages.value).toHaveLength(1);
    expect(state.clipboardImages.value[0]).toMatchObject({
      mime: "image/png",
      bytesBase64: "",
      savedPath: "C:/workspace/downloads/pasted.png",
    });
  });
});
