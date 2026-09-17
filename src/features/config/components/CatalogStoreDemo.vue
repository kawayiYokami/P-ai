<template>
  <div class="mx-auto w-full max-w-3xl">
    <!-- 搜索：唯一的入口，跨源聚合，用户不需要知道有哪几个源 -->
    <label class="input input-bordered flex w-full items-center gap-2">
      <Search class="h-4 w-4 opacity-50" />
      <input
        v-model="query"
        type="text"
        class="grow text-sm"
        placeholder="想让助手学会什么？例如「地图」「看代码」「做 PPT」"
      />
      <button v-if="query" type="button" class="btn btn-ghost btn-xs btn-circle" @click="query = ''">✕</button>
    </label>

    <!-- 分类：只分「装到哪一页」这一件事，不再叠加维度 -->
    <div class="mt-3 flex items-center gap-1">
      <button
        v-for="option in groupOptions"
        :key="option.value"
        type="button"
        class="btn btn-sm min-h-[2rem] h-8"
        :class="group === option.value ? 'btn-neutral' : 'btn-ghost text-base-content/60'"
        @click="group = option.value"
      >
        {{ option.label }}
      </button>
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="mt-4 rounded-xl border border-base-300 bg-base-100 px-4 py-12 text-center">
      <span class="loading loading-spinner loading-sm text-primary"></span>
      <p class="mt-2 text-xs text-base-content/50">正在搜索…</p>
    </div>

    <!-- 卡片：连接器与技能各按其决策重点布局 -->
    <div v-else-if="visible.length > 0" class="config-grid-auto-sm mt-4">
      <template v-for="item in visible" :key="item.name">
        <!-- 连接器卡：工程面板感，热度归到底部与作者同行，左侧 info 色条 -->
        <div
          v-if="item.group === 'connector'"
          class="rounded-2xl border border-base-300 border-l-4 border-l-info bg-base-100 p-4 flex flex-col justify-between gap-3"
        >
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1 truncate text-sm font-semibold text-base-content" :title="item.name">
              {{ item.name }}
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <span v-if="item.transport" class="badge badge-sm badge-ghost font-mono">{{ item.transport }}</span>
              <span v-if="item.needsKey" class="badge badge-sm badge-outline border-warning/40 text-warning">
                需密钥
              </span>
            </div>
          </div>

          <p class="line-clamp-2 min-h-[2.5rem] text-xs leading-relaxed text-base-content/70">
            {{ item.description }}
          </p>

          <!-- 工具清单：部分来源不提供，缺失时整块不渲染 -->
          <div v-if="item.tools && item.tools.length" class="rounded-md border border-base-300 bg-base-200/50 px-2.5 py-2">
            <div class="flex flex-wrap gap-1">
              <span
                v-for="tool in item.tools.slice(0, TOOL_PREVIEW_LIMIT)"
                :key="tool"
                class="max-w-[9rem] truncate rounded border border-base-300 bg-base-100 px-1.5 py-0.5 font-mono text-caption"
              >
                {{ tool }}
              </span>
              <span v-if="item.tools.length > TOOL_PREVIEW_LIMIT" class="px-1 py-0.5 font-mono text-caption opacity-50">
                +{{ item.tools.length - TOOL_PREVIEW_LIMIT }}
              </span>
            </div>
          </div>

          <div class="flex items-center justify-between border-t border-base-300 pt-2.5">
            <div class="flex min-w-0 items-center gap-1.5 text-caption opacity-60">
              <span class="max-w-[9rem] truncate" :title="item.author">{{ item.author }}</span>
              <span class="opacity-50">·</span>
              <span class="shrink-0">{{ formatPopularity(item.popularity) }}</span>
            </div>
            <template v-if="item.installed">
              <span class="text-caption opacity-60">{{ item.enabled ? "已启用" : "已关闭" }}</span>
              <input
                type="checkbox"
                class="toggle toggle-xs toggle-primary"
                :checked="item.enabled"
                @change="item.enabled = !item.enabled"
              />
            </template>
            <button
              v-else
              type="button"
              class="btn btn-primary btn-sm h-8 min-h-[2rem] px-3.5"
              :disabled="item.installing"
              @click="install(item)"
            >
              <span v-if="item.installing" class="loading loading-spinner loading-xs"></span>
              <span v-else>安装</span>
            </button>
          </div>
        </div>

        <!-- 技能卡：内容卡感，热度提到顶部与名称同行，左侧 accent 色条 -->
        <div
          v-else
          class="rounded-2xl border border-base-300 border-l-4 border-l-accent bg-base-100 p-4 flex flex-col justify-between gap-3"
        >
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1 truncate font-mono text-sm font-semibold text-base-content" :title="item.name">
              {{ item.name }}
            </div>
            <div class="flex shrink-0 items-center gap-1.5 text-caption opacity-60">
              <span class="max-w-[8rem] truncate" :title="item.author">{{ item.author }}</span>
              <span class="opacity-50">·</span>
              <span>{{ formatPopularity(item.popularity) }}</span>
            </div>
          </div>

          <p class="line-clamp-3 min-h-[3.25rem] text-xs leading-relaxed text-base-content/70">
            {{ item.description }}
          </p>

          <div class="flex items-center justify-end gap-1.5 border-t border-base-300 pt-2.5">
            <template v-if="item.installed">
              <span class="text-caption opacity-60">{{ item.enabled ? "已启用" : "已关闭" }}</span>
              <input
                type="checkbox"
                class="toggle toggle-xs toggle-primary"
                :checked="item.enabled"
                @change="item.enabled = !item.enabled"
              />
            </template>
            <button
              v-else
              type="button"
              class="btn btn-primary btn-sm h-8 min-h-[2rem] px-3.5"
              :disabled="item.installing"
              @click="install(item)"
            >
              <span v-if="item.installing" class="loading loading-spinner loading-xs"></span>
              <span v-else>安装</span>
            </button>
          </div>
        </div>
      </template>
    </div>

    <!-- 空态：告诉用户下一步做什么，而不是只报错 -->
    <div v-else class="mt-4 rounded-xl border border-dashed border-base-300 px-4 py-10 text-center">
      <p class="text-sm text-base-content/70">没有找到「{{ query }}」</p>
      <p class="mt-1 text-xs text-base-content/45">换个说法试试，比如只说用途</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Search } from "@lucide/vue";

