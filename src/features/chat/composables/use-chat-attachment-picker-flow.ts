import type { Ref } from "vue";
import type { AttachmentReceipt } from "../../../services/attachment-transfer";
import {
  ingestTransportAttachmentSource,
  pickTransportAttachmentSources,
} from "../../../services/tauri-api";

type QueuedAttachmentNotice = {
  id: string;
  fileName: string;
  path: string;
  mime: string;
  /** 上传中占位：文件已选但尚未读取完成。 */
  pending?: boolean;
};

type UseChatAttachmentPickerFlowOptions = {
  chatting: Ref<boolean>;
  trimming: Ref<boolean>;
  queuedAttachmentNotices: Ref<QueuedAttachmentNotice[]>;
  applyPickedAttachment: (receipt: AttachmentReceipt) => void;
  setStatusError: (key: string, error: unknown) => void;
};

export function useChatAttachmentPickerFlow(options: UseChatAttachmentPickerFlowOptions) {
  function removeQueuedAttachmentNotice(index: number) {
    if (index < 0 || index >= options.queuedAttachmentNotices.value.length) return;
    options.queuedAttachmentNotices.value.splice(index, 1);
  }

  function removePendingPlaceholder(sourceId: string) {
    const list = options.queuedAttachmentNotices.value;
    const index = list.findIndex((item) => item.id === sourceId && item.pending);
    if (index >= 0) list.splice(index, 1);
  }

  async function pickChatAttachments(kind?: "image" | "file") {
    // Product rule: 流式回复进行中不锁定输入工具栏（与粘贴/拖拽图片一致），
    // 仅压缩整理（trimming）期间禁止新增附件。
    if (options.trimming.value) return;
    try {
      const sources = await pickTransportAttachmentSources({
        multiple: true,
        directory: false,
        title: kind === "image" ? "选择图片" : "选择附件",
        filters:
          kind === "image"
            ? [{ name: "图片", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"] }]
            : undefined,
      });
      if (sources.length === 0) return;
      // 先占位，再逐个读取替换，避免大文件读取期间界面无反馈。
      for (const source of sources) {
        options.queuedAttachmentNotices.value.push({
          id: source.id,
          fileName: source.fileName,
          path: "",
          mime: "",
          pending: true,
        });
      }
      for (const source of sources) {
        try {
          const receipt = await ingestTransportAttachmentSource(source);
          removePendingPlaceholder(source.id);
          options.applyPickedAttachment(receipt);
        } catch (error) {
          removePendingPlaceholder(source.id);
          options.setStatusError("status.pasteImageReadFailed", error);
        }
      }
    } catch (error) {
      options.setStatusError("status.pasteImageReadFailed", error);
    }
  }

  return {
    removeQueuedAttachmentNotice,
    pickChatAttachments,
  };
}
