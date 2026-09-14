<template>
  <div class="space-y-4 p-1">
    <!-- 控制栏：分类切换 + 宽度切换 + 主题切换 -->
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-base-300 bg-base-100 p-3 shadow-2xs">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-xs font-semibold text-base-content/70">分类：</span>
        <SegmentedControl
          v-model="activeTab"
          :options="tabSegmentOptions"
          :full-width="false"
          size="xs"
        />
      </div>

      <div class="flex flex-wrap items-center gap-3">
        <div class="flex items-center gap-1.5">
          <span class="text-xs font-semibold text-base-content/70">画布宽：</span>
          <SegmentedControl
            v-model="activeWidth"
            :options="widthSegmentOptions"
            :full-width="false"
            size="xs"
          />
          <span class="font-mono text-caption opacity-50">{{ currentWidthStyle ? currentWidthStyle : '自适应' }}</span>
        </div>

        <div class="flex items-center gap-1.5">
          <span class="text-xs font-semibold text-base-content/70">主题：</span>
          <select v-model="currentTheme" class="select select-bordered select-xs w-28" @change="applyTheme">
            <option value="light">明亮 (Light)</option>
            <option value="dark">暗黑 (Dark)</option>
            <option value="cupcake">纸杯蛋糕</option>
            <option value="dracula">德古拉</option>
            <option value="nord">极光 (Nord)</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 画布容器 -->
    <div
      class="mx-auto rounded-2xl border border-base-300 bg-base-200/60 p-4 transition-all duration-200 space-y-6"
      :style="{ width: currentWidthStyle, maxWidth: '100%' }"
    >
      <!-- 1. 供应商卡片 (ApiTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'api'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Server class="h-4 w-4 text-primary" />
            <h4 class="text-sm font-bold text-base-content">供应商与模型 (API Providers)</h4>
            <span class="badge badge-sm badge-neutral">{{ mockProviders.length }}</span>
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-md (min: 20rem)</span>
        </div>

        <div class="config-grid-auto-md">
          <div
            v-for="provider in mockProviders"
            :key="provider.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-3.5 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex items-center justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          >
            <!-- 左侧：供应商名称 + 状态徽章 + 端点地址 -->
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                  {{ provider.name }}
                </span>
                <span v-if="provider.isDefault" class="badge badge-primary badge-xs shrink-0">默认</span>
                <span v-if="provider.isDirty" class="badge badge-warning badge-xs shrink-0">未保存</span>
              </div>
              <div class="font-mono text-caption opacity-50 truncate mt-1">
                {{ formatEndpointDisplay(provider.baseUrl) }}
              </div>
            </div>

            <!-- 右侧：协议格式 + 进入箭头 -->
            <div class="shrink-0 flex items-center gap-2">
              <span class="badge badge-ghost badge-xs font-mono uppercase">{{ provider.requestFormat }}</span>
              <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
            </div>
          </div>
        </div>
      </section>

      <!-- 2. 技能卡片 (SkillTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'skill'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Sparkles class="h-4 w-4 text-secondary" />
            <h4 class="text-sm font-bold text-base-content">技能库 (Skills)</h4>
            <span class="badge badge-sm badge-neutral">{{ mockSkills.length }}</span>
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-sm (min: 18rem)</span>
        </div>

        <div class="config-grid-auto-sm">
          <div
            v-for="item in mockSkills"
            :key="item.name"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          >
            <!-- 头部：技能名称 + 内置标记 -->
            <div class="flex items-center justify-between gap-2 min-w-0">
              <div class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors flex-1 min-w-0">
                {{ item.name }}
              </div>
              <span v-if="item.isBuiltin" class="badge badge-ghost badge-xs shrink-0 font-mono opacity-70">内置</span>
            </div>

            <!-- 中部：描述预览（无描述时呈现优雅占位，高度固定 2.5rem 保持整齐） -->
            <p class="text-xs line-clamp-2 leading-relaxed min-h-[2.5rem] break-words" :class="item.description ? 'text-base-content/70' : 'text-base-content/40 italic'">
              {{ item.description || '暂无描述' }}
            </p>

            <!-- 底栏：正文内容规模 + 进入箭头 -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption opacity-70">
              <span class="font-mono text-xs">{{ item.contentWords }} 字正文</span>
              <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
            </div>
          </div>
        </div>
      </section>

      <!-- 3. 连接器卡片 (McpTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'mcp'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Cpu class="h-4 w-4 text-accent" />
            <h4 class="text-sm font-bold text-base-content">连接器 (MCP Connectors)</h4>
            <span class="badge badge-sm badge-neutral">{{ mockMcpServers.length }}</span>
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-md (min: 20rem)</span>
        </div>

        <div class="config-grid-auto-md">
          <div
            v-for="server in mockMcpServers"
            :key="server.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
            :class="{ 'opacity-65 bg-base-100/60': !server.enabled }"
          >
            <!-- 头部：名称/命令 + 启用开关 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ server.name }}
                  </span>
                  <span v-if="server.isDirty" class="badge badge-warning badge-xs shrink-0">未保存</span>
                </div>
                <div v-if="server.command" class="font-mono text-caption opacity-50 truncate mt-0.5">
                  {{ server.command }}
                </div>
              </div>

              <input
                type="checkbox"
                :checked="server.enabled"
                class="toggle toggle-sm toggle-success shrink-0"
                @click.stop
              />
            </div>

            <!-- 多成员聚合提示（仅当存在多个子服务时展示） -->
            <div v-if="server.members" class="text-caption opacity-60 truncate font-mono">
              {{ server.members }}
            </div>

            <!-- 底栏：启用时显示状态与工具数；未启用时左侧留空，右上角 Toggle 已经自明 -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption">
              <div class="flex items-center gap-1.5 min-h-[1.5rem]">
                <template v-if="server.enabled">
                  <span class="badge badge-sm" :class="server.statusClass">
                    {{ server.statusText }}
                  </span>
                  <span class="badge badge-sm badge-ghost">
                    {{ server.toolCount }} 工具
                  </span>
                </template>
              </div>
              <div class="flex items-center gap-1.5 opacity-70">
                <span v-if="server.transport && server.transport !== 'mcp'" class="font-mono text-caption opacity-60">
                  {{ server.transport }}
                </span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 4. 人格卡片 (PersonaTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'persona'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <UserCircle class="h-4 w-4 text-info" />
            <h4 class="text-sm font-bold text-base-content">人格成员 (Personas)</h4>
            <span class="badge badge-sm badge-neutral">{{ mockPersonas.length }}</span>
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-md (min: 20rem)</span>
        </div>

        <div class="config-grid-auto-md">
          <div
            v-for="persona in mockPersonas"
            :key="persona.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          >
            <!-- 头部：头像 + 姓名 + 部门与身份 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="flex items-center gap-2.5 min-w-0 flex-1">
                <div class="avatar shrink-0">
                  <div class="w-10 h-10 rounded-full ring-1 ring-base-200 overflow-hidden flex items-center justify-center bg-primary/10 text-primary font-bold text-sm">
                    {{ persona.initial }}
                  </div>
                </div>

                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-1.5">
                    <span class="font-semibold text-sm truncate group-hover:text-primary transition-colors">
                      {{ persona.name }}
                    </span>
                    <span v-if="persona.isUser" class="badge badge-info badge-xs shrink-0 font-medium">用户</span>
                    <span v-else-if="persona.isSystem" class="badge badge-neutral badge-xs shrink-0 font-medium">系统</span>
                  </div>

                  <!-- 部门归属（干净清爽，不挂刺眼黄色未入部门标签） -->
                  <div class="mt-1 flex flex-wrap items-center gap-1">
                    <span
                      v-if="persona.department"
                      class="badge badge-ghost badge-xs gap-1 font-mono text-caption"
                    >
                      <Building2 class="h-3 w-3 opacity-60" />
                      {{ persona.department }}
                    </span>
                    <span
                      v-if="persona.isDefault"
                      class="badge badge-primary badge-outline badge-xs text-caption"
                    >
                      默认助理
                    </span>
                  </div>
                </div>
              </div>

              <!-- 右上角删除按钮（占位） -->
              <button
                v-if="persona.canDelete"
                type="button"
                class="btn btn-ghost btn-xs btn-circle opacity-0 group-hover:opacity-100 hover:text-error transition-opacity shrink-0"
                title="删除"
                @click.stop
              >
                <Trash2 class="h-4 w-4" />
              </button>
            </div>

            <!-- 中部：Prompt 预览（直接在卡片上自然呈现，杜绝卡片套卡片） -->
            <p class="text-xs text-base-content/70 line-clamp-2 leading-relaxed min-h-[2.5rem] break-words">
              {{ persona.prompt }}
            </p>

            <!-- 底栏：特性标签 + 进入指示（无多余“详情”文字） -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption opacity-70">
              <div class="flex items-center gap-1.5">
                <span v-if="persona.privateMemory" class="badge badge-sm badge-accent badge-outline text-caption">
                  私有记忆
                </span>
                <span v-if="persona.recallMode" class="badge badge-sm badge-ghost text-caption">
                  {{ persona.recallMode }}
                </span>
              </div>
              <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all ml-auto" />
            </div>
          </div>
        </div>
      </section>

      <!-- 5. 部门卡片 (DepartmentTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'department'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Users class="h-4 w-4 text-warning" />
            <h4 class="text-sm font-bold text-base-content">部门组织 (Departments)</h4>
            <SegmentedControl
              v-model="deptCategoryTab"
              :options="deptCategoryOptions"
              :full-width="false"
              size="xs"
              class="ml-2"
            />
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-md (min: 20rem)</span>
        </div>

        <div class="config-grid-auto-md">
          <div
            v-for="dept in displayedMockDepartments"
            :key="dept.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          >
            <!-- 头部：部门名称 + 徽章 + 模型 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <span class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ dept.name }}
                  </span>
                  <span v-if="dept.isAssistant" class="badge badge-primary badge-xs">助理办</span>
                  <span v-else-if="dept.isBuiltin" class="badge badge-neutral badge-xs opacity-70">内置</span>
                </div>
                <div class="text-caption opacity-50 truncate mt-0.5 font-mono">
                  {{ dept.model }}
                </div>
              </div>
            </div>

            <!-- 中间：部门简介 -->
            <p class="text-xs text-base-content/70 line-clamp-2 leading-relaxed min-h-[2.5rem] break-words">
              {{ dept.summary }}
            </p>

            <!-- 底栏：成员数 + 权限模式（仅开启白名单等规则时才显示） + 进入指示 -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption opacity-70">
              <div class="flex items-center gap-1.5">
                <Users class="h-3.5 w-3.5 opacity-60" />
                <span>{{ dept.membersCount }} 位成员</span>
              </div>
              <div class="flex items-center gap-1.5">
                <span v-if="dept.permissionMode !== '权限未启用'" class="font-mono text-xs">
                  {{ dept.permissionMode }}
                </span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 6. 联系人渠道卡片 (RemoteImTab) -->
      <section v-if="activeTab === 'all' || activeTab === 'remoteIm'" class="space-y-2.5">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Radio class="h-4 w-4 text-success" />
            <h4 class="text-sm font-bold text-base-content">外部接入与联系人 (Remote IM)</h4>
            <span class="badge badge-sm badge-neutral">{{ mockChannels.length }}</span>
          </div>
          <span class="text-caption opacity-50 font-mono">.config-grid-auto-sm (min: 18rem)</span>
        </div>

        <div class="config-grid-auto-sm">
          <div
            v-for="ch in mockChannels"
            :key="ch.id"
            role="button"
            tabindex="0"
            class="rounded-xl border border-base-200/80 bg-base-100 p-4 hover:border-primary/50 hover:shadow-md transition-all duration-150 cursor-pointer flex flex-col justify-between gap-3 select-none active:scale-[0.99] shadow-2xs group"
          >
            <!-- 头部：平台图标 + 渠道名称 + 平台标识 + 启停开关 -->
            <div class="flex items-start justify-between gap-2.5 min-w-0">
              <div class="flex items-center gap-2.5 min-w-0 flex-1">
                <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border font-bold text-xs shadow-2xs" :class="ch.platformColor">
                  {{ ch.platformBadge }}
                </div>
                <div class="min-w-0 flex-1">
                  <div class="text-sm font-semibold text-base-content truncate group-hover:text-primary transition-colors">
                    {{ ch.name }}
                  </div>
                  <div class="text-caption opacity-50 truncate mt-0.5">
                    {{ ch.platformLabel }}
                  </div>
                </div>
              </div>

              <!-- 启用开关 -->
              <input
                type="checkbox"
                class="toggle toggle-primary toggle-sm shrink-0"
                :checked="ch.enabled"
                @click.stop
              />
            </div>

            <!-- 底栏：在线状态 + 联系人计数 + 进入箭头 -->
            <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5 text-caption">
              <div class="flex items-center gap-1.5">
                <span class="size-2 rounded-full shrink-0" :class="ch.statusDot"></span>
                <span class="opacity-70">{{ ch.statusText }}</span>
              </div>
              <div class="flex items-center gap-2">
                <span class="font-mono opacity-60">{{ ch.contactsCount }} 位联系人</span>
                <ChevronRight class="h-3.5 w-3.5 opacity-40 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" />
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Building2,
  ChevronRight,
  Cpu,
  Radio,
  Server,
  Sparkles,
  Trash2,
  UserCircle,
  Users,
} from "@lucide/vue";