type StoreGroup = "all" | "connector" | "skill";

type StoreItem = {
  name: string;
  description: string;
  group: Exclude<StoreGroup, "all">;
  author: string;
  popularity: number;
  /// 仅连接器：传输方式，空串表示来源未提供。
  transport?: string;
  /// 仅连接器：工具清单，缺失表示来源未提供。
  tools?: string[];
  needsKey: boolean;
  installed: boolean;
  /// 已安装时的启用状态。
  enabled: boolean;
  installing: boolean;
};

const groupOptions: Array<{ value: StoreGroup; label: string }> = [
  { value: "all", label: "全部" },
  { value: "connector", label: "连接器" },
  { value: "skill", label: "技能" },
];

const TOOL_PREVIEW_LIMIT = 6;

const query = ref("");
const group = ref<StoreGroup>("all");
const loading = ref(false);

const items = ref<StoreItem[]>([
  {
    name: "高德地图",
    description:
      "高德地图是一个支持任何 MCP 协议客户端的服务器，允许用户轻松利用高德地图 MCP 服务器获取各种基于位置的服务。",
    group: "connector",
    author: "amap",
    popularity: 140000000,
    transport: "stdio",
    tools: ["地图搜索", "路径规划", "周边检索", "地理编码", "天气查询", "距离测量", "IP 定位", "行政区划查询"],
    needsKey: true,
    installed: false,
    enabled: false,
    installing: false,
  },
  {
    name: "必应搜索中文",
    description: "让 AI 助手能够使用必应搜索引擎实时获取网络信息，专为中文搜索优化，无需申请 API 密钥。",
    group: "connector",
    author: "slcatwujian",
    popularity: 130000000,
    transport: "stdio",
    needsKey: false,
    installed: false,
    enabled: false,
    installing: false,
  },
  {
    name: "GitHub 官方 MCP",
    description: "查看仓库、检索 issue、读取提交记录与拉取请求，由 GitHub 官方维护。",
    group: "connector",
    author: "github",
    popularity: 33000,
    transport: "streamable_http",
    needsKey: true,
    installed: true,
    enabled: true,
    installing: false,
  },
  {
    name: "12306 车票查询",
    description: "查询车次、余票与票价，支持按出发到达站与日期检索。",
    group: "connector",
    author: "modelscope",
    popularity: 162000000,
    transport: "stdio",
    tools: ["车次查询", "余票查询", "中转换乘", "车站检索", "票价查询"],
    needsKey: false,
    installed: false,
    enabled: false,
    installing: false,
  },
  {
    name: "前端设计",
    description:
      "给界面出配色、排版与交互建议，包含对间距、层级与可读性的具体判断，可直接用于日常界面打磨。",
    group: "skill",
    author: "社区作者",
    popularity: 14835,
    needsKey: false,
    installed: false,
    enabled: false,
    installing: false,
  },
  {
    name: "幻灯片生成",
    description: "把零散内容整理成一页页幻灯片，自动分出章节与要点，适合快速产出汇报初稿。",
    group: "skill",
    author: "社区作者",
    popularity: 9200,
    needsKey: false,
    installed: false,
    enabled: false,
    installing: false,
  },
  {
    name: "流程教学",
    description: "教助手学会你的一套固定做法，把口头经验沉淀成可复用的步骤说明。",
    group: "skill",
    author: "社区作者",
    popularity: 4100,
    needsKey: false,
    installed: true,
    enabled: false,
    installing: false,
  },
  {
    name: "经验复盘",
    description: "定期回看并沉淀踩过的坑，把散落的教训整理成下一次可以直接引用的清单。",
    group: "skill",
    author: "社区作者",
    popularity: 2600,
    needsKey: false,
    installed: false,
    enabled: false,
    installing: false,
  },
]);

const filtered = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return items.value.filter((item) => {
    if (group.value !== "all" && item.group !== group.value) return false;
    if (!keyword) return true;
    return `${item.name}${item.description}`.toLowerCase().includes(keyword);
  });
});

const visible = computed(() => filtered.value);

let loadingTimer: number | undefined;

watch([query, group], () => {
  loading.value = true;
  window.clearTimeout(loadingTimer);
  loadingTimer = window.setTimeout(() => {
    loading.value = false;
  }, 350);
});

function formatPopularity(value: number): string {
  if (!value || value <= 0) return "0";
  if (value >= 100000000) return `${(value / 100000000).toFixed(1)} 亿`;
  if (value >= 10000) return `${(value / 10000).toFixed(1)} 万`;
  return value.toLocaleString();
}

function install(item: StoreItem) {
  item.installing = true;
  window.setTimeout(() => {
    item.installing = false;
    item.installed = true;
    // 与真实行为一致：商店安装的条目默认关闭。
    item.enabled = false;
  }, 600);
}
</script>
