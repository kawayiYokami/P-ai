import { ref } from "vue";

type BinaryAttachment = { mime: string; bytesBase64?: string; dataUrl?: string };

export function useChatImagePreview() {
  const imagePreviewOpen = ref(false);
  const imagePreviewDataUrl = ref("");
  const imagePreviewZoom = ref(1);
  const previewOffsetX = ref(0);
  const previewOffsetY = ref(0);
  const previewDragging = ref(false);

  const IMAGE_PREVIEW_MIN_ZOOM = 0.2;
  const IMAGE_PREVIEW_MAX_ZOOM = 5;
  const IMAGE_PREVIEW_ZOOM_STEP = 0.1;

  let previewPointerId: number | null = null;
  let previewDragStartX = 0;
  let previewDragStartY = 0;
  let previewDragOriginOffsetX = 0;
  let previewDragOriginOffsetY = 0;

  function clampPreviewZoom(value: number): number {
    return Math.min(IMAGE_PREVIEW_MAX_ZOOM, Math.max(IMAGE_PREVIEW_MIN_ZOOM, value));
  }

  function zoomInPreview() {
    imagePreviewZoom.value = clampPreviewZoom(imagePreviewZoom.value + IMAGE_PREVIEW_ZOOM_STEP);
  }

  function zoomOutPreview() {
    imagePreviewZoom.value = clampPreviewZoom(imagePreviewZoom.value - IMAGE_PREVIEW_ZOOM_STEP);
    if (imagePreviewZoom.value <= 1) {
      previewOffsetX.value = 0;
      previewOffsetY.value = 0;
    }
  }

  function resetPreviewZoom() {
    imagePreviewZoom.value = 1;
    previewOffsetX.value = 0;
    previewOffsetY.value = 0;
  }

  function onPreviewWheel(event: WheelEvent) {
    if (event.deltaY < 0) {
      zoomInPreview();
    } else if (event.deltaY > 0) {
      zoomOutPreview();
    }
  }

  function openImagePreview(image?: BinaryAttachment | null) {
    if (!image) return;
    const mime = String(image.mime || "").trim() || "image/webp";
    const dataUrl = String(image.dataUrl || "").trim();
    const bytes = String(image.bytesBase64 || "").trim();
    if (dataUrl) {
      imagePreviewDataUrl.value = dataUrl;
    } else {
      if (!bytes) return;
      imagePreviewDataUrl.value = `data:${mime};base64,${bytes}`;
    }
    imagePreviewZoom.value = 1;
    previewOffsetX.value = 0;
    previewOffsetY.value = 0;
    previewDragging.value = false;
    previewPointerId = null;
    imagePreviewOpen.value = true;
  }

  function closeImagePreview() {
    imagePreviewOpen.value = false;
    imagePreviewDataUrl.value = "";
    imagePreviewZoom.value = 1;
    previewOffsetX.value = 0;
    previewOffsetY.value = 0;
    previewDragging.value = false;
    previewPointerId = null;
  }

  function onPreviewPointerDown(event: PointerEvent) {
    if (imagePreviewZoom.value <= 1) return;
    previewDragging.value = true;
    previewPointerId = event.pointerId;
    previewDragStartX = event.clientX;
    previewDragStartY = event.clientY;
    previewDragOriginOffsetX = previewOffsetX.value;
    previewDragOriginOffsetY = previewOffsetY.value;
    (event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId);
  }

  function onPreviewPointerMove(event: PointerEvent) {
    if (!previewDragging.value || previewPointerId !== event.pointerId) return;
    const deltaX = event.clientX - previewDragStartX;
    const deltaY = event.clientY - previewDragStartY;
    previewOffsetX.value = previewDragOriginOffsetX + deltaX;
    previewOffsetY.value = previewDragOriginOffsetY + deltaY;
  }

  function onPreviewPointerUp(event: PointerEvent) {
    if (previewPointerId !== null && previewPointerId !== event.pointerId) return;
    previewDragging.value = false;
    previewPointerId = null;
  }

  return {
    imagePreviewOpen,
    imagePreviewDataUrl,
    imagePreviewZoom,
    IMAGE_PREVIEW_MIN_ZOOM,
    IMAGE_PREVIEW_MAX_ZOOM,
    previewOffsetX,
    previewOffsetY,
    previewDragging,
    zoomInPreview,
    zoomOutPreview,
    resetPreviewZoom,
    onPreviewWheel,
    openImagePreview,
    closeImagePreview,
    onPreviewPointerDown,
    onPreviewPointerMove,
    onPreviewPointerUp,
  };
}
