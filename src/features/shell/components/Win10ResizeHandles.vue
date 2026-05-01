<template>
  <div v-if="enabled" class="win10-resize-handles" aria-hidden="true">
    <div
      v-for="handle in resizeHandles"
      :key="handle.direction"
      :class="['win10-resize-handle', `win10-resize-handle-${handle.edge}`]"
      @mousedown.prevent="startResize(handle.direction)"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

type ResizeDirection = "East" | "North" | "NorthEast" | "NorthWest" | "South" | "SouthEast" | "SouthWest" | "West";

defineProps<{
  enabled: boolean;
}>();

const resizeHandles = computed<Array<{ edge: string; direction: ResizeDirection }>>(() => [
  { edge: "north", direction: "North" },
  { edge: "south", direction: "South" },
  { edge: "west", direction: "West" },
  { edge: "east", direction: "East" },
  { edge: "north-west", direction: "NorthWest" },
  { edge: "north-east", direction: "NorthEast" },
  { edge: "south-west", direction: "SouthWest" },
  { edge: "south-east", direction: "SouthEast" },
]);

function startResize(direction: ResizeDirection): void {
  getCurrentWindow().startResizeDragging(direction).catch((error) => {
    console.warn("[窗口] Win10 自定义边缘缩放启动失败", error);
  });
}
</script>

<style scoped>
.win10-resize-handles {
  position: fixed;
  inset: 0;
  z-index: 9998;
  pointer-events: none;
}

.win10-resize-handle {
  position: absolute;
  pointer-events: auto;
  user-select: none;
  -webkit-user-select: none;
}

.win10-resize-handle-north,
.win10-resize-handle-south {
  left: 8px;
  right: 8px;
  height: 6px;
  cursor: ns-resize;
}

.win10-resize-handle-north {
  top: 0;
}

.win10-resize-handle-south {
  bottom: 0;
}

.win10-resize-handle-west,
.win10-resize-handle-east {
  top: 8px;
  bottom: 8px;
  width: 6px;
  cursor: ew-resize;
}

.win10-resize-handle-west {
  left: 0;
}

.win10-resize-handle-east {
  right: 0;
}

.win10-resize-handle-north-west,
.win10-resize-handle-north-east,
.win10-resize-handle-south-west,
.win10-resize-handle-south-east {
  width: 10px;
  height: 10px;
}

.win10-resize-handle-north-west {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.win10-resize-handle-north-east {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.win10-resize-handle-south-west {
  bottom: 0;
  left: 0;
  cursor: nesw-resize;
}

.win10-resize-handle-south-east {
  right: 0;
  bottom: 0;
  cursor: nwse-resize;
}
</style>