import SegmentedControl from "./SegmentedControl.vue";
import { formatEndpointDisplay } from "../utils/api-config-display";

const urlParams = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
const initialTabParam = urlParams?.get("tab") as any;
const initialWidthParam = urlParams?.get("w") as any;
const initialThemeParam = urlParams?.get("theme");

const activeTab = ref<"all" | "api" | "skill" | "mcp" | "persona" | "department" | "remoteIm">(
  ["all", "api", "skill", "mcp", "persona", "department", "remoteIm"].includes(initialTabParam) ? initialTabParam : "all"
);
const activeWidth = ref<"380" | "640" | "860" | "1200" | "full">(
  ["380", "640", "860", "1200", "full"].includes(initialWidthParam) ? initialWidthParam : "860"
);
const currentTheme = ref(initialThemeParam || "light");
if (initialThemeParam && typeof document !== "undefined") {
  document.documentElement.setAttribute("data-theme", initialThemeParam);
}

const tabSegmentOptions = [
  { value: "all", label: "全部 6 类" },
  { value: "api", label: "供应商" },
  { value: "skill", label: "技能" },
  { value: "mcp", label: "连接器" },
  { value: "persona", label: "人格" },
  { value: "department", label: "部门" },
  { value: "remoteIm", label: "联系人渠道" },
];

