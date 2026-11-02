<template>
  <div
    v-if="kind === 'image'"
    class="file-reader-media-stage h-full overflow-auto"
    @contextmenu.prevent="emit('contextMenu', $event)"
  >
    <img
      class="file-reader-media-image"
      :src="src"
      :alt="title"
      @error="emit('loadError')"
    />
  </div>
  <div
    v-else-if="kind === 'audio'"
    class="flex h-full items-center justify-center px-6"
    @contextmenu.prevent="emit('contextMenu', $event)"
  >
    <audio
      class="w-full max-w-3xl"
      :src="src"
      controls
      preload="metadata"
      @error="emit('loadError')"
    ></audio>
  </div>
  <div
    v-else-if="kind === 'video'"
    class="file-reader-media-stage h-full bg-base-200/35"
    @contextmenu.prevent="emit('contextMenu', $event)"
  >
    <video
      class="file-reader-media-video"
      :src="src"
      controls
      preload="metadata"
      @error="emit('loadError')"
    ></video>
  </div>
</template>

<script setup lang="ts">
/**
 * 文件阅读器里的图片 / 音频 / 视频预览。
 *
 * 单独成组件是为了让同一份渲染在阅读器之外也能被复用（demo 页），
 * 不在别处再写一套媒体预览。来源 url 由调用方决定：
 * 磁盘文件走本地文件 url，聊天里的远程图走内联 dataUrl。
 */
defineProps<{
  kind: "image" | "audio" | "video";
  src: string;
  title?: string;
}>();

const emit = defineEmits<{
  (e: "loadError"): void;
  (e: "contextMenu", event: MouseEvent): void;
}>();
</script>

<style scoped>
.file-reader-media-stage {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  padding: 1rem;
}
.file-reader-media-image {
  display: block;
  max-width: 100%;
  max-height: calc(100vh - 6rem);
  width: auto;
  height: auto;
  object-fit: contain;
}
.file-reader-media-video {
  display: block;
  width: 100%;
  height: 100%;
  max-height: calc(100vh - 4rem);
  object-fit: contain;
  background: #000;
}
</style>
