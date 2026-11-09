import { ref } from "vue";
import { describe, expect, it } from "vitest";
import { useChatFlowSendInput } from "./use-chat-flow-send-input";
import { useChatFlowSendPayloads } from "./use-chat-flow-send-payloads";

describe("useChatFlowSendInput", () => {
  it("keeps image attachment metadata single when the same file is also an image payload", () => {
    const queuedAttachmentNotices = ref([
      {
        id: "C:/workspace/downloads/source.png::image/png",
        fileName: "source.png",
        path: "C:/workspace/downloads/source.png",
        mime: "image/png",
      },
    ]);
    const clipboardImages = ref([
      {
        mime: "image/png",
        bytesBase64: "aW1n",
        savedPath: "C:/workspace/downloads/source.png",
      },
    ]);
    const payloads = useChatFlowSendPayloads({ queuedAttachmentNotices });
    const sendInput = useChatFlowSendInput({
      chatInput: ref("看图"),
      clipboardImages,
      queuedAttachmentNotices,
      selectedMentions: ref([]),
      latestUserText: ref(""),
      latestUserImages: ref([]),
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      buildQueuedAttachmentPayload: payloads.buildQueuedAttachmentPayload,
      buildImageAttachmentPayload: payloads.buildImageAttachmentPayload,
      mergeAttachmentPayloads: payloads.mergeAttachmentPayloads,
    });

    const prepared = sendInput.prepareSendInput();

    expect(prepared?.attachments).toEqual([
      {
        fileName: "source.png",
        path: "C:/workspace/downloads/source.png",
        mime: "image/png",
      },
    ]);
    expect(prepared?.sentImages).toHaveLength(1);
  });

  it("adds an attachment metadata fallback for image payloads that only have savedPath", () => {    const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; path: string; mime: string }>>([]);
    const clipboardImages = ref([
      {
        mime: "image/png",
        bytesBase64: "aW1n",
        savedPath: "C:/workspace/downloads/pasted.png",
      },
    ]);
    const payloads = useChatFlowSendPayloads({ queuedAttachmentNotices });
    const sendInput = useChatFlowSendInput({
      chatInput: ref(""),
      clipboardImages,
      queuedAttachmentNotices,
      selectedMentions: ref([]),
      latestUserText: ref(""),
      latestUserImages: ref([]),
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      buildQueuedAttachmentPayload: payloads.buildQueuedAttachmentPayload,
      buildImageAttachmentPayload: payloads.buildImageAttachmentPayload,
      mergeAttachmentPayloads: payloads.mergeAttachmentPayloads,
    });

    const prepared = sendInput.prepareSendInput();

    expect(prepared?.attachments).toEqual([
      {
        fileName: "pasted.png",
        path: "C:/workspace/downloads/pasted.png",
        mime: "image/png",
      },
    ]);
    expect(prepared?.sentImages).toHaveLength(1);
  });

  // 交互答题一类的答复是用 overrides.text 发出去的：这类发送绝不能动输入框，
  // 否则用户正在打的那句话会被一条系统代发的答复清掉。
  it("override 发送不清空输入框、不带走待发附件与已选提及", () => {
    const queuedAttachmentNotices = ref([
      {
        id: "C:/workspace/downloads/note.md::text/markdown",
        fileName: "note.md",
        path: "C:/workspace/downloads/note.md",
        mime: "text/markdown",
      },
    ]);
    const clipboardImages = ref([
      { mime: "image/png", bytesBase64: "aW1n", savedPath: "C:/workspace/downloads/shot.png" },
    ]);
    const chatInput = ref("我还没打完的半句话");
    const selectedMentions = ref([{ agentId: "agent-2", agentName: "另一个" }]);
    const payloads = useChatFlowSendPayloads({ queuedAttachmentNotices });
    const sendInput = useChatFlowSendInput({
      chatInput,
      clipboardImages,
      queuedAttachmentNotices,
      selectedMentions,
      latestUserText: ref(""),
      latestUserImages: ref([]),
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      buildQueuedAttachmentPayload: payloads.buildQueuedAttachmentPayload,
      buildImageAttachmentPayload: payloads.buildImageAttachmentPayload,
      mergeAttachmentPayloads: payloads.mergeAttachmentPayloads,
    });

    const prepared = sendInput.prepareSendInput({ text: "【题一】\n→ A1" });
    expect(prepared).not.toBeNull();
    expect(prepared?.useOverrideMessage).toBe(true);
    expect(prepared?.plainText).toBe("【题一】\n→ A1");
    expect(prepared?.attachments).toEqual([]);
    expect(prepared?.sentImages).toEqual([]);

    sendInput.applyPreparedSendInput(prepared!);

    expect(chatInput.value).toBe("我还没打完的半句话");
    expect(queuedAttachmentNotices.value).toHaveLength(1);
    expect(clipboardImages.value).toHaveLength(1);
    expect(selectedMentions.value).toHaveLength(1);
  });
});
