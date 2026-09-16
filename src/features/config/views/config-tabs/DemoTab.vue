<template>
  <div class="grid h-full gap-3 overflow-y-auto pr-1">
    <div class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">组件样式 Demo</h3>
          <p class="text-sm text-base-content/70">通过下拉框切换需要展示的组件样式。</p>
        </div>
        <div class="flex flex-wrap items-center gap-3">
          <select v-model="demoComponentKey" class="select select-bordered select-sm w-64">
            <option value="question">提问卡（ChatQuestionPanel）</option>
            <option value="bubbles">自研气泡</option>
            <option value="delegates">DelegateProgressLine</option>
            <option value="templates">ConfigTemplate</option>
            <option value="overview">监控概览（MonitorOverview）</option>
            <option value="home-cards">预览卡画廊</option>
          <option value="conversation-list">列表卡画廊</option>
            <option value="composer">输入面板新结构</option>
            <option value="double-deck">双层卡（DoubleDeck）</option>
            <option value="thinking-preview">思考预览 bar</option>
            <option value="session-float-dock">会话悬浮操作区</option>
            <option value="session-timeline">会话时间线（垂直）</option>
            <option value="config-cards">配置卡片画廊（六大卡片第一性原理）</option>
            <option value="catalog-store">能力商店</option>
          </select>
          <span class="text-xs text-base-content/50">当前：{{ demoComponentLabel }}</span>
        </div>

        <div v-if="demoComponentKey === 'question'" class="space-y-3 pt-2">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-xs text-base-content/60">预设：</span>
            <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'single' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'single'">shell单行</button>
            <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'singleLong' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'singleLong'">shell 10 行</button>
            <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'ten' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'ten'">patch 10 行</button>
            <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'multi' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'multi'">多条 3 题</button>
            <button type="button" class="btn btn-xs" :class="demoQuestionPreset === 'custom' ? 'btn-primary' : 'btn-ghost'" @click="demoQuestionPreset = 'custom'">多选项</button>
            <button type="button" class="btn btn-xs btn-ghost" @click="resetDemoQuestionAnswers">重置</button>
            <span v-if="demoQuestionLastSubmit" class="text-xs text-success">已提交 {{ demoQuestionLastSubmit.length }} 题</span>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <label class="flex cursor-pointer items-center gap-1.5 text-xs">
              <input type="checkbox" class="checkbox checkbox-xs" v-model="demoWithWorkspace">
              <span>演示目录全权（可记忆工作区）</span>
            </label>
            <span v-if="demoWithWorkspace" class="text-xs text-base-content/50">右上角会出现“目录全权”</span>
          </div>
          <div class="flex justify-center">
            <ChatQuestionPanel
              :key="demoQuestionPreset + String(demoWithWorkspace)"
              :items="demoQuestionItems"
              v-model="demoQuestionAnswers"
              class="flex-1"
              @submit="onDemoQuestionSubmit"
              @approve-for-workspace="onDemoQuestionApproveForWorkspace"
            />
          </div>
          <div v-if="demoQuestionLastWorkspaceApproved" class="alert alert-success py-2 text-xs">
            <span>已演示触发目录全权：{{ demoQuestionLastWorkspaceApproved }}</span>
          </div>
          <div v-if="demoQuestionLastSubmit" class="mockup-code max-h-64 overflow-auto text-xs">
            <pre class="whitespace-pre-wrap break-all"><code>{{ JSON.stringify(demoQuestionLastSubmit, undefined, 2) }}</code></pre>
          </div>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'overview'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">监控概览（MonitorOverview）</h3>
          <p class="text-sm text-base-content/70">一键切换典型状态；卡片跳转与终止事件在 demo 中不生效。</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-xs text-base-content/60">预设：</span>
          <button type="button" class="btn btn-xs" :class="demoOverviewPreset === 'normal' ? 'btn-primary' : 'btn-ghost'" @click="demoOverviewPreset = 'normal'">常规</button>
          <button type="button" class="btn btn-xs" :class="demoOverviewPreset === 'empty' ? 'btn-primary' : 'btn-ghost'" @click="demoOverviewPreset = 'empty'">空闲</button>
          <button type="button" class="btn btn-xs" :class="demoOverviewPreset === 'busy' ? 'btn-primary' : 'btn-ghost'" @click="demoOverviewPreset = 'busy'">高负载</button>
          <button type="button" class="btn btn-xs" :class="demoOverviewPreset === 'backgroundOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoOverviewPreset = 'backgroundOnly'">仅后台任务</button>
        </div>
        <div class="flex justify-center">
          <div class="h-[440px] w-[400px] overflow-hidden rounded-lg border border-base-300 bg-base-100 shadow-sm">
            <MonitorOverview
              class="h-full w-full"
              :delegate-statuses="demoOverviewDelegates"
              :running-tasks="demoOverviewTasks"
              :background-shells="demoOverviewBackgroundShells"
              :latest-batches="demoOverviewBatches"
            />
          </div>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'composer'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">输入面板新结构</h3>
          <p class="text-sm text-base-content/70">矩形容器包裹，底色 base100→base200→base300 循环切换；灌满 ↔ 悬浮 一键切换。</p>
        </div>
        <ChatComposerStructureDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'double-deck'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">双层卡（DoubleDeck）</h3>
          <p class="text-sm text-base-content/70">底卡包面卡 · 露头在面卡上方 · [[abc]]空底座 / [[abc]def]多队列 · 先调顺再回输入面板。</p>
        </div>
        <DoubleDeckCardDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'thinking-preview'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">思考预览 bar（ChatThinkingPreviewBar）</h3>
          <p class="text-sm text-base-content/70">输入框上方的思维链预览：毛玻璃底、节流切换、显示末尾四行并带上/下位移动画。</p>
        </div>
        <ChatThinkingPreviewBarDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'session-float-dock'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">会话悬浮操作区（下排独立按钮）</h3>
          <p class="text-sm text-base-content/70">下排改成一个个彼此独立的磨砂元素，左对齐、放不下换行；元素之间不互斥，有就显示、没有就隐藏。开关可逐项试「有／没有」，勾「窄容器」看换行。</p>
        </div>
        <SessionFloatDockDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'session-timeline'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">会话时间线（垂直）</h3>
          <p class="text-sm text-base-content/70">
            时间线按钮悬停拉成胶囊，点击展开一块几乎盖满聊天区的垂直时间线；用 daisyUI 的 <code>timeline timeline-vertical</code> 逐轮铺开历史消息，内部可上下滚动。
          </p>
        </div>
        <SessionTimelineDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'config-cards'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">配置卡片画廊（六大卡片第一性原理）</h3>
          <p class="text-sm text-base-content/70">集中渲染联系人渠道、部门、连接器、供应商、人格、技能 6 类卡片的真实模拟数据与自适应网格，支持切换宽度与主题进行视觉检验。</p>
        </div>
        <ConfigCardsGalleryDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'catalog-store'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">能力商店</h3>
          <p class="text-sm text-base-content/70">模拟数据。搜索跨源聚合，一条一行，只留「这是什么」和「一个动作」。</p>
        </div>
        <CatalogStoreDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'home-cards'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">预览卡画廊</h3>
          <p class="text-sm text-base-content/70">右侧主页的每张卡单独展示一次，面板宽度按聊天窗口右侧还原；模拟数据只用于看样式。</p>
        </div>
        <HomeCardGalleryDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'conversation-list'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">列表卡画廊</h3>
          <p class="text-sm text-base-content/70">左侧会话列表条目（ChatConversationItem）的分组与状态样本，直接渲染真实组件；切换视图模式与预设查看不同层级、不同状态。</p>
        </div>
        <ChatConversationListGalleryDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'templates'" class="grid gap-3">
      <ConfigTemplate v-model="configTemplateDemo" :groups="configTemplateGroups" />
    </div>

    <div v-if="demoComponentKey === 'templates'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">{{ t("config.demo.nativeNotificationTitle") }}</h3>
          <p class="text-sm text-base-content/70">
            {{ t("config.demo.nativeNotificationSummary") }}
          </p>
          <p class="text-xs text-base-content/60">
            {{ t("config.demo.nativeNotificationDevHint") }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-primary"
            :disabled="sending"
            @click="sendNativeNotification"
          >
            {{ sending ? t("config.demo.sending") : t("config.demo.sendNativeNotification") }}
          </button>
          <span class="text-xs text-base-content/60">{{ t("config.demo.backgroundHint") }}</span>
        </div>

        <div v-if="errorText" class="alert alert-error text-sm">
          <span>{{ errorText }}</span>
        </div>

        <div v-else-if="resultText" class="alert alert-success text-sm whitespace-pre-wrap">
          <span>{{ resultText }}</span>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'templates'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">{{ t("config.demo.restartTitle") }}</h3>
          <p class="text-sm text-base-content/70">
            {{ t("config.demo.restartSummary") }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-warning"
            :disabled="restarting"
            @click="restartApp"
          >
            <RotateCcw class="size-4" aria-hidden="true" />
            {{ restarting ? t("config.demo.restarting") : t("config.demo.restartApp") }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'templates'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">后端内存快照</h3>
          <p class="text-sm text-base-content/70">
            调用后端调试命令，查看会话缓存、message_store 缓存和其他长生命周期状态的占用概况。
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            type="button"
            class="btn btn-secondary"
            :disabled="loadingMemoryStats"
            @click="loadMemoryStats"
          >
            {{ loadingMemoryStats ? "查询中..." : "查询后端内存" }}
          </button>
        </div>

        <div v-if="memoryStatsText" class="mockup-code max-h-96 overflow-auto text-xs">
          <pre class="whitespace-pre-wrap break-all"><code>{{ memoryStatsText }}</code></pre>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'delegates'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">DelegateProgressLine 预览</h3>
          <p class="text-sm text-base-content/70">折叠卡片第二行的实时进度组件样本。</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-xs text-base-content/60">预设：</span>
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'normal' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'normal'">仅委托</button>
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'tasksOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'tasksOnly'">仅任务</button>
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'shellsOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'shellsOnly'">仅后台终端</button>
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'all' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'all'">全部运行</button>
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'empty' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'empty'">空闲</button>
        </div>
        <div class="flex w-full max-w-2xl flex-wrap items-center gap-2">
          <SessionControlItems
            workspace-button-label="工作空间"
            workspace-button-name="easy_call_ai"
            workspace-work-mode="worktree"
            :delegates="demoDelegateStatuses"
            :running-task-count="demoRunningTaskCount"
            :running-shell-count="demoRunningShellCount"
          />
        </div>
        <div class="flex flex-col gap-1 py-2">
          <DelegateCard
            title="示例：代码审查（pending）"
            :running="true"
            :elapsed-ms="demoDelegateStatuses[0]?.elapsedMs"
            :request-count="demoDelegateStatuses[0]?.requestCount"
            :token-count="demoDelegateStatuses[0]?.tokenCount"
            last-tool-name="apply_patch"
          />
          <DelegateCard
            title="示例：委托任务（运行中）"
            :running="true"
            :elapsed-ms="demoDelegateStatuses[1]?.elapsedMs"
            :request-count="demoDelegateStatuses[1]?.requestCount"
            :token-count="demoDelegateStatuses[1]?.tokenCount"
            last-tool-name="shell_exec"
          />
          <DelegateCard
            title="示例：审查报告（完成）"
            text="整体判定：正确，置信度 0.92"
          />
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'bubbles'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">自研气泡组件 Demo</h3>
          <p class="text-sm text-base-content/70">
            不使用 DaisyUI 原生 chat/chat-bubble。助理消息无气泡，用户消息使用半透明圆角矩形气泡，单独验证多助理、左右布局、头像避让和名称对齐。
          </p>
        </div>

        <div class="overflow-hidden rounded-[1.75rem] border border-base-300/80 bg-base-200/70 p-4 shadow-inner">
          <div class="flex flex-col gap-4 rounded-[1.35rem] border border-base-100/70 bg-base-100/30 px-3 py-4">
            <ChatBubbleShell
              v-for="message in bubbleDemoDisplayMessages"
              :key="message.id"
              :tone="message.tone"
              :name="message.name"
              :meta="message.meta"
              :avatar-url="message.avatarUrl"
              :streaming="!!message.streaming"
              :separated="message.separated"
            >
              <template v-if="message.reasoning?.length || message.tools?.length" #activity>
                <div class="flex flex-col opacity-90">
                  <div class="flex min-w-0 flex-wrap items-center gap-1.5 py-0.5 text-sm font-normal text-base-content/42">
                    <span class="shrink-0">思考与工具</span>
                    <span v-if="message.tools?.length" class="inline-flex h-3 items-center text-base-content/30">·</span>
                    <span v-if="message.tools?.length" class="min-w-0 truncate text-base-content/42">
                      {{ message.tools.length }} 个工具
                    </span>
                  </div>
                  <div class="flex flex-col pt-1 text-xs text-base-content/70">
                    <div
                      v-if="message.reasoning?.length"
                      class="flex gap-1.5 border-l border-base-content/15 py-1 pr-1 pl-2"
                    >
                      <span class="inline-flex w-3 shrink-0 items-center justify-center font-mono text-xs leading-none text-info">•</span>
                      <ExpandableText
                        class="min-w-0 flex-1"
                        :text="message.reasoning.join('\n')"
                        text-class="text-base-content/70"
                      />
                    </div>
                    <details
                      v-for="(tool, toolIndex) in message.tools || []"
                      :key="`${tool.name}-${toolIndex}`"
                      class="collapse rounded-none border-l border-base-content/15 pl-2"
                      :open="bubbleDemoActivityItemOpen(message.id, bubbleDemoToolItemKey(tool, toolIndex))"
                      @toggle="onBubbleDemoActivityItemToggle(message.id, bubbleDemoToolItemKey(tool, toolIndex), $event)"
                    >
                      <summary class="collapse-title flex min-h-0 items-center gap-1.5 px-1 py-1 text-xs hover:bg-base-200">
                        <span class="inline-flex w-3 shrink-0 items-center justify-center leading-none text-success">
                          <FileText v-if="tool.icon === 'file'" class="size-3" aria-hidden="true" />
                          <SquareTerminal v-else-if="tool.icon === 'terminal'" class="size-3" aria-hidden="true" />
                          <Wrench v-else class="size-3" aria-hidden="true" />
                        </span>
                        <span class="min-w-0 flex-1 truncate text-base-content/75">
                          {{ tool.name }}
                          <span class="ml-1 text-success">✓</span>
                        </span>
                      </summary>
                      <div v-if="bubbleDemoActivityItemOpen(message.id, bubbleDemoToolItemKey(tool, toolIndex))" class="collapse-content px-1 pb-2 pt-1">
                        <div class="whitespace-pre-wrap wrap-break-word text-xs leading-relaxed text-base-content/70">
                          {{ tool.detail }}
                        </div>
                      </div>
                    </details>
                  </div>
                </div>
              </template>
              <div class="space-y-2 text-sm leading-relaxed">
                <p v-for="line in message.lines" :key="line" class="m-0 whitespace-pre-wrap break-words">
                  {{ line }}
                </p>
                <div
                  v-if="message.chips?.length"
                  :class="[
                    'flex flex-wrap gap-1.5 pt-1',
                    message.tone === 'user' ? 'justify-end' : 'justify-start',
                  ]"
                >
                  <span
                    v-for="chip in message.chips"
                    :key="chip"
                    class="rounded-lg bg-base-100/35 px-2 py-1 text-xs text-base-content/60 backdrop-blur-sm"
                  >
                    {{ chip }}
                  </span>
                </div>
                <ChatAttachmentList
                  v-if="message.attachments?.length"
                  :attachments="message.attachments"
                  :align="message.tone === 'user' ? 'end' : 'start'"
                  :interactive-kinds="['audio']"
                  :playing-id="playingBubbleDemoAudioId"
                  @activate="handleBubbleDemoAttachmentActivate"
                />
              </div>
              <template #footer>
                <span v-if="message.footer">{{ message.footer }}</span>
                <span v-if="!message.streaming" class="inline-flex items-center gap-1">
                  <Copy class="size-3" aria-hidden="true" />
                  复制
                </span>
                <span v-if="!message.streaming && message.canRecall" class="inline-flex items-center gap-1">
                  <Undo2 class="size-3" aria-hidden="true" />
                  撤回
                </span>
              </template>
            </ChatBubbleShell>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Copy, FileText, RotateCcw, SquareTerminal, Undo2, Wrench } from "@lucide/vue";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  invokeTauri,
  restartTransportApplicationDemo,
  sendTransportNativeNotificationDemo,
} from "../../../../services/tauri-api";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import ChatQuestionPanel from "../../../chat/components/ChatQuestionPanel.vue";
import type { QuestionItem as ChatQuestionItem } from "../../../chat/components/ChatQuestionPanel.vue";
import ChatBubbleShell from "../../../chat/components/ChatBubbleShell.vue";
import ExpandableText from "../../../shared/components/ExpandableText.vue";
import ChatAttachmentList from "../../../chat/components/ChatAttachmentList.vue";
import DelegateCard from "../../../chat/components/DelegateCard.vue";
import MonitorOverview from "../../../chat/components/MonitorOverview.vue";
import ChatComposerStructureDemo from "../../../chat/components/ChatComposerStructureDemo.vue";
import DoubleDeckCardDemo from "../../../chat/components/DoubleDeckCardDemo.vue";
import ChatThinkingPreviewBarDemo from "../../../chat/components/ChatThinkingPreviewBarDemo.vue";
import SessionFloatDockDemo from "../../../chat/components/SessionFloatDockDemo.vue";
import SessionTimelineDemo from "../../../chat/components/SessionTimelineDemo.vue";
import HomeCardGalleryDemo from "../../../chat/components/HomeCardGalleryDemo.vue";
import ChatConversationListGalleryDemo from "../../../chat/components/ChatConversationListGalleryDemo.vue";
import ConfigCardsGalleryDemo from "../../components/ConfigCardsGalleryDemo.vue";
import CatalogStoreDemo from "../../components/CatalogStoreDemo.vue";
import SessionControlItems from "../../../chat/components/SessionControlItems.vue";
import type { AppConfig, BackgroundShellTaskSummary, ConversationDelegateStatusSummary, PersonaProfile } from "../../../../types/app";
import type { ToolReviewBatchSummary } from "../../../chat/composables/use-chat-tool-review";
import type { TaskEntry } from "./task-editor";
import type { ChatAttachmentView } from "../../../chat/utils/chat-attachment-display";