const widthSegmentOptions = [
  { value: "380", label: "窄窗 (380px)" },
  { value: "640", label: "半屏 (640px)" },
  { value: "860", label: "标准窗 (860px)" },
  { value: "1200", label: "宽屏 (1200px)" },
  { value: "full", label: "全宽 100%" },
];

const currentWidthStyle = computed(() => {
  if (activeWidth.value === "full") return "100%";
  return `${activeWidth.value}px`;
});

function applyTheme() {
  document.documentElement.setAttribute("data-theme", currentTheme.value);
}

// 1. 模拟供应商数据
const mockProviders = [
  {
    id: "deepseek-official",
    name: "DeepSeek 官方 API",
    baseUrl: "https://api.deepseek.com/v1",
    requestFormat: "openai",
    isDefault: true,
    isDirty: false,
  },
  {
    id: "anthropic-direct",
    name: "Anthropic Claude",
    baseUrl: "https://api.anthropic.com",
    requestFormat: "anthropic",
    isDefault: false,
    isDirty: false,
  },
  {
    id: "siliconflow-cloud",
    name: "硅基流动 SiliconFlow",
    baseUrl: "https://api.siliconflow.cn/v1",
    requestFormat: "openai",
    isDefault: false,
    isDirty: true,
  },
  {
    id: "local-ollama",
    name: "本地 Ollama 引擎",
    baseUrl: "http://localhost:11434/v1",
    requestFormat: "openai",
    isDefault: false,
    isDirty: false,
  },
];

