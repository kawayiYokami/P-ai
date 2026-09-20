<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose">
    <div class="modal-box p-3 max-w-md">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.cropAvatar") }}</h3>
      <!-- 画布高度必须显式给：cropper-canvas 自带 min-height:100px，不给高度就会塌成一条横条 -->
      <div
        class="relative mx-auto aspect-square w-full overflow-hidden rounded-box border border-base-300 bg-base-200"
      >
        <img ref="imageRef" :src="source" alt="" class="block" />
      </div>
      <div class="mt-3 flex items-center gap-2">
        <span class="text-xs opacity-60">{{ t("config.persona.cropZoom") }}</span>
        <input
          v-model.number="zoomPercent"
          type="range"
          min="100"
          :max="MAX_ZOOM * 100"
          :step="SLIDER_STEP"
          class="range range-xs flex-1"
          :aria-label="t('config.persona.cropZoom')"
          @input="onZoomInput"
        />
        <span class="w-10 text-right text-xs tabular-nums opacity-70">{{ zoomPercent }}%</span>
        <button class="btn btn-xs btn-ghost" @click="resetView">{{ t("config.persona.cropReset") }}</button>
      </div>
      <div class="mt-3 flex items-center gap-3">
        <!-- 预览就是最终头像的样子：和资料页一样是圆形，按真实显示尺寸 56px 展示 -->
        <div class="h-14 w-14 shrink-0 overflow-hidden rounded-full bg-base-200">
          <canvas
            ref="previewRef"
            :width="PREVIEW_SIZE"
            :height="PREVIEW_SIZE"
            class="block h-full w-full"
          ></canvas>
        </div>
        <div class="flex-1 text-xs opacity-60">{{ t("config.persona.cropHint") }}</div>
      </div>
      <div v-if="error || localError" class="mt-2 text-sm text-error break-all">{{ error || localError }}</div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeDialog">{{ t("common.cancel") }}</button>
        <button class="btn btn-sm btn-primary" :disabled="!ready || saving" @click="confirmCrop">
          {{ saving ? t("config.api.saving") : t("config.persona.saveAvatar") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Cropper from "cropperjs";

const props = defineProps<{
  open: boolean;
  source: string;
  saving: boolean;
  error: string;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "confirm", value: { mime: string; bytesBase64: string }): void;
}>();

const { t } = useI18n();

// 结构与官方默认模板一致（docs/guide.md 的默认模板），只改四处：
// 图片 initial-center-size="cover" 铺满画布、选择框 aspect-ratio="1" 取方形、给一个初始占比、
// 画布尺寸写进模板。画布尺寸必须在这里给：库按选择框父元素（就是画布）的 offsetHeight 算初始
// 取景框，而 cropper-canvas 自带 min-height:100px，构造完再补样式就晚了——初始取景框会按 100px
// 算成 80×80（0.8×100）。写进模板后属性回调触发时画布已是最终尺寸。
// 取景范围就是选择框，导出走 selection.$toCanvas，缩放走库自带的滚轮与 image.$scale。
const CROP_TEMPLATE = [
  '<cropper-canvas style="width:100%;height:100%;min-width:0;min-height:0">',
  '<cropper-image initial-center-size="cover" translatable scalable></cropper-image>',
  "<cropper-shade></cropper-shade>",
  '<cropper-handle action="move" plain></cropper-handle>',
  '<cropper-selection initial-coverage="0.8" aspect-ratio="1" movable resizable>',
  '<cropper-grid role="grid" bordered covered></cropper-grid>',
  "<cropper-crosshair centered></cropper-crosshair>",
  '<cropper-handle action="move" theme-color="rgba(255, 255, 255, 0.35)"></cropper-handle>',
  '<cropper-handle action="n-resize"></cropper-handle>',
  '<cropper-handle action="e-resize"></cropper-handle>',
  '<cropper-handle action="s-resize"></cropper-handle>',
  '<cropper-handle action="w-resize"></cropper-handle>',
  '<cropper-handle action="ne-resize"></cropper-handle>',
  '<cropper-handle action="nw-resize"></cropper-handle>',
  '<cropper-handle action="se-resize"></cropper-handle>',
  '<cropper-handle action="sw-resize"></cropper-handle>',
  "</cropper-selection>",
  "</cropper-canvas>",
].join("");
const SLIDER_STEP = 5;
const MIN_ZOOM = 1;
const MAX_ZOOM = 4;
const OUTPUT_SIZE = 128;
// 预览与导出同尺寸：小圆框里按 56px 显示，高分屏下也够清晰
const PREVIEW_SIZE = OUTPUT_SIZE;

type CropperImageElement = NonNullable<ReturnType<Cropper["getCropperImage"]>>;
type CropperSelectionElement = NonNullable<ReturnType<Cropper["getCropperSelection"]>>;
type Rect = { x: number; y: number; width: number; height: number };

const dialogRef = ref<HTMLDialogElement | null>(null);
const imageRef = ref<HTMLImageElement | null>(null);
const previewRef = ref<HTMLCanvasElement | null>(null);
const ready = ref(false);
const localError = ref("");
const zoomPercent = ref(100);

let cropper: Cropper | null = null;
let previewFrame = 0;

function destroyCropper() {
  const image = cropper?.getCropperImage();
  const selection = cropper?.getCropperSelection();
  image?.removeEventListener("transform", onImageTransform);
  selection?.removeEventListener("change", onSelectionChange);
  if (previewFrame) {
    cancelAnimationFrame(previewFrame);
    previewFrame = 0;
  }
  cropper?.destroy();
  cropper = null;
  ready.value = false;
  zoomPercent.value = 100;
}

/**
 * 预览：直接调取景框的 $toCanvas（和保存走同一条路），画进小圆框。
 * 拖拽与滚轮会连续触发事件，所以合并到下一帧再画，避免一帧画多次。
 * 事件都是「先派发、后生效」，必须等下一帧才能取到已生效的矩阵与取景框。
 */
function schedulePreview() {
  if (previewFrame) return;
  previewFrame = requestAnimationFrame(() => {
    previewFrame = 0;
    void renderPreview();
  });
}

async function renderPreview() {
  const selection = cropper?.getCropperSelection();
  const target = previewRef.value;
  if (!selection || !target || !ready.value) return;
  try {
    const output = await selection.$toCanvas({ width: PREVIEW_SIZE, height: PREVIEW_SIZE });
    // 等待期间可能已经换图或关闭，本次结果作废
    if (cropper?.getCropperSelection() !== selection) return;
    const context = target.getContext("2d");
    if (!context) return;
    context.clearRect(0, 0, target.width, target.height);
    context.drawImage(output, 0, 0, target.width, target.height);
  } catch (error) {
    // 预览失败不影响裁剪本身，只在控制台留痕
    console.warn("[配置][头像裁剪] 预览渲染失败", error);
  }
}

/** 图片铺满画布所需的倍率，与库 $center('cover') 同一算法，只用于把矩阵换算成百分比 */
function coverScale(): number {
  const image = cropper?.getCropperImage();
  const canvas = cropper?.getCropperCanvas();
  if (!image || !canvas) return 0;
  const { naturalWidth, naturalHeight } = image.$image;
  const width = canvas.offsetWidth;
  const height = canvas.offsetHeight;
  if (!naturalWidth || !naturalHeight || !width || !height) return 0;
  return Math.max(width / naturalWidth, height / naturalHeight);
}

function syncZoom(scale: number) {
  const base = coverScale();
  if (!base || !scale) return;
  const percent = Math.round((scale / base) * 100 / SLIDER_STEP) * SLIDER_STEP;
  zoomPercent.value = Math.min(MAX_ZOOM * 100, Math.max(MIN_ZOOM * 100, percent));
}

function setZoomPercent(percent: number) {
  const image = cropper?.getCropperImage();
  const base = coverScale();
  if (!image || !base) return;
  const snapped = Math.min(MAX_ZOOM * 100, Math.max(MIN_ZOOM * 100, Math.round(percent / SLIDER_STEP) * SLIDER_STEP));
  const target = base * (snapped / 100);
  const current = image.$getTransform()[0] || base;
  if (Math.abs(target - current) > 1e-4) {
    // 缩放本身会触发 transform 事件，百分比与边界判定都交给事件处理，避免滑块与画面脱节
    image.$scale(target / current);
    return;
  }
  zoomPercent.value = snapped;
}

function onZoomInput() {
  setZoomPercent(zoomPercent.value);
}

/** 回到初始取景：图片重新 cover 居中，选择框回到初始位置与大小 */
function resetView() {
  const image = cropper?.getCropperImage();
  if (!image) return;
  image.$resetTransform();
  image.$center("cover");
  cropper?.getCropperSelection()?.$reset();
}

function closeDialog() {
  dialogRef.value?.close();
}

function onDialogClose() {
  emit("update:open", false);
}

function inSelection(selection: Rect, max: Rect, tolerance = 0): boolean {
  return (
    selection.x >= max.x - tolerance &&
    selection.y >= max.y - tolerance &&
    selection.x + selection.width <= max.x + max.width + tolerance &&
    selection.y + selection.height <= max.y + max.height + tolerance
  );
}

function rectInCanvas(canvas: HTMLElement, element: HTMLElement): Rect {
  const canvasRect = canvas.getBoundingClientRect();
  const elementRect = element.getBoundingClientRect();
  return {
    x: elementRect.left - canvasRect.left,
    y: elementRect.top - canvasRect.top,
    width: elementRect.width,
    height: elementRect.height,
  };
}

/** 画布自身的边界，就是官方示例里 within: canvas 那组的 maxSelection */
function canvasRect(canvas: HTMLElement): Rect {
  const rect = canvas.getBoundingClientRect();
  return { x: 0, y: 0, width: rect.width, height: rect.height };
}

/**
 * 图片边界限制，照抄官方「Limit boundaries」示例（docs/api/cropper-selection.md）：
 * 复制一个图片节点套用待生效的矩阵，量出它的边界，越界就拦掉这次变换。
 * 这里判的是「图片必须盖住画布」——画布 overflow:hidden，图片一旦小于画布就会露出底色，
 * 滑块的百分比也会跟着失准。0.5px 容差是为了让库自己算出的那次 cover 变换能通过。
 */
function onImageTransform(event: Event) {
  const canvas = cropper?.getCropperCanvas();
  const image = cropper?.getCropperImage();
  if (!canvas || !image) return;
  const matrix = (event as CustomEvent<{ matrix: number[] }>).detail.matrix;
  const clone = image.cloneNode() as CropperImageElement;
  clone.style.transform = `matrix(${matrix.join(", ")})`;
  clone.style.opacity = "0";
  canvas.appendChild(clone);
  const cloneRect = rectInCanvas(canvas, clone);
  canvas.removeChild(clone);
  const accepted = inSelection(canvasRect(canvas), cloneRect, 0.5);
  if (!accepted) {
    event.preventDefault();
  }
  // 库自己居中、滚轮与捏合缩放都会走这里，所以滑块百分比始终跟着真实矩阵
  syncZoom(accepted ? matrix[0] : image.$getTransform()[0]);
  schedulePreview();
}

/** 取景框边界限制，照抄官方同一示例的 within: canvas 分支：取景框不许越出画布 */
function onSelectionChange(event: Event) {
  const canvas = cropper?.getCropperCanvas();
  if (!canvas) return;
  if (!inSelection((event as CustomEvent<Rect>).detail, canvasRect(canvas))) {
    event.preventDefault();
  }
  schedulePreview();
}

async function initCropper() {
  destroyCropper();
  if (!imageRef.value) {
    localError.value = t("config.persona.cropInitFailed");
    return;
  }
  localError.value = "";
  // 先开弹窗：画布要有尺寸，库才会按 initial-coverage 把选择框摆到正确的大小与位置
  if (!dialogRef.value?.open) {
    dialogRef.value?.showModal();
  }
  let instance: Cropper;
  try {
    instance = new Cropper(imageRef.value, { template: CROP_TEMPLATE });
  } catch (error) {
    console.warn("[配置][头像裁剪] 初始化失败", error);
    localError.value = t("config.persona.cropInitFailed");
    return;
  }
  cropper = instance;
  const image = instance.getCropperImage();
  const selection = instance.getCropperSelection();
  if (!image || !selection) {
    localError.value = t("config.persona.cropInitFailed");
    return;
  }
  image.addEventListener("transform", onImageTransform);
  selection.addEventListener("change", onSelectionChange);
  try {
    await image.$ready();
  } catch (error) {
    // 等待期间可能已经重建或销毁，本次结果作废
    if (cropper !== instance) return;
    console.warn("[配置][头像裁剪] 图片加载失败", error);
    localError.value = t("config.persona.avatarReadFailed", {
      err: error instanceof Error ? error.message : String(error),
    });
    return;
  }
  // 同上：换图会让本轮初始化作废
  if (cropper !== instance) return;
  ready.value = true;
  schedulePreview();
}

async function confirmCrop() {
  const selection = cropper?.getCropperSelection();
  if (!selection) {
    localError.value = t("config.persona.cropperNotReady");
    return;
  }
  localError.value = "";
  try {
    const output = await selection.$toCanvas({
      width: OUTPUT_SIZE,
      height: OUTPUT_SIZE,
      beforeDraw(context: CanvasRenderingContext2D) {
        context.imageSmoothingEnabled = true;
        context.imageSmoothingQuality = "high";
      },
    });
    const dataUrl = output.toDataURL("image/webp", 0.8);
    const marker = "base64,";
    const idx = dataUrl.indexOf(marker);
    if (idx < 0) {
      localError.value = t("config.persona.avatarSaveEncodeFailed");
      return;
    }
    emit("confirm", { mime: "image/webp", bytesBase64: dataUrl.slice(idx + marker.length) });
    closeDialog();
  } catch (error) {
    localError.value = t("config.persona.avatarSaveEncodeFailed");
    console.warn("[配置][头像裁剪] 保存失败", error);
  }
}

watch([() => props.open, () => props.source], async ([open]) => {
  if (!open) {
    destroyCropper();
    dialogRef.value?.close();
    return;
  }
  await nextTick();
  await initCropper();
});

onBeforeUnmount(destroyCropper);
</script>