type NativeNotificationDemoResult = {
  permissionBefore: string;
  permissionAfter: string;
  title: string;
  body: string;
  sentAt: string;
};

type BubbleDemoMessage = {
  id: string;
  tone: "assistant" | "user" | "system";
  personaSlot: "assistant-primary" | "assistant-reviewer" | "assistant-memory" | "assistant-system" | "user";
  name: string;
  meta: string;
  reasoning?: string[];
  tools?: Array<{
    name: string;
    detail: string;
    icon: "file" | "terminal" | "tool";
  }>;
  lines: string[];
  chips?: string[];
  attachments?: ChatAttachmentView[];
  footer?: string;
  canRecall?: boolean;
  streaming?: boolean;
};

type BubbleDemoTool = NonNullable<BubbleDemoMessage["tools"]>[number];

type BubbleDemoDisplayMessage = BubbleDemoMessage & {
  avatarUrl: string;
  separated: boolean;
};

const props = withDefaults(defineProps<{
  config?: AppConfig;
  personas?: PersonaProfile[];
  personaAvatarUrlMap?: Record<string, string>;
  assistantDepartmentAgentId?: string;
  initialKey?: string;
}>(), {
  config: () => ({} as AppConfig),
  personas: () => [],
  personaAvatarUrlMap: () => ({}),
  assistantDepartmentAgentId: "",
  initialKey: "question",
});