// 2. 模拟技能数据
const mockSkills = [
  {
    name: "git-rebase-wizard",
    description: "智能分析本地 Git 分支拓扑，自动生成高质量 rebase 和 squash 命令链，安全处理变基冲突并提供撤销点",
    descWords: 48,
    contentWords: 1420,
    isBuiltin: false,
  },
  {
    name: "vue3-component-refactor",
    description: "第一性原理重构 Vue 3 组件，提取组合式函数，消除冗余样式与无用标签，保持 DaisyUI 一致性",
    descWords: 52,
    contentWords: 2380,
    isBuiltin: false,
  },
  {
    name: "sql-explain-analyzer",
    description: "深度剖析 PostgreSQL / MySQL 执行计划，识别全表扫描和索引失效，给出重写建议",
    descWords: 42,
    contentWords: 960,
    isBuiltin: false,
  },
  {
    name: "quick-note",
    description: "",
    descWords: 0,
    contentWords: 150,
    isBuiltin: false,
  },
  {
    name: "file-search",
    description: "系统内置高速文件名与文本搜索能力，基于 ripgrep 与 fd 引擎实现精准定位",
    descWords: 38,
    contentWords: 820,
    isBuiltin: true,
  },
  {
    name: "bash-command-runner",
    description: "受限终端执行环境，支持在独立的工作区内运行自动化命令与脚手架验证",
    descWords: 36,
    contentWords: 1100,
    isBuiltin: true,
  },
];

