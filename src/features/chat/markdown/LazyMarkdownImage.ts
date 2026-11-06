import AppIcon from "../../shared/components/AppIcon.vue";
import { computed, defineComponent, h, nextTick, onBeforeUnmount, onMounted, ref, watch, type PropType } from "vue";
import { isDesktopTauriHost, readTransportChatImage, resolveLocalFileUrl } from "../../../services/tauri-api";
import { markdownImageDisplayMode, resolveMarkdownImageSource, type MarkdownImagePreviewPayload, type MarkdownImageSource } from "./MarkdownImage";

const transportThumbnailCache = new Map<string, string>();
const transportThumbnailPromiseCache = new Map<string, Promise<string>>();
const TRANSPORT_THUMBNAIL_CACHE_LIMIT = 40;

function cacheTransportThumbnail(path: string, dataUrl: string) {
  transportThumbnailCache.delete(path);
  transportThumbnailCache.set(path, dataUrl);
  while (transportThumbnailCache.size > TRANSPORT_THUMBNAIL_CACHE_LIMIT) {
    const oldestPath = transportThumbnailCache.keys().next().value;
    if (!oldestPath) break;
    transportThumbnailCache.delete(oldestPath);
  }
}

function localAssetUrl(source: MarkdownImageSource): string {
  if (source.kind !== "local") return "";
  return resolveLocalFileUrl(source.path);
}

function isMemeImagePath(path: string): boolean {
  return /(^|[/\\])\.meme([/\\]|$)/.test(String(path || ""));
}

export default defineComponent({
  name: "LazyMarkdownImage",
  props: {
    src: { type: String, required: true },
    alt: { type: String, default: "" },
    localImageBasePath: { type: String, default: "" },
    onOpenPreview: {
      type: Function as PropType<(payload: MarkdownImagePreviewPayload) => void>,
      default: undefined,
    },
  },
  setup(imageProps) {
    const rootRef = ref<HTMLElement | null>(null);
    const inViewport = ref(false);
    const imageLoaded = ref(false);
    const imageErrored = ref(false);
    const transportThumbnailSrc = ref("");
    const source = computed(() => resolveMarkdownImageSource(imageProps.src, imageProps.localImageBasePath));
    let observer: IntersectionObserver | null = null;
    let thumbnailLoadVersion = 0;

    function ensureVisible() {
      inViewport.value = true;
      observer?.disconnect();
      observer = null;
    }

    function resetObserver() {
      observer?.disconnect();
      observer = null;
      inViewport.value = false;
      imageLoaded.value = false;
      imageErrored.value = false;
      transportThumbnailSrc.value = "";
    }

    function observeRoot() {
      if (typeof IntersectionObserver === "undefined") {
        ensureVisible();
        return;
      }
      observer = new IntersectionObserver((entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          ensureVisible();
        }
      }, { rootMargin: "200px 0px" });
      if (rootRef.value) observer.observe(rootRef.value);
    }

    onMounted(observeRoot);

    onBeforeUnmount(() => {
      thumbnailLoadVersion += 1;
      observer?.disconnect();
      observer = null;
    });

    watch(source, async () => {
      resetObserver();
      await nextTick();
      observeRoot();
    });

    watch(
      [source, inViewport],
      ([current, visible]) => {
        const version = ++thumbnailLoadVersion;
        transportThumbnailSrc.value = "";
        if (!visible || markdownImageDisplayMode(current, localAssetUrl(current), isDesktopTauriHost()) !== "transport" || current.kind !== "local") return;
        const path = current.path;
        const cached = transportThumbnailCache.get(path);
        if (cached) {
          transportThumbnailSrc.value = cached;
          return;
        }
        const existing = transportThumbnailPromiseCache.get(path);
        const task = existing || readTransportChatImage({ path })
          .then((result) => {
            const dataUrl = String(result?.dataUrl || "").trim();
            if (dataUrl) cacheTransportThumbnail(path, dataUrl);
            transportThumbnailPromiseCache.delete(path);
            return dataUrl;
          })
          .catch((error) => {
            transportThumbnailPromiseCache.delete(path);
            console.warn("[Markdown图片] 本地缩略图加载失败", { path, error });
            return "";
          });
        if (!existing) transportThumbnailPromiseCache.set(path, task);
        void task.then((dataUrl) => {
          if (version !== thumbnailLoadVersion) return;
          transportThumbnailSrc.value = dataUrl;
          if (!dataUrl) imageErrored.value = true;
        });
      },
      { immediate: true },
    );

    return () => {
      const current = source.value;
      const alt = String(imageProps.alt || "").trim();
      const openPreview = () => {
        if (typeof imageProps.onOpenPreview !== "function") return;
        if (current.kind === "remote") {
          imageProps.onOpenPreview({ src: current.src, alt });
          return;
        }
        if (current.kind === "local") {
          imageProps.onOpenPreview({ localPath: current.path, alt: alt || current.path });
        }
      };

      if (current.kind === "blocked") {
        return h("span", { ref: rootRef, class: "ecall-md-image-placeholder ecall-md-image-error" }, alt || current.label || imageProps.src);
      }

      const displayMode = markdownImageDisplayMode(current, localAssetUrl(current), isDesktopTauriHost());
      const resolvedSrc = displayMode === "remote" && current.kind === "remote"
        ? current.src
        : displayMode === "asset"
          ? localAssetUrl(current)
          : displayMode === "transport"
            ? transportThumbnailSrc.value
            : "";
      const title = current.kind === "local" ? (alt || current.path) : alt;
      const imageClass = current.kind === "local" && isMemeImagePath(current.path)
        ? "ecall-md-meme-image"
        : "";
      const errorLabel = current.kind === "local"
        ? (alt || current.path.split(/[\\/]/).filter(Boolean).pop() || current.path)
        : (alt || current.src);

      return h("span", {
        ref: rootRef,
        class: "relative inline-block max-w-full align-middle",
      }, [
        !imageLoaded.value && !imageErrored.value
          ? h("span", {
            class: "ecall-md-image-skeleton skeleton inline-flex aspect-video w-64 max-w-full items-center justify-center rounded-lg",
            "aria-hidden": "true",
          }, [h(AppIcon, { name: "image", class: "h-8 w-8 text-base-content/20" })])
          : null,
        inViewport.value && resolvedSrc
          ? h("img", {
            class: ["ecall-md-image", imageClass, "cursor-zoom-in"],
            src: resolvedSrc,
            alt: title,
            title,
            // IntersectionObserver 已经负责懒加载；进入视口后必须立即发起请求。
            loading: "eager",
            decoding: "async",
            // 不能使用 display:none，否则浏览器可能永远不加载 lazy 图片，形成骨架死锁。
            style: imageLoaded.value
              ? undefined
              : { position: "absolute", inset: "0", width: "100%", height: "100%", opacity: "0", pointerEvents: "none" },
            onLoad: () => {
              imageLoaded.value = true;
              imageErrored.value = false;
            },
            onError: () => {
              imageLoaded.value = false;
              imageErrored.value = true;
            },
            onClick: (event: MouseEvent) => {
              event.preventDefault();
              event.stopPropagation();
              openPreview();
            },
          })
          : null,
        imageErrored.value
          ? h("span", {
            class: "ecall-md-image-placeholder ecall-md-image-error cursor-zoom-in",
            title,
            onClick: (event: MouseEvent) => {
              event.preventDefault();
              event.stopPropagation();
              openPreview();
            },
          }, errorLabel)
          : null,
      ]);
    };
  },
});