const sending = ref(false);
const restarting = ref(false);

const loadingMemoryStats = ref(false);
const errorText = ref("");
const resultText = ref("");
const memoryStatsText = ref("");
const configTemplateDemo = ref<Record<string, unknown>>({
  openLocalDocument: true,
  openOnlineDocument: true,
  autoHideAtScreenEdge: true,
  showTaskbarIcon: true,
  closeApp: "tray",
  openWebInSystemBrowser: true,
  homepage: "https://pai.example.com",
  browserNote: "",
});
const demoComponentKey = ref<"question" | "bubbles" | "delegates" | "templates" | "overview" | "home-cards" | "conversation-list" | "composer" | "double-deck" | "thinking-preview" | "session-float-dock" | "session-timeline" | "config-cards" | "catalog-store">((props.initialKey as any) || "question");
const demoComponentLabel = computed(() => {
  if (demoComponentKey.value === "catalog-store") return "能力商店";
  if (demoComponentKey.value === "config-cards") return "配置卡片画廊";
  if (demoComponentKey.value === "session-float-dock") return "会话悬浮操作区";
  if (demoComponentKey.value === "session-timeline") return "会话时间线（垂直）";
  if (demoComponentKey.value === "thinking-preview") return "思考预览 bar";
  if (demoComponentKey.value === "double-deck") return "双层卡";
  if (demoComponentKey.value === "home-cards") return "预览卡画廊";
  if (demoComponentKey.value === "conversation-list") return "列表卡画廊";
  if (demoComponentKey.value === "composer") return "输入面板新结构";
  if (demoComponentKey.value === "question") return "提问卡";
  if (demoComponentKey.value === "bubbles") return "自研气泡";
  if (demoComponentKey.value === "delegates") return "DelegateProgressLine";
  if (demoComponentKey.value === "overview") return "监控概览（MonitorOverview）";
  return "ConfigTemplate";
});