// 3. 模拟 MCP 连接器数据
const mockMcpServers = [
  {
    id: "deepwiki",
    name: "deepwiki",
    command: "https://mcp.deepwiki.com/mcp",
    transport: "流式 http",
    toolCount: 3,
    enabled: true,
    statusText: "就绪",
    statusClass: "badge-success",
    members: "",
    isDirty: false,
  },
  {
    id: "github-mcp",
    name: "GitHub 官方连接器",
    command: "npx -y @modelcontextprotocol/server-github",
    transport: "npx",
    toolCount: 26,
    enabled: true,
    statusText: "就绪",
    statusClass: "badge-success",
    members: "3 个子服务: Issue 管理, PR 评审, 仓库搜索",
    isDirty: false,
  },
  {
    id: "postgres-inspector",
    name: "PostgreSQL 数据库",
    command: "uvx mcp-server-postgres",
    transport: "uvx",
    toolCount: 8,
    enabled: true,
    statusText: "就绪",
    statusClass: "badge-success",
    members: "", // 单体服务，不强行霸占空间
    isDirty: false,
  },
  {
    id: "puppeteer-browser",
    name: "Puppeteer 无头浏览器",
    command: "docker run -i --rm mcp/puppeteer",
    transport: "docker",
    toolCount: 5,
    enabled: true,
    statusText: "连接异常",
    statusClass: "badge-error",
    members: "",
    isDirty: true,
  },
  {
    id: "local-fetcher",
    name: "简易 HTTP 抓取器",
    command: "http://localhost:8080/mcp",
    transport: "流式 http",
    toolCount: 1,
    enabled: false,
    statusText: "未连接",
    statusClass: "badge-ghost",
    members: "",
    isDirty: false,
  },
];

// 4. 模拟人格数据
const mockPersonas = [
  {
    id: "architect",
    name: "架构师 Alex",
    initial: "A",
    prompt: "你是一位拥有 15 年经验的系统架构师，擅长用最简优雅的设计构建高内聚低耦合的代码体系，崇尚第一性原理思考。",
    department: "工程研发中心",
    noDepartment: false,
    isDefault: true,
    isUser: false,
    isSystem: false,
    privateMemory: true,
    recallMode: "主动回忆",
    canDelete: true,
  },
  {
    id: "writer",
    name: "技术写手 Chloe",
    initial: "C",
    prompt: "你专注于技术文档与开发手册的提炼，语言精炼清晰，从不使用空洞假大空的客套套话，直击问题核心。",
    department: "产品内容组",
    noDepartment: false,
    isDefault: false,
    isUser: false,
    isSystem: false,
    privateMemory: false,
    recallMode: "",
    canDelete: true,
  },
  {
    id: "default-assistant",
    name: "PAI 助理",
    initial: "P",
    prompt: "PAI 默认桌面对话助理，响应快速，支持系统控制、文件处理与日常工作流。",
    department: "助理办",
    noDepartment: false,
    isDefault: false,
    isUser: false,
    isSystem: true,
    privateMemory: false,
    recallMode: "",
    canDelete: false,
  },
  {
    id: "freelancer",
    name: "独立探索者",
    initial: "独",
    prompt: "无部门归属的自由实验智能体，用于尝试新提示词和原型能力。",
    department: "",
    noDepartment: true,
    isDefault: false,
    isUser: false,
    isSystem: false,
    privateMemory: true,
    recallMode: "仅上下文",
    canDelete: true,
  },
];