const demoOverviewPreset = ref<"empty" | "normal" | "busy" | "backgroundOnly">("normal");

function makeDemoOverviewDelegate(index: number, status: string): ConversationDelegateStatusSummary {
  const titles = ["整理会话记录并归档", "排查后台任务页加载失败", "梳理委托状态轮询调用链"];
  const tools = ["exec", "read", "rg"];
  return {
    delegateId: `demo-delegate-${index}`,
    kind: "delegate",
    conversationId: "demo",
    rootConversationId: "demo",
    title: titles[index % titles.length],
    status,
    active: true,
    startedAt: new Date(Date.now() - 62000 * (index + 1)).toISOString(),
    updatedAt: new Date().toISOString(),
    elapsedMs: 62000 * (index + 1),
    requestCount: 4 + index * 8,
    toolCallCount: 9 + index * 25,
    lastToolName: tools[index % tools.length],
    tokenCount: 12800 + index * 16200,
  };
}

function makeDemoOverviewTask(index: number): TaskEntry {
  const goals = [
    "每周整理一次 changelog 未发布条目",
    "补齐后台 shell 终止的回归测试",
    "梳理监控面板 tab 的状态链路",
    "清理未使用的组件导入",
    "核对三语言包缺失的 key",
  ];
  return {
    taskId: `demo-task-${index}`,
    orderIndex: index,
    goal: goals[index % goals.length],
    why: "保持发布流程可追溯",
    todo: "检查清单并逐项归类",
    completionState: "active",
    completionConclusion: "",
    progressNotes: [],
    trigger: index < 3
      ? { next_run_at: new Date(Date.now() + (index === 0 ? 5 : index === 1 ? 125 : 1490) * 60000).toISOString() }
      : {},
    createdAtLocal: new Date().toISOString(),
    updatedAtLocal: new Date().toISOString(),
  };
}

function makeDemoOverviewShell(index: number, outputTail: string): BackgroundShellTaskSummary {
  const descriptions = ["前端 dev server 预热", "批量截图回归任务", "Rust 增量编译检查"];
  const commands = ["pnpm dev", "pnpm smoke", "cargo check"];
  return {
    id: `demo-shell-${index}`,
    kind: "shell",
    status: "running",
    exitCode: null,
    description: descriptions[index % descriptions.length],
    command: commands[index % commands.length],
    cwd: "E:/github/easy_call_ai",
    startedAt: new Date(Date.now() - 124000 * (index + 1)).toISOString(),
    timeoutMs: null,
    log: `C:/demo/bg-shell-demo-${index}.log`,
    outputTail,
  };
}

const demoOverviewDelegates = computed<ConversationDelegateStatusSummary[]>(() => {
  const preset = demoOverviewPreset.value;
  if (preset === "empty" || preset === "backgroundOnly") return [];
  if (preset === "busy") return [0, 1, 2].map((index) => makeDemoOverviewDelegate(index, index === 2 ? "delivered" : "running"));
  return [makeDemoOverviewDelegate(0, "running"), makeDemoOverviewDelegate(1, "delivered")];
});

const demoOverviewTasks = computed<TaskEntry[]>(() => {
  const preset = demoOverviewPreset.value;
  if (preset === "empty" || preset === "backgroundOnly") return [];
  if (preset === "busy") return [0, 1, 2, 3, 4].map((index) => makeDemoOverviewTask(index));
  return [makeDemoOverviewTask(0)];
});