// 5. 模拟部门数据
const deptCategoryTab = ref<"custom" | "preset">("preset");
const mockDepartments = [
  {
    id: "dept-core-office",
    name: "核心助理办",
    model: "claude-3-7-sonnet",
    summary: "主控桌面调度与全局热键唤醒，处理日常快捷会话与即时任务分发。",
    membersCount: 2,
    permissionMode: "白名单模式",
    isAssistant: true,
    isBuiltin: true,
  },
  {
    id: "dept-leader-office",
    name: "领导办公室",
    model: "deepseek-reasoner",
    summary: "重大决策与高权重任务把关，把控关键技术方案走向与跨部门协同裁决。",
    membersCount: 1,
    permissionMode: "全权限开放",
    isAssistant: false,
    isBuiltin: true,
  },
  {
    id: "dept-deputy-office",
    name: "副手协同处",
    model: "deepseek-chat",
    summary: "日常事务性分流与长任务状态追踪，随时提供备选方案与阶段交付报告。",
    membersCount: 1,
    permissionMode: "黑名单模式",
    isAssistant: false,
    isBuiltin: true,
  },
  {
    id: "dept-engineering",
    name: "工程与研发中心",
    model: "deepseek-reasoner",
    summary: "负责系统底层架构、Tauri 原生接口集成、端到端响应优化与自动化测试体系。",
    membersCount: 6,
    permissionMode: "白名单模式",
    isAssistant: false,
    isBuiltin: false,
  },
  {
    id: "dept-qa",
    name: "质量与冒烟保障组",
    model: "gpt-4o-mini",
    summary: "负责单元测试验证、类型检查合规性扫描与发布前回归冒烟流程。",
    membersCount: 3,
    permissionMode: "黑名单模式",
    isAssistant: false,
    isBuiltin: false,
  },
  {
    id: "dept-empty",
    name: "新筹备创新孵化组",
    model: "deepseek-chat",
    summary: "暂未配置专属职责指南，请点击卡片进入详情进行成员编排与权限划定。",
    membersCount: 0,
    permissionMode: "权限未启用",
    isAssistant: false,
    isBuiltin: false,
  },
];

const mockPresetDepartments = computed(() => mockDepartments.filter((d) => d.isBuiltin));
const mockCustomDepartments = computed(() => mockDepartments.filter((d) => !d.isBuiltin));
const displayedMockDepartments = computed(() =>
  deptCategoryTab.value === "custom" ? mockCustomDepartments.value : mockPresetDepartments.value
);
const deptCategoryOptions = computed(() => [
  { value: "custom" as const, label: "自定义部门", badge: mockCustomDepartments.value.length },
  { value: "preset" as const, label: "系统预设", badge: mockPresetDepartments.value.length },
]);

// 6. 模拟联系人渠道数据
const mockChannels = [
  {
    id: "wecom-bot",
    name: "企业微信应用机器人",
    platformLabel: "企业微信 (WeCom)",
    platformBadge: "企微",
    platformColor: "text-blue-500 bg-blue-500/10 border-blue-500/20",
    enabled: true,
    statusDot: "bg-success",
    statusText: "运行正常 (200)",
    contactsCount: 128,
  },
  {
    id: "feishu-alert",
    name: "飞书研发协作群",
    platformLabel: "飞书 (Feishu)",
    platformBadge: "飞书",
    platformColor: "text-sky-500 bg-sky-500/10 border-sky-500/20",
    enabled: true,
    statusDot: "bg-success",
    statusText: "长连接已建立",
    contactsCount: 46,
  },
  {
    id: "telegram-private",
    name: "个人通知电报机器人",
    platformLabel: "Telegram Bot",
    platformBadge: "TG",
    platformColor: "text-cyan-500 bg-cyan-500/10 border-cyan-500/20",
    enabled: true,
    statusDot: "bg-warning",
    statusText: "正在建立轮询",
    contactsCount: 8,
  },
  {
    id: "dingtalk-ops",
    name: "钉钉运维告警通道",
    platformLabel: "钉钉 (DingTalk)",
    platformBadge: "钉钉",
    platformColor: "text-amber-500 bg-amber-500/10 border-amber-500/20",
    enabled: false,
    statusDot: "bg-neutral",
    statusText: "已停用",
    contactsCount: 0,
  },
];
</script>