const demoOverviewBackgroundShells = computed<BackgroundShellTaskSummary[]>(() => {
  const preset = demoOverviewPreset.value;
  if (preset === "empty") return [];
  if (preset === "busy") {
    return [
      makeDemoOverviewShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/"),
      makeDemoOverviewShell(1, "[smoke] 12/24 用例通过，正在执行终端联动用例…"),
      makeDemoOverviewShell(2, "Checking the core runtime... 412 crates checked.\nwarning: unused import: `serde::Serialize`\n= note: `#[warn(unused_imports)]` on part of module `commands`"),
    ];
  }
  if (preset === "backgroundOnly") {
    return [
      makeDemoOverviewShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/"),
      makeDemoOverviewShell(1, "3 分钟计时任务剩余 01:42…"),
    ];
  }
  return [makeDemoOverviewShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/")];
});

const demoOverviewBatches = computed<ToolReviewBatchSummary[]>(() => {
  const preset = demoOverviewPreset.value;
  if (preset === "empty" || preset === "backgroundOnly") return [];
  if (preset === "busy") {
    return [
      {
        batchKey: "demo-batch-busy-1",
        userMessageId: "demo-message-1",
        userMessageText: "重构监控面板并提取概览组件，同步更新三语言包文案",
        itemCount: 14,
        unreviewedCount: 9,
        changedFiles: 6,
        addedLines: 218,
        deletedLines: 34,
        items: [],
      },
      {
        batchKey: "demo-batch-busy-2",
        userMessageId: "demo-message-2",
        userMessageText: "后台任务页接通 monitor.changed 脏标记事件",
        itemCount: 7,
        unreviewedCount: 2,
        changedFiles: 3,
        addedLines: 64,
        deletedLines: 41,
        items: [],
      },
      {
        batchKey: "demo-batch-busy-3",
        userMessageId: "demo-message-3",
        userMessageText: "修复 DeepSeek 协议下思考开关不生效的问题",
        itemCount: 4,
        unreviewedCount: 0,
        changedFiles: 1,
        addedLines: 12,
        deletedLines: 9,
        items: [],
      },
    ];
  }
  return [
    {
      batchKey: "demo-batch",
      userMessageId: "demo-message",
      userMessageText: "修复后台任务页打不开的问题并补充回归测试",
      itemCount: 8,
      unreviewedCount: 3,
      changedFiles: 4,
      addedLines: 96,
      deletedLines: 12,
      items: [],
    },
  ];
});
const demoQuestionPreset = ref<"single" | "singleLong" | "multi" | "ten" | "custom">("single");
const demoWithWorkspace = ref(true);
const demoQuestionLastWorkspaceApproved = ref("");
const demoQuestionAnswers = ref<Record<string, { optionId: string; label: string; comment: string }>>({});
const demoQuestionLastSubmit = ref<Array<{ id: string; optionId: string; label: string; comment: string }> | null>(null);
const demoQuestionItems = computed<ChatQuestionItem[]>(() => {
  if (demoQuestionPreset.value === "single") {
    return [
      {
        id: "q-single",
        title: "是否允许执行此命令？",
        description: "单行 shell 命令示例",
        previewText: "$ pnpm exec eslint src --ext .ts,.tsx --max-warnings 0 --format stylish --cache --cache-location .eslintcache",
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "singleLong") {
    const tenShell = [
      "$ pnpm exec eslint src --ext .ts,.tsx --max-warnings 0",
      "$ pnpm typecheck",
      "$ pnpm build",
      "$ cargo check --manifest-path src-tauri/Cargo.toml",
      "$ cargo test --manifest-path src-tauri/Cargo.toml",
      "$ pnpm exec vitest run --reporter=basic",
      "$ git status --short",
      "$ git diff --stat",
      "$ pnpm changelog:build",
      "$ pnpm tauri build --debug",
    ].join("\n");
    return [
      {
        id: "q-single-long",
        title: "是否允许执行这组命令？（10 行 shell）",
        description: "10 行 shell 命令，超过 6 行应在内容区内滚动",
        previewText: tenShell,
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "custom") {
    return [
      {
        id: "q-custom",
        title: "请选择处理方式",
        description: "演示多行选项架构：直接选项为纯按钮，补充选项为输入框+按钮联体。",
        previewText: "你可以扩展任意数量的选项，AI提问也能复用同一张卡。",
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "direct-a", label: "直接同意", kind: "direct" },
          { id: "with-a", label: "同意并补充", kind: "withInput", placeholder: "补充说明（可选）" },
          { id: "with-b", label: "拒绝并说明原因", kind: "withInput", placeholder: "请填写拒绝原因（必填）", inputRequired: true },
        ],
      },
    ];
  }
  if (demoQuestionPreset.value === "ten") {
    const tenPatch = [
      "*** Begin Patch",
      "*** Update File: src/features/chat/components/ChatQuestionPanel.vue",
      "@@ -1,10 +1,15 @@",
      ...Array.from({ length: 10 }, (_, i) => `- 旧行 ${i + 1}: const a${i} = ${i}`),
      ...Array.from({ length: 10 }, (_, i) => `+ 新行 ${i + 1}: const b${i} = ${i * 2}`),
      "*** End Patch",
    ].join("\n");
    return [
      {
        id: "q-ten",
        title: "10 行 diff 演示",
        description: "超过 6 行的 diff，应在内容区内滚动并保留红绿高亮。",
        previewText: tenPatch,
        canRememberWorkspace: demoWithWorkspace.value || undefined,
        workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
        options: [
          { id: "approve", label: "同意", kind: "direct" },
          { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
        ],
      },
    ];
  }
  return [
    {
      id: "q-1",
      title: "是否允许改写 src/App.vue ?",
      description: "删除 12 行，新增 8 行",
      previewText: "*** Begin Patch\n*** Update File: src/App.vue\n@@\n- 旧行\n+ 新行",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
    {
      id: "q-2",
      title: "是否允许删除 src/old.ts ?",
      description: "该文件已无引用，删除后需回归 typecheck。",
      previewText: "DELETE src/old.ts (42 行)",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
    {
      id: "q-3",
      title: "是否允许新增 src/utils/util.ts ?",
      description: "新增通用工具，约 60 行。",
      previewText: "*** Add File: src/utils/util.ts\n+ export function helper() {}",
      canRememberWorkspace: demoWithWorkspace.value || undefined,
      workspaceLabel: demoWithWorkspace.value ? "easy_call_ai" : undefined,
      options: [
        { id: "approve", label: "同意", kind: "direct" },
        { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
      ],
    },
  ];
});

function resetDemoQuestionAnswers() {
  demoQuestionAnswers.value = {};
  demoQuestionLastSubmit.value = null;
  demoQuestionLastWorkspaceApproved.value = "";
}

function onDemoQuestionSubmit(payload: Array<{ id: string; optionId: string; label: string; comment: string }>) {
  demoQuestionLastSubmit.value = payload;
}

function onDemoQuestionApproveForWorkspace(requestId: string) {
  demoQuestionLastWorkspaceApproved.value = String(requestId || "").trim() || "unknown";
}

watch(demoQuestionPreset, () => {
  resetDemoQuestionAnswers();
});
watch(demoWithWorkspace, () => {
  resetDemoQuestionAnswers();
});

const bubbleDemoActivityItemOpenKey = ref("");
const playingBubbleDemoAudioId = ref("");
const { t } = useI18n();

const bubbleDemoAttachmentImage = `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" width="520" height="300" viewBox="0 0 520 300"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop stop-color="#dbeafe"/><stop offset="1" stop-color="#c4b5fd"/></linearGradient></defs><rect width="520" height="300" rx="24" fill="url(#g)"/><rect x="38" y="38" width="444" height="224" rx="16" fill="#111827" fill-opacity=".82"/><path d="M72 92h156M72 126h260M72 160h208M72 194h300" stroke="#bfdbfe" stroke-linecap="round" stroke-width="12"/><circle cx="418" cy="104" r="28" fill="#a7f3d0" fill-opacity=".9"/><text x="72" y="238" fill="#e0e7ff" font-family="Segoe UI, sans-serif" font-size="18">screenshot.png</text></svg>',
 )}`;
const configTemplateGroups: ConfigTemplateGroup[] = [
  {
    title: t("config.demo.generalSection"),
    rows: [
      {
        items: [
          {
            key: "openLocalDocument",
            label: t("config.demo.openLocalDocument"),
            description: t("config.demo.openLocalDocumentHint"),
            type: "toggle",
          },
        ],
      },
      {
        items: [
          {
            key: "openOnlineDocument",
            label: t("config.demo.openOnlineDocument"),
            type: "toggle",
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.mainPanelSection"),
    rows: [
      {
        items: [
          { key: "autoHideAtScreenEdge", label: t("config.demo.autoHideAtScreenEdge"), type: "toggle" },
          { key: "showTaskbarIcon", label: t("config.demo.showTaskbarIcon"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "closeApp",
            label: t("config.demo.closeApp"),
            type: "select",
            options: [
              { value: "tray", label: t("config.demo.closeAppToTray") },
              { value: "quit", label: t("config.demo.closeAppQuit") },
            ],
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.browseSection"),
    rows: [
      {
        items: [
          { key: "openWebInSystemBrowser", label: t("config.demo.openWebInSystemBrowser"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "homepage",
            label: t("config.demo.homepage"),
            type: "text",
            placeholder: t("config.demo.homepagePlaceholder"),
          },
          {
            key: "browserNote",
            label: t("config.demo.browserNote"),
            type: "textarea",
            placeholder: t("config.demo.browserNotePlaceholder"),
          },
        ],
      },
    ],
  },
];
const bubbleDemoMessages: BubbleDemoMessage[] = [
  {
    id: "assistant-opening",
    tone: "assistant",
    personaSlot: "assistant-primary",
    name: "Pai",
    meta: "刚刚",
    reasoning: [
      "**Considering layout migration** The user wants to replace DaisyUI chat with a lighter custom message shell.",
      "Need preserve current activity presentation while separating assistant content from user bubbles.",
      "Only the user side should keep a translucent rounded rectangle bubble.",
    ],
    tools: [
      { name: "exec", detail: 'rg -n "chat-bubble|activityPanel|thinkingAndTools" src/features/chat', icon: "terminal" },
      { name: "exec", detail: 'rg -n "ChatMessageItem|DemoTab|ChatBubbleShell" src/features', icon: "terminal" },
    ],
    lines: [
      "我把名字、气泡和底部操作都放在同一列里，头像永远只占头像列。",
      "这样左侧消息不会被头像压到下面，右侧消息也能自然贴齐内容列。",
    ],
    chips: ["半透明背景", "无气泡尾巴", "名称对齐"],
    canRecall: true,
  },
  {
    id: "reviewer-assistant",
    tone: "assistant",
    personaSlot: "assistant-reviewer",
    name: "代码审查员",
    meta: "刚刚",
    reasoning: [
      "**Reviewing assistant identity** Multiple assistant speakers should stay visually distinct without adding bubbles.",
      "The content column must align name, activity, final answer, and footer under the same speaker identity.",
    ],
    tools: [
      { name: "exec", detail: 'rg -n "speakerAgentId|personaNameMap|activityItems" src/features/chat', icon: "terminal" },
      { name: "exec", detail: 'Get-Content src/features/chat/components/ChatMessageItem.vue | Select-Object -First 220', icon: "terminal" },
    ],
    lines: [
      "这里是另一个助理身份。左侧消息没有气泡底，只保留头像、名称、内容和工具状态。",
      "如果多个助理连续发言，视觉上应该靠头像和名称区分，而不是靠不同气泡颜色。",
    ],
    chips: ["review", "layout"],
    canRecall: true,
  },
  {
    id: "user-reply",
    tone: "user",
    personaSlot: "user",
    name: "我",
    meta: "1 分钟前",
    lines: [
      "右侧也保持同一套结构：头像在右，名称和气泡右对齐，不再依赖 DaisyUI chat 的网格。",
    ],
    attachments: [
      { kind: "file", label: "screenshot.png" },
      { kind: "file", label: "design-notes.md" },
    ],
    canRecall: true,
  },
  {
    id: "user-all-attachments",
    tone: "user",
    personaSlot: "user",
    name: "我",
    meta: "刚刚",
    lines: ["这是一条全附件气泡，用来观察不同附件内容放在一起时的层级。"],
    attachments: [
      { kind: "image", label: "screenshot.png", src: bubbleDemoAttachmentImage },
      { id: "demo-meeting-note", kind: "audio", label: "meeting-note.m4a", detail: "0:18" },
      { kind: "file", label: "design-notes.md" },
      { kind: "context", label: "ChatMessageItem.vue" },
      { kind: "text", label: "保持附件标签轻量，避免重复说明。" },
    ],
    canRecall: true,
  },
  {
    id: "memory-assistant",
    tone: "assistant",
    personaSlot: "assistant-memory",
    name: "记忆管家",
    meta: "1 分钟前",
    tools: [
      { name: "exec", detail: 'rg -n "ChatBubbleShell|message footer" src/features/chat', icon: "terminal" },
    ],
    lines: [
      "我补一条更短的助理消息，观察无气泡状态下短文本是否过于飘。",
      "如果太轻，可以后续给助理内容区加极淡的左侧引导线，而不是恢复气泡。",
    ],
    chips: ["memory: UI preference", "命中 3"],
    canRecall: true,
  },
  {
    id: "assistant-long",
    tone: "system",
    personaSlot: "assistant-system",
    name: "系统人格",
    meta: "2 分钟前",
    reasoning: [
      "**Preparing demo patch** Keep the current activity visual language, but make sample tool content match real runtime traces.",
      "The final answer below should remain independent from activity, so future rendering can lazy-load details.",
    ],
    tools: [
      { name: "apply_patch", detail: "更新 DemoTab.vue 的自研气泡样张内容", icon: "tool" },
      { name: "exec", detail: "pnpm typecheck", icon: "terminal" },
    ],
    lines: [
      "长内容会限制在内容列最大宽度内，背景是圆角矩形半透明层，适合后续接 Markdown、工具卡片和附件。",
      "这个 demo 先只验证外壳：左右、头像、名称、气泡、footer。确认方向后，再迁移到正式 ChatMessageItem。",
    ],
    chips: ["未来兼容: assistant-attachment.md"],
    streaming: true,
  },
];
const userPersona = computed(
  () => props.personas.find((persona) => persona.isBuiltInUser || persona.id === "user-persona") ?? null,
);
const assistantPersonas = computed(() =>
  props.personas.filter((persona) =>
    !persona.isBuiltInUser
    && !persona.isBuiltInSystem
    && persona.id !== "user-persona"
    && persona.id !== "system-persona",
  ),
);
const primaryAssistantPersona = computed(
  () =>
    assistantPersonas.value.find((persona) => persona.id === props.assistantDepartmentAgentId)
    ?? assistantPersonas.value[0]
    ?? null,
);
const bubbleDemoDisplayMessages = computed<BubbleDemoDisplayMessage[]>(() =>
  bubbleDemoMessages.map((message, index) => {
    const persona = bubbleDemoPersona(message.personaSlot);
    const name = String(persona?.name || message.name).trim() || message.name;
    const previous = bubbleDemoMessages[index - 1];
    return {
      ...message,
      name,
      avatarUrl: String((persona?.id ? props.personaAvatarUrlMap[persona.id] : "") || "").trim(),
      separated: !!previous && message.tone !== "user" && previous.tone !== "user",
    };
  }),
);
const demoDelegateStatuses = ref<ConversationDelegateStatusSummary[]>([
  createDemoDelegateStatus("demo-code-review", "示例：代码审查（pending）", 45000, 12, 15600, "apply_patch"),
  createDemoDelegateStatus("demo-research", "示例：委托任务（运行中）", 120000, 34, 52800, "shell_exec"),
  {
    ...createDemoDelegateStatus("demo-report", "示例：审查报告（完成）", 347000, 18, 23600, ""),
    status: "completed",
    active: false,
    completedAt: new Date().toISOString(),
  },
]);
let delegateDemoTimer = 0;
const demoMonitorPreset = ref<"normal" | "tasksOnly" | "shellsOnly" | "all" | "empty">("normal");
const demoHasRunningDelegates = computed(() => demoMonitorPreset.value === "normal" || demoMonitorPreset.value === "all");
const demoRunningTaskCount = computed(() => (demoMonitorPreset.value === "tasksOnly" || demoMonitorPreset.value === "all") ? 2 : 0);
const demoRunningShellCount = computed(() => (demoMonitorPreset.value === "shellsOnly" || demoMonitorPreset.value === "all") ? 3 : 0);

function bubbleDemoPersona(slot: BubbleDemoMessage["personaSlot"]): PersonaProfile | null {
  if (slot === "user") return userPersona.value;
  const assistants = assistantPersonas.value;
  if (assistants.length === 0) return null;
  const primary = primaryAssistantPersona.value ?? assistants[0] ?? null;
  const alternates = assistants.filter((persona) => persona.id !== primary?.id);
  if (slot === "assistant-primary") return primary;
  if (slot === "assistant-reviewer") return alternates[0] ?? assistants[1] ?? primary;
  if (slot === "assistant-memory") return alternates[1] ?? assistants[2] ?? primary;
  return alternates[2] ?? assistants[3] ?? primary;
}

function detailsOpenFromEvent(event: Event): boolean {
  const target = event.target;
  return target instanceof HTMLDetailsElement ? target.open : false;
}

function bubbleDemoActivityItemKey(messageId: string, itemKey: string): string {
  return `${messageId}:${itemKey}`;
}

function bubbleDemoActivityItemOpen(messageId: string, itemKey: string): boolean {
  return bubbleDemoActivityItemOpenKey.value === bubbleDemoActivityItemKey(messageId, itemKey);
}

function bubbleDemoToolItemKey(tool: BubbleDemoTool, index: number): string {
  return `tool:${index}:${String(tool.name || "").trim()}`;
}

function toggleBubbleDemoAudio(attachmentId: string): void {
  playingBubbleDemoAudioId.value = playingBubbleDemoAudioId.value === attachmentId ? "" : attachmentId;
}

function handleBubbleDemoAttachmentActivate(payload: { attachment: ChatAttachmentView }): void {
  if (payload.attachment.kind !== "audio" || !payload.attachment.id) return;
  toggleBubbleDemoAudio(payload.attachment.id);
}

function onBubbleDemoActivityItemToggle(messageId: string, itemKey: string, event: Event): void {
  const nextKey = bubbleDemoActivityItemKey(messageId, itemKey);
  bubbleDemoActivityItemOpenKey.value = detailsOpenFromEvent(event) ? nextKey : "";
}

async function sendNativeNotification() {
  sending.value = true;
  errorText.value = "";
  resultText.value = "";

  try {
    const result = await sendTransportNativeNotificationDemo<NativeNotificationDemoResult>();
    resultText.value = [
      t("config.demo.nativeNotificationSent"),
      `title: ${result.title}`,
      `permissionBefore: ${result.permissionBefore}`,
      `permissionAfter: ${result.permissionAfter}`,
      `sentAt: ${result.sentAt}`,
    ].join("\n");
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
  } finally {
    sending.value = false;
  }
}

async function restartApp() {
  restarting.value = true;
  errorText.value = "";
  resultText.value = "";

  try {
    await restartTransportApplicationDemo();
    resultText.value = t("config.demo.restartRequested");
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
    restarting.value = false;
  }
}

async function loadMemoryStats() {
  loadingMemoryStats.value = true;
  errorText.value = "";
  memoryStatsText.value = "";

  try {
    const result = await invokeTauri<unknown>("dump_memory_cache_stats");
    memoryStatsText.value = JSON.stringify(result, undefined, 2);
  } catch (error) {
    errorText.value = error instanceof Error ? error.message : String(error);
  } finally {
    loadingMemoryStats.value = false;
  }
}

function createDemoDelegateStatus(
  delegateId: string,
  title: string,
  elapsedMs: number,
  requestCount: number,
  tokenCount: number,
  lastToolName: string,
): ConversationDelegateStatusSummary {
  const now = new Date().toISOString();
  return {
    delegateId,
    kind: "normal",
    conversationId: `${delegateId}-conversation`,
    rootConversationId: "demo-root-conversation",
    title,
    status: "running",
    active: true,
    startedAt: now,
    updatedAt: now,
    elapsedMs,
    requestCount,
    toolCallCount: requestCount,
    lastToolName,
    tokenCount,
    targetAgentId: "demo-agent",
  };
}

function advanceDemoDelegateStatus() {
  demoDelegateStatuses.value = demoDelegateStatuses.value.map((delegate, index) => {
    if (!delegate.active) return delegate;
    const nextStep = index === 0 ? 1 : 2;
    const nextToken = index === 0 ? 680 : 1340;
    return {
      ...delegate,
      elapsedMs: delegate.elapsedMs + 1000,
      requestCount: delegate.requestCount + nextStep,
      toolCallCount: delegate.toolCallCount + nextStep,
      tokenCount: delegate.tokenCount + nextToken,
      updatedAt: new Date().toISOString(),
    };
  });
}

watch(demoHasRunningDelegates, (nextActive) => {
  demoDelegateStatuses.value = demoDelegateStatuses.value.map((delegate, index) => {
    if (index > 1) return delegate;
    return {
      ...delegate,
      status: nextActive ? "running" : "completed",
      active: nextActive,
      completedAt: nextActive ? undefined : new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
  });
}, { immediate: true });

onMounted(() => {
  delegateDemoTimer = window.setInterval(advanceDemoDelegateStatus, 1000);
});

onBeforeUnmount(() => {
  if (!delegateDemoTimer) return;
  window.clearInterval(delegateDemoTimer);
  delegateDemoTimer = 0;
});
</script>
