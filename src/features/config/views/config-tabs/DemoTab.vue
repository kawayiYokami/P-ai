<template>
  <div class="grid h-full gap-3 overflow-y-auto pr-1">
    <div :class="bare ? '' : 'card border border-base-300 bg-base-100'">
      <div :class="bare ? 'grid gap-3' : 'card-body gap-3 p-4'">
        <div v-if="!bare" class="space-y-1">
          <h3 class="card-title text-base">组件样式 Demo</h3>
          <p class="text-sm text-base-content/70">通过下拉框切换需要展示的组件样式。</p>
        </div>
        <div v-if="!bare" class="flex flex-wrap items-center gap-3">
          <select v-model="demoComponentKey" class="select select-bordered select-sm w-64">
            <option value="question">提问卡（ChatQuestionPanel）</option>
            <option value="question-tool">会话提问（question 工具）</option>
            <option value="bubbles">自研气泡</option>
            <option value="delegates">DelegateProgressLine</option>
            <option value="templates">ConfigTemplate</option>
            <option value="overview">监控概览（MonitorOverview）</option>
            <option value="home-cards">预览卡画廊</option>
          <option value="conversation-list">列表卡画廊</option>
            <option value="conversation-item">会话条目（状态×未读）</option>
            <option value="chat-message">消息项（队伍消息气泡）</option>
            <option value="thinking-and-tools">思考与工具（折叠块 + 300TPS 流式压测）</option>
            <option value="chat-view-stream">对话视图流式复现（ChatView 全组件）</option>
            <option value="chat-view-margin">对话视图滚动落点（startMargin 实测）</option>
            <option value="file-reader-wrap">文件阅读器换行模式（总高与末尾可达性）</option>
            <option value="image-preview-dialog">图片预览百分比读数（与缩放一致性）</option>
            <option value="composer">输入面板新结构</option>
            <option value="double-deck">双层卡（DoubleDeck）</option>
            <option value="thinking-preview">思考预览 bar</option>
            <option value="rich-block-embed">代码块 / mermaid 内嵌气泡</option>
            <option value="markdown-view">Markdown 全量渲染</option>
            <option value="theme-cards">主题预设卡片</option>
            <option value="session-float-dock">会话悬浮操作区</option>
            <option value="session-timeline">会话时间线（垂直）</option>
            <option value="config-cards">配置卡片画廊（六大卡片第一性原理）</option>
            <option value="quick-model-card">快速模型不可用卡片</option>
            <option value="catalog-store">能力商店</option>
            <option value="catalog-entry-card">能力商店条目卡（CatalogEntryCard）</option>
            <option value="persona-capability">人格主页（移动端极简版）</option>
            <option value="persona-permission">人格权限页（黑白名单树）</option>
            <option value="settings-shell">设置页头部（面包屑 + 标签页）</option>
            <option value="search-engines">搜索设置（供应商页第六个标签）</option>
            <option value="kanban-board">议题看板页（KanbanBoardPanel）</option>
            <option value="kanban-card">议题看板磁贴（3×3）</option>
            <option value="team-member-tiles">队员磁贴（自适应列数）</option>
            <option value="chat-home">首页磁贴区（追问入口 + 队员状态表）</option>
            <option value="skill-managed">技能面板（托管技能禁用态）</option>
            <option value="panel-tabs">标签栏组件（PanelTabStrip）</option>
            <option value="file-reader-preview">右栏内容视图（HTML 沙箱 / 源码 / 图片）</option>
            <option value="app-icons">图标语义名总表（AppIcon 全量）</option>
            <option value="app-loading">加载指示器（AppLoading 四叶草）</option>
          </select>
          <span class="text-xs text-base-content/50">当前：{{ demoComponentLabel }}</span>
        </div>

        <div v-if="demoComponentKey === 'persona-capability'" class="pt-2">
          <PersonaTabDemo view="profile" />
        </div>

        <div v-if="demoComponentKey === 'persona-permission'" class="pt-2">
          <PersonaTabDemo view="permission" />
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

        <div v-if="demoComponentKey === 'question-tool'" class="space-y-3 pt-2">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-xs text-base-content/60">预设：</span>
            <button type="button" class="btn btn-xs" :class="demoQuestionToolItems.length > 0 ? 'btn-primary' : 'btn-ghost'" @click="setDemoQuestionToolPreset('pending')">3 题待答（#2/#5/#7）</button>
            <button type="button" class="btn btn-xs" :class="demoQuestionToolItems.length === 0 ? 'btn-primary' : 'btn-ghost'" @click="setDemoQuestionToolPreset('empty')">全部发完（空态）</button>
            <button type="button" class="btn btn-xs btn-ghost" @click="resetDemoQuestionTool">重置</button>
            <span v-if="demoQuestionToolLastSubmit" class="text-xs text-success">已发送 {{ demoQuestionToolLastSubmitLabel }}<template v-if="demoQuestionToolItems.length > 0">，还剩 {{ demoQuestionToolItems.length }} 题</template></span>
          </div>
          <p class="text-xs text-base-content/60">
            数据走真实映射 <code class="text-xs">buildConversationQuestionItems</code>，
            节奏 <code class="text-xs">submit-mode="manual"</code>：编号原样显示，点选项只是勾选，
            点右下角「发送 N 题」才把这批已选题一起发出去，发出的题从列表消失。
            选项末尾「其他（自己写）」可以自己写一段，写下的内容就是这一题的答案。
            点题干右侧「拒绝回答」只把这一题移除，不发送任何内容、不产生消息，其余题的勾选保留。
          </p>
          <div v-if="demoQuestionToolPanelItems.length > 0" class="flex justify-center">
            <ChatQuestionPanel
              key="demo-question-tool"
              :items="demoQuestionToolPanelItems"
              v-model="demoQuestionToolAnswers"
              submit-mode="manual"
              class="flex-1"
              @submit="onDemoQuestionToolSubmit"
              @reject="onDemoQuestionToolReject"
            />
          </div>
          <div v-else class="rounded-lg border border-dashed border-base-300 p-6 text-center text-sm text-base-content/60">
            当前会话没有问题：面板不渲染，输入面板恢复原样
          </div>
          <div v-if="demoQuestionToolLastSubmit" class="mockup-code max-h-64 overflow-auto text-xs">
            <pre class="whitespace-pre-wrap break-all"><code>{{ JSON.stringify(demoQuestionToolLastSubmit, undefined, 2) }}</code></pre>
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

    <div v-if="demoComponentKey === 'rich-block-embed'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">代码块 / mermaid 内嵌气泡（ChatRichBlockEmbedDemo）</h3>
          <p class="text-sm text-base-content/70">左边是改动前的分段气泡（代码块、mermaid 各自独立成块），右边是改动后富块内嵌进气泡、代码块去掉底色与框线的样子。两边用的是同一段 markdown 与真实的 markdown 渲染器。</p>
        </div>
        <ChatRichBlockEmbedDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'markdown-view'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">Markdown 全量渲染（AppMarkdownRendererDemo）</h3>
          <p class="text-sm text-base-content/70">一份文本覆盖解析器全部块与行内类型，可切三种疏密档位、chat/document 变体、streaming 与深色配色。</p>
        </div>
        <AppMarkdownRendererDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'theme-cards'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">主题预设卡片（AutoThemeGrid）</h3>
          <p class="text-sm text-base-content/70">每张卡片在品牌色（主色/次色/强调/中性）之下列出四大状态色（成功/警告/错误/信息），用来判断该主题的状态色是否够醒目。卡片用真实 DaisyUI 主题渲染。</p>
        </div>
        <AutoThemeGrid
          :light-themes="demoLightThemes"
          :dark-themes="demoDarkThemes"
          auto-light-theme="light"
          auto-dark-theme="dark"
          :light-custom-tokens="demoLightTokens"
          :dark-custom-tokens="demoDarkTokens"
          @select-light="() => {}"
          @select-dark="() => {}"
        />
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

    <div v-if="demoComponentKey === 'quick-model-card'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">快速模型不可用卡片</h3>
          <p class="text-sm text-base-content/70">
            头像生成失败时弹出：一句可读的原因、快速模型下拉框、以及「想禁用就设为不配置」的说明。招募人格在聊天窗口弹、保存人格在配置窗口弹，同一份组件。
          </p>
        </div>
        <QuickModelUnavailableCardDemo />
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

    <div v-if="demoComponentKey === 'chat-home'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">首页磁贴区（追问入口 + 队员状态表）</h3>
          <p class="text-sm text-base-content/70">
            首页只留一个「追问」入口磁贴，追问会话本身不铺卡；队员用一块状态表占磁贴墙的 2×2 格，内部每行一个队员、可见两行，多的在里面滚动。点队员那一行打开该队员会话（demo 里原地丢弃）。
          </p>
        </div>
        <ChatHomePanelDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'skill-managed'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">技能面板（托管技能禁用态）</h3>
          <p class="text-sm text-base-content/70">
            渲染真实 SkillTab，喂两个托管技能（persona-craft / pai-guide）与一个普通内置技能、一个自定义技能。托管技能标「系统托管」，开关是禁用态，详情页写着一句为什么关不掉；开关与保存在这张 demo 里不写库。
          </p>
        </div>
        <SkillTabDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'team-member-tiles'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">队员磁贴（首页磁贴墙占两列）</h3>
          <p class="text-sm text-base-content/70">
            一格一个队员：头像、队员名、他手上那件事。有在办议题时写「负责 #3 标题」，最右边一圈转着表示正在跑；没有议题就不硬凑句子，只显示他自己的运行时状态（忙碌 / 空闲 / 出错）。队员名取自会话绑定的 agent，会话标题交给自动标题生成。
          </p>
        </div>
        <TeamMemberTileGridDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'config-cards'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">配置卡片画廊（五大卡片第一性原理）</h3>
          <p class="text-sm text-base-content/70">集中渲染联系人渠道、连接器、供应商、人格、技能 5 类卡片的真实模拟数据与自适应网格，支持切换宽度与主题进行视觉检验。</p>
        </div>
        <ConfigCardsGalleryDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'catalog-entry-card'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">能力商店条目卡</h3>
          <p class="text-sm text-base-content/70">真实组件 + mock 条目。一行只放图标、名称、来源、两行说明和一个动作。</p>
        </div>
        <CatalogEntryCardDemo />
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

    <div v-if="demoComponentKey === 'conversation-item'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">会话条目（状态 × 未读）</h3>
          <p class="text-sm text-base-content/70">状态胶囊只讲状态，未读红圆只讲未读，两者并排不合并。含当前会话与 mini 行。</p>
        </div>
        <ChatConversationItemDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'chat-message'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">消息项（队伍邮件 = 普通气泡）</h3>
          <p class="text-sm text-base-content/70">队伍消息就是一条普通 assistant 消息：头像、人格名、时间、正文按 markdown 渲染。邮件本身拼成一段 markdown，标题在 <code>&lt;summary&gt;</code> 里、正文与状态行在折叠体内，默认收起。与上一条普通消息对照看，气泡形态完全一致，多出来的只是标题那一行可点开。</p>
        </div>
        <ChatMessageItemDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'chat-view-stream'" class="h-[720px] min-h-0 overflow-hidden rounded-box border border-base-300">
      <ChatViewStreamDemo />
    </div>

    <div v-if="demoComponentKey === 'chat-view-margin'" class="min-h-0">
      <ChatViewDemo />
    </div>

    <div v-if="demoComponentKey === 'file-reader-wrap'" class="min-h-0">
      <FileReaderPanelDemo />
    </div>

    <div v-if="demoComponentKey === 'image-preview-dialog'" class="min-h-0">
      <ChatImagePreviewDialogDemo />
    </div>

    <div v-if="demoComponentKey === 'thinking-and-tools'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">思考与工具（折叠块样本 + 流式压测）</h3>
          <p class="text-sm text-base-content/70">
            直接渲染真实 <code>ChatMessageItem</code>，展示块由 <code>useChatMessageBlocks</code> 从模拟消息派生——派生与渲染两段都是产品代码，这里只换数据。
            上面是形态样本：长思维链、三种工具形态、正文代码块与表格，以及跑完 / 正在跑 / 失败中断三种状态；
            下面是流式压测，按时间表往真实消息的正文里逐帧追加字符，默认 300 TPS / 200K，可暂停与重置。
          </p>
        </div>
        <ChatThinkingAndToolsDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'settings-shell'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1">
          <h3 class="card-title text-base">设置页头部（面包屑 + 左格 + 右格）</h3>
          <p class="text-sm text-base-content/70">所有设置页共用的头部骨架：面包屑、左格（标签页或搜索框）、右格（操作按钮），以及切换标签时内容区的左右进出。直接渲染真实组件。</p>
        </div>
        <SettingsShellGalleryDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'search-engines'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div class="space-y-1" data-demo-chrome>
          <h3 class="card-title text-base">搜索设置（供应商页第六个标签）</h3>
          <p class="text-sm text-base-content/70">真实 SettingsPageShell + SearchEnginesPanel：调度模式一张卡，五个搜索来源各一张卡。</p>
        </div>
        <SearchEnginesDemo />
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
            <AppIcon name="rotate-ccw" class="size-4" aria-hidden="true" />
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
          <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'shellsOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'shellsOnly'">仅终端</button>
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

        <div class="overflow-hidden rounded-[1.75rem] border border-base-300 bg-base-200/70 p-4 shadow-inner">
          <div class="flex flex-col gap-4 rounded-[1.35rem] border border-base-300 bg-base-100/30 px-3 py-4">
            <ChatBubbleShell
              v-for="message in bubbleDemoDisplayMessages"
              :key="message.id"
              :tone="message.tone"
              :name="message.name"
              :meta="message.meta"
              :avatar-url="message.avatarUrl"
              :streaming="!!message.streaming"
              :streaming-text="message.streamingText"
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
                          <AppIcon name="file-text" v-if="tool.icon === 'file'" class="size-3" aria-hidden="true" />
                          <AppIcon name="square-terminal" v-else-if="tool.icon === 'terminal'" class="size-3" aria-hidden="true" />
                          <AppIcon name="wrench" v-else class="size-3" aria-hidden="true" />
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
                  <AppIcon name="copy" class="size-3" aria-hidden="true" />
                  复制
                </span>
                <span v-if="!message.streaming && message.canRecall" class="inline-flex items-center gap-1">
                  <AppIcon name="undo" class="size-3" aria-hidden="true" />
                  撤回
                </span>
              </template>
            </ChatBubbleShell>
          </div>
        </div>
      </div>
    </div>

    <!-- 议题看板页：容器宽度模拟默认 618px 窗口下的右栏内容区 586px -->
    <div v-if="demoComponentKey === 'kanban-board'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-2 p-3">
        <p class="text-xs text-base-content/60">容器宽 586px，对应默认 618px 窗口下的右栏内容区。</p>
        <div class="h-[660px] w-[586px] overflow-hidden rounded-box border border-base-300">
          <KanbanBoardPanel
            :issues="demoKanbanIssues"
            :loading="false"
            :error-text="''"
            :approving-number="null"
            leader-name="Claude"
            :leader-avatar-url="demoKanbanAvatarMap['leader']"
            :avatar-url-map="demoKanbanAvatarMap"
          />
        </div>
      </div>
    </div>

    <!-- 议题看板磁贴：放进真实的 .ecall-home-flow 网格里看 3×3 效果 -->
    <div v-if="demoComponentKey === 'kanban-card'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-3">
        <p class="text-xs text-base-content/60">
          网格宽 586px（默认窗口右栏内容区），auto-fill 出 3 列，磁贴占满整行跨三行。
        </p>
        <div class="ecall-kanban-demo-flow w-full max-w-[586px]">
          <HomeKanbanCard
            :issues="demoKanbanIssues"
            :avatar-url-map="demoKanbanAvatarMap"
            leader-name="Claude"
            :leader-avatar-url="demoKanbanAvatarMap['leader']"
          />
          <div class="ecall-kanban-demo-filler">环境卡占位</div>
          <div class="ecall-kanban-demo-filler">计划卡占位</div>
          <div class="ecall-kanban-demo-filler">工作区卡占位</div>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'panel-tabs'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-4 p-4">
        <div>
          <h4 class="card-title text-base">自适应标签栏 (PanelTabStrip)</h4>
          <p class="text-xs text-base-content/60">
            当宽度不足时自动折叠多余标签至“...”更多菜单（默认鼠标悬停即可直接展开），每张卡片保留最小保护宽度（默认 104px / 文件 92px），当前激活标签优先保留在可见区。
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <span class="text-xs text-base-content/70">容器宽度调节：</span>
          <input
            v-model.number="demoPanelTabsContainerWidth"
            type="range"
            min="200"
            max="700"
            step="10"
            class="range range-xs range-primary w-64"
          />
          <span class="font-mono text-xs">{{ demoPanelTabsContainerWidth }}px</span>
          <button
            type="button"
            class="btn btn-ghost btn-xs"
            @click="demoPanelTabsContainerWidth = 360"
          >
            默认 (360px)
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs"
            @click="demoPanelTabsContainerWidth = 240"
          >
            窄视口 (240px)
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs"
            @click="demoPanelTabsContainerWidth = 560"
          >
            宽视口 (560px)
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs text-primary"
            @click="resetDemoPanelTabs"
          >
            重置标签列表
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <div class="mb-1.5 flex items-center justify-between text-xs font-medium text-base-content/70">
              <span>会话风格 (variant="default")</span>
              <span class="text-micro text-base-content/50">当前激活：{{ demoPanelActiveKey }}</span>
            </div>
            <div
              class="overflow-hidden rounded-box border border-base-300 bg-base-200"
              :style="{ width: `${demoPanelTabsContainerWidth}px`, maxWidth: '100%' }"
            >
              <PanelTabStrip
                :tabs="demoPanelTabs"
                :active-key="demoPanelActiveKey"
                aria-label="Demo Tabs"
                close-title="关闭"
                @select-tab="(k) => (demoPanelActiveKey = k)"
                @close-tab="onDemoClosePanelTab"
              />
            </div>
          </div>

          <div>
            <div class="mb-1.5 flex items-center justify-between text-xs font-medium text-base-content/70">
              <span>文件风格 (variant="file")</span>
              <span class="text-micro text-base-content/50">当前激活：{{ demoPanelFileActiveKey }}</span>
            </div>
            <div
              class="overflow-hidden rounded-box border border-base-300 bg-base-200"
              :style="{ width: `${demoPanelTabsContainerWidth}px`, maxWidth: '100%' }"
            >
              <PanelTabStrip
                variant="file"
                :tabs="demoPanelFileTabs"
                :active-key="demoPanelFileActiveKey"
                aria-label="Demo File Tabs"
                close-title="关闭"
                @select-tab="(k) => (demoPanelFileActiveKey = k)"
                @close-tab="onDemoCloseFileTab"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="demoComponentKey === 'file-reader-preview'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-4 p-4">
        <div>
          <h4 class="file-reader-preview-demo-caption card-title text-base">右栏 reader 内容视图（HTML 沙箱 / 源码 / 图片）</h4>
          <p class="file-reader-preview-demo-caption text-xs text-base-content/60">
            面板里用的同一批组件：html 走 InteractiveHtmlSandbox 沙箱渲染，切源码复用同一段原文，图片走 image 预览通道（聊天图片就是靠这条通道进右栏）。
          </p>
        </div>
        <FileReaderPreviewDemo />
      </div>
    </div>

    <div v-if="demoComponentKey === 'app-icons'" class="card border border-base-300 bg-base-100">
      <div class="card-body gap-3 p-4">
        <div>
          <h4 class="card-title text-base">AppIcon 语义名总表（{{ appIconNames.length }} 个）</h4>
          <p class="text-xs text-base-content/60">
            全部取自 icon-map.ts，格子里就是 AppIcon 按该语义名渲染的真实结果，换图标库时先在这里逐名核对。
          </p>
        </div>
        <label class="input input-bordered input-sm flex items-center gap-2">
          <AppIcon name="search" class="h-3.5 w-3.5 opacity-50" />
          <input v-model="appIconFilter" class="grow bg-transparent outline-none" placeholder="按语义名过滤" />
        </label>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-1.5">
          <div
            v-for="name in filteredAppIconNames"
            :key="name"
            class="flex min-w-0 flex-col items-center gap-1.5 rounded-lg border border-base-300/70 bg-base-200/40 px-1.5 py-2"
          >
            <AppIcon :name="name" class="h-6 w-6" />
            <span class="w-full truncate text-center font-mono text-[10px] leading-tight text-base-content/60" :title="name">{{ name }}</span>
          </div>
        </div>
        <p v-if="!filteredAppIconNames.length" class="py-6 text-center text-xs text-base-content/50">
          没有匹配「{{ appIconFilter }}」的语义名
        </p>
      </div>
    </div>

    <div v-if="demoComponentKey === 'app-loading'" class="card border border-base-300 bg-base-100">
      <div class="card-body p-0"><AppLoadingDemo /></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "../../../shared/components/AppIcon.vue";
import AppLoadingDemo from "../../../shared/components/AppLoadingDemo.vue";
import { APP_ICON_MAP, type AppIconName } from "../../../shared/icons/icon-map";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import KanbanBoardPanel from "../../../chat/components/kanban/KanbanBoardPanel.vue";
import HomeKanbanCard from "../../../chat/components/chat-home/HomeKanbanCard.vue";
import type { KanbanIssue } from "../../../chat/composables/use-kanban-board";

function svgDataUrl(svg: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const demoKanbanAvatarMap: Record<string, string> = {
  leader: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><g fill="#FFFFFF" transform="translate(10, 10)"><path d="M10 2.5a1.5 1.5 0 0 0-1.5 1.5v3.2A7.5 7.5 0 0 0 4 10a7.5 7.5 0 0 0 7.5 7.5 7.5 7.5 0 0 0 6-3v1.5a1.5 1.5 0 0 0 3 0V4a1.5 1.5 0 0 0-1.5-1.5H10z" opacity="0.95"/><circle cx="10" cy="10" r="3" fill="#FDE68A"/></g></svg>`),
  小吉祥草王: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#10B981"/><path d="M20 9c-4 5-3 10 0 14 3-4 4-9 0-14zM14 17c5-2 9 0 11 3-5 2-9 0-11-3zM26 17c-5-2-9 0-11 3 5 2 9 0 11-3z" fill="#ECFDF5"/><circle cx="20" cy="18" r="3" fill="#A7F3D0"/></svg>`),
  莫娜: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#6366F1"/><path d="M20 8l2.5 8.5L31 19l-8.5 2.5L20 30l-2.5-8.5L9 19l8.5-2.5z" fill="#EEF2FF"/><circle cx="20" cy="19" r="2.5" fill="#C7D2FE"/></svg>`),
  欧科塔维亚: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#06B6D4"/><circle cx="20" cy="20" r="9" fill="none" stroke="#E0F2FE" stroke-width="2.5"/><path d="M16 16v8l7-4z" fill="#E0F2FE"/></svg>`),
  桑多涅: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#EC4899"/><circle cx="20" cy="20" r="6" fill="#FCE7F3"/><path d="M14 16c3 2 3 6 0 8 3-1 6-1 8-3-2-2-5-2-8-5zM26 16c-3 2-3 6 0 8-3-1-6-1-8-3 2-2 5-2 8-5z" fill="#FFF1F2"/></svg>`),
  钟离: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#D97706"/><rect x="13" y="13" width="14" height="14" rx="2" fill="none" stroke="#FEF3C7" stroke-width="2.5"/><rect x="17" y="17" width="6" height="6" fill="#FEF3C7"/></svg>`),
  小黑塔: svgDataUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40"><rect width="40" height="40" rx="20" fill="#8B5CF6"/><circle cx="20" cy="15" r="4.5" fill="#EDE9FE"/><path d="M13 27c0-4 3.5-6 7-6s7 2 7 6z" fill="#EDE9FE"/></svg>`),
};

/** 看板 demo 用的假议题：覆盖看板呈现的六个状态，且四类提醒各有内容，便于一眼看全。 */
const demoKanbanIssues: KanbanIssue[] = [
  { number: 3, title: "发布流程要不要加一步回归检查", status: "in_review", assignee: "小吉祥草王", note: "三条回归都跑过了，等确认口径", updatedAt: "2026-11-01T11:20:00+08:00" },
  { number: 5, title: "看板磁贴尺寸定稿", status: "in_review", assignee: "莫娜", updatedAt: "2026-11-01T11:35:00+08:00", note: "" },
  { number: 11, title: "提问工具的回答要不要带时间戳", status: "in_review", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:46:00+08:00", note: "" },
  { number: 12, title: "队伍名册改角色后的文案", status: "in_review", assignee: "leader", updatedAt: "2026-11-01T11:47:00+08:00", note: "" },
  { number: 7, title: "队员 3 号的 worktree 有未解决冲突", status: "blocked", assignee: "桑多涅", note: "两边都改了 ChatView.vue，需要你定谁先合", updatedAt: "2026-11-01T10:05:00+08:00" },
  { number: 13, title: "摘要工具的回滚逻辑要重做", status: "blocked", assignee: "钟离", note: "等确认快照失败时回滚到哪一版", updatedAt: "2026-11-01T10:30:00+08:00" },
  { number: 8, title: "提问工具要不要支持多选", status: "discussing", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:40:00+08:00", note: "" },
  { number: 9, title: "会话行待定问题数的视觉样式", status: "discussing", assignee: "桑多涅", updatedAt: "2026-11-01T11:42:00+08:00", note: "" },
  { number: 10, title: "议题编号不回收的边界测试", status: "discussing", assignee: "莫娜", updatedAt: "2026-11-01T11:44:00+08:00", note: "" },
  { number: 14, title: "看板页要不要支持拖拽改状态", status: "discussing", assignee: "莫娜", updatedAt: "2026-11-01T11:48:00+08:00", note: "" },
  { number: 1, title: "模型继承链改成父会话优先", status: "in_progress", assignee: "小黑塔", updatedAt: "2026-11-01T11:10:00+08:00", note: "" },
  { number: 2, title: "专家模型改成被动跟随", status: "todo", assignee: "小黑塔", note: "" },
  { number: 4, title: "重跑队长提示词行为测试", status: "todo", assignee: "观测员", note: "" },
  { number: 6, title: "队伍名册改用角色标识", status: "done", assignee: "桑多涅", note: "已确认由界面通过" },
];
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
import {
  buildConversationQuestionItems,
  pruneConversationQuestionAnswers,
  type ConversationQuestionView,
} from "../../../chat/utils/conversation-question";
import ChatBubbleShell from "../../../chat/components/ChatBubbleShell.vue";
import ExpandableText from "../../../shared/components/ExpandableText.vue";
import ChatAttachmentList from "../../../chat/components/ChatAttachmentList.vue";
import DelegateCard from "../../../chat/components/DelegateCard.vue";
import MonitorOverview from "../../../chat/components/MonitorOverview.vue";
import ChatComposerStructureDemo from "../../../chat/components/ChatComposerStructureDemo.vue";
import DoubleDeckCardDemo from "../../../chat/components/DoubleDeckCardDemo.vue";
import ChatThinkingPreviewBarDemo from "../../../chat/components/ChatThinkingPreviewBarDemo.vue";
import ChatRichBlockEmbedDemo from "../../../chat/components/ChatRichBlockEmbedDemo.vue";
import AppMarkdownRendererDemo from "../../../chat/markdown/AppMarkdownRendererDemo.vue";
import AutoThemeGrid from "../../components/AutoThemeGrid.vue";
import {
  generateGeneratedThemeTokens,
  getGeneratedThemeDefaultControls,
} from "../../../shell/theme/theme-generator";
import SessionFloatDockDemo from "../../../chat/components/SessionFloatDockDemo.vue";
import SessionTimelineDemo from "../../../chat/components/SessionTimelineDemo.vue";
import TeamMemberTileGridDemo from "../../../chat/components/TeamMemberTileGridDemo.vue";
import ChatHomePanelDemo from "../../../chat/components/ChatHomePanelDemo.vue";
import HomeCardGalleryDemo from "../../../chat/components/HomeCardGalleryDemo.vue";
import ChatConversationListGalleryDemo from "../../../chat/components/ChatConversationListGalleryDemo.vue";
import ChatConversationItemDemo from "../../../chat/components/ChatConversationItemDemo.vue";
import ChatMessageItemDemo from "../../../chat/components/ChatMessageItemDemo.vue";
import ChatThinkingAndToolsDemo from "../../../chat/components/ChatThinkingAndToolsDemo.vue";
import ChatViewStreamDemo from "../../../chat/views/ChatViewStreamDemo.vue";
import ChatImagePreviewDialogDemo from "../../../chat/components/dialogs/ChatImagePreviewDialogDemo.vue";
import ChatViewDemo from "../../../chat/views/ChatViewDemo.vue";
import FileReaderPanelDemo from "../../../file-reader/components/FileReaderPanelDemo.vue";
import ConfigCardsGalleryDemo from "../../components/ConfigCardsGalleryDemo.vue";
import QuickModelUnavailableCardDemo from "../../components/QuickModelUnavailableCardDemo.vue";
import SettingsShellGalleryDemo from "../../components/SettingsShellGalleryDemo.vue";
import SearchEnginesDemo from "./SearchEnginesDemo.vue";
import CatalogStoreDemo from "../../components/CatalogStoreDemo.vue";
import CatalogEntryCardDemo from "../../components/CatalogEntryCardDemo.vue";
import PersonaTabDemo from "../../components/PersonaTabDemo.vue";
import SkillTabDemo from "../../components/SkillTabDemo.vue";
import PanelTabStrip from "../../../shared/components/PanelTabStrip.vue";
import FileReaderPreviewDemo from "../../../file-reader/components/FileReaderPreviewDemo.vue";
import type { PanelTabStripItem } from "../../../shared/components/panel-tab-strip.types";
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
  streamingText?: string;
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
  assistantAgentId?: string;
  initialKey?: string;
  bare?: boolean;
}>(), {
  config: () => ({} as AppConfig),
  personas: () => [],
  personaAvatarUrlMap: () => ({}),
  assistantAgentId: "",
  initialKey: "question",
  bare: false,
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
const demoComponentKey = ref<"question" | "question-tool" | "bubbles" | "delegates" | "templates" | "overview" | "home-cards" | "conversation-list" | "conversation-item" | "chat-message" | "thinking-and-tools" | "chat-view-stream" | "chat-view-margin" | "file-reader-wrap" | "image-preview-dialog" | "composer" | "double-deck" | "thinking-preview" | "rich-block-embed" | "markdown-view" | "theme-cards" | "session-float-dock" | "session-timeline" | "config-cards" | "quick-model-card" | "catalog-store" | "catalog-entry-card" | "persona-capability" | "persona-permission" | "settings-shell" | "search-engines" | "kanban-board" | "kanban-card" | "team-member-tiles" | "chat-home" | "skill-managed" | "panel-tabs" | "file-reader-preview" | "app-icons" | "app-loading">((props.initialKey as any) || "question");
/** 图标语义名总表：直接取映射表的键，杜绝手抄漏项 */
const appIconNames = Object.keys(APP_ICON_MAP).sort() as AppIconName[];
const appIconFilter = ref("");
const filteredAppIconNames = computed(() => {
  const keyword = appIconFilter.value.trim().toLowerCase();
  return keyword ? appIconNames.filter((name) => name.includes(keyword)) : appIconNames;
});
const demoLightThemes = ["light", "cupcake", "corporate", "lofi", "fantasy", "autumn"];
const demoDarkThemes = ["dark", "halloween", "forest", "dracula", "business", "dim"];
const demoLightTokens = generateGeneratedThemeTokens(getGeneratedThemeDefaultControls("light"));
const demoDarkTokens = generateGeneratedThemeTokens(getGeneratedThemeDefaultControls("dark"));

// bare 模式下把内容高度写进标题，便于无头截图按内容高度精确裁剪。
onMounted(() => {
  if (!props.bare) return;
  window.setTimeout(() => {
    document.title = `${document.documentElement.scrollHeight}`;
  }, 1500);
});
const demoComponentLabel = computed(() => {
  if (demoComponentKey.value === "persona-capability") return "人格主页";
  if (demoComponentKey.value === "persona-permission") return "人格权限页";
  if (demoComponentKey.value === "catalog-store") return "能力商店";
  if (demoComponentKey.value === "catalog-entry-card") return "能力商店条目卡";
  if (demoComponentKey.value === "settings-shell") return "设置页头部";
  if (demoComponentKey.value === "search-engines") return "搜索设置";
  if (demoComponentKey.value === "kanban-board") return "议题看板页";
  if (demoComponentKey.value === "kanban-card") return "议题看板磁贴";
  if (demoComponentKey.value === "team-member-tiles") return "队员磁贴";
  if (demoComponentKey.value === "chat-home") return "首页磁贴区";
  if (demoComponentKey.value === "skill-managed") return "技能面板（托管技能禁用态）";
  if (demoComponentKey.value === "config-cards") return "配置卡片画廊";
  if (demoComponentKey.value === "quick-model-card") return "快速模型不可用卡片";
  if (demoComponentKey.value === "session-float-dock") return "会话悬浮操作区";
  if (demoComponentKey.value === "session-timeline") return "会话时间线（垂直）";
  if (demoComponentKey.value === "thinking-preview") return "思考预览 bar";
  if (demoComponentKey.value === "rich-block-embed") return "代码块 / mermaid 内嵌气泡";
  if (demoComponentKey.value === "markdown-view") return "Markdown 全量渲染";
  if (demoComponentKey.value === "theme-cards") return "主题预设卡片";
  if (demoComponentKey.value === "double-deck") return "双层卡";
  if (demoComponentKey.value === "home-cards") return "预览卡画廊";
  if (demoComponentKey.value === "conversation-list") return "列表卡画廊";
  if (demoComponentKey.value === "conversation-item") return "会话条目（状态×未读）";
  if (demoComponentKey.value === "chat-message") return "消息项（队伍消息气泡）";
  if (demoComponentKey.value === "thinking-and-tools") return "思考与工具（折叠块 + 流式压测）";
  if (demoComponentKey.value === "chat-view-stream") return "对话视图流式复现（ChatView 全组件）";
  if (demoComponentKey.value === "chat-view-margin") return "对话视图滚动落点（startMargin 实测）";
  if (demoComponentKey.value === "file-reader-wrap") return "文件阅读器换行模式（总高与末尾可达性）";
  if (demoComponentKey.value === "image-preview-dialog") return "图片预览百分比读数（与缩放一致性）";
  if (demoComponentKey.value === "composer") return "输入面板新结构";
  if (demoComponentKey.value === "question") return "提问卡";
  if (demoComponentKey.value === "question-tool") return "会话提问（question 工具）";
  if (demoComponentKey.value === "bubbles") return "自研气泡";
  if (demoComponentKey.value === "delegates") return "DelegateProgressLine";
  if (demoComponentKey.value === "overview") return "监控概览（MonitorOverview）";
  if (demoComponentKey.value === "panel-tabs") return "标签栏组件（PanelTabStrip）";
  if (demoComponentKey.value === "file-reader-preview") return "右栏内容视图（HTML 沙箱 / 源码 / 图片）";
  if (demoComponentKey.value === "app-icons") return "图标语义名总表";
  if (demoComponentKey.value === "app-loading") return "加载指示器（AppLoading）";
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

// ==================== 会话提问（question 工具）====================

/** 模拟后端 `question_list` 返回：编号不连续（#2/#5/#7），验证面板按实际编号显示。 */
const demoQuestionToolInitialItems: ConversationQuestionView[] = [
  {
    number: 2,
    title: "要不要现在发布？",
    background: "改动已经全绿，但 release tag 还没写。",
    options: ["现在发布", "再等等"],
  },
  { number: 5, title: "用哪个分支？", options: ["main", "release"] },
  { number: 7, title: "回滚由谁执行？", options: ["我自己来", "交给值班"] },
];
const demoQuestionToolItems = ref<ConversationQuestionView[]>([...demoQuestionToolInitialItems]);
const demoQuestionToolAnswers = ref<Record<string, { optionId: string; label: string; comment: string }>>({});
const demoQuestionToolLastSubmit = ref<Array<{ id: string; optionId: string; label: string; comment: string }> | null>(null);

/** 走真实映射，和 ChatView 是同一份实现，不手写静态复现。 */
const demoQuestionToolPanelItems = computed(() =>
  buildConversationQuestionItems(demoQuestionToolItems.value),
);

/** 逐题提交时每次只有一题，展示编号比展示条数更有信息量。 */
const demoQuestionToolLastSubmitLabel = computed(() =>
  (demoQuestionToolLastSubmit.value ?? [])
    .map((answer) => `#${answer.id} ${answer.label}`)
    .join("、"),
);

function resetDemoQuestionTool() {
  demoQuestionToolItems.value = [...demoQuestionToolInitialItems];
  demoQuestionToolAnswers.value = {};
  demoQuestionToolLastSubmit.value = null;
}

function setDemoQuestionToolPreset(preset: "pending" | "empty") {
  demoQuestionToolAnswers.value = {};
  demoQuestionToolLastSubmit.value = null;
  demoQuestionToolItems.value = preset === "empty" ? [] : [...demoQuestionToolInitialItems];
}

/** 演示后端答题的效果：该编号从列表移除，答完全部就回到空态。 */
function onDemoQuestionToolSubmit(
  payload: Array<{ id: string; optionId: string; label: string; comment: string }>,
) {
  demoQuestionToolLastSubmit.value = payload;
  const answered = new Set(payload.map((answer) => Number(answer.id)));
  demoQuestionToolItems.value = demoQuestionToolItems.value.filter(
    (item) => !answered.has(item.number),
  );
  demoQuestionToolAnswers.value = {};
}

/** 演示拒绝回答：只把被拒的题移除，不产生任何"已发送"记录，其余题的勾选保留。 */
function onDemoQuestionToolReject(number: number) {
  demoQuestionToolItems.value = demoQuestionToolItems.value.filter(
    (item) => item.number !== number,
  );
  demoQuestionToolAnswers.value = pruneConversationQuestionAnswers(
    demoQuestionToolItems.value,
    demoQuestionToolAnswers.value,
  );
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
    streamingText: "正在执行工具并等待供应商返回结果",
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
    assistantPersonas.value.find((persona) => persona.id === props.assistantAgentId)
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

// ==================== 自适应标签栏 (PanelTabStrip) Demo ====================
const demoPanelTabsContainerWidth = ref(360);
const demoPanelActiveKey = ref("herta");
const demoPanelFileActiveKey = ref("readme");

const initialDemoPanelTabs: PanelTabStripItem[] = [
  {
    key: "herta",
    label: "小黑塔",
    iconSrc: demoKanbanAvatarMap["小黑塔"],
    statusDotClass: "size-2 shrink-0 rounded-full bg-success border border-base-100",
    statusTitle: "在线执行中",
    closeable: true,
  },
  {
    key: "alhaitham",
    label: "书记官",
    iconSrc: demoKanbanAvatarMap["leader"],
    statusDotClass: "size-2 shrink-0 rounded-full bg-base-content/30 border border-base-100",
    statusTitle: "空闲就绪",
    closeable: true,
  },
  { key: "yae", label: "八重神子", iconSrc: demoKanbanAvatarMap["莫娜"], closeable: true },
  { key: "nahida", label: "小吉祥草王", iconSrc: demoKanbanAvatarMap["小吉祥草王"], closeable: true },
  { key: "sandrone", label: "桑多涅", iconSrc: demoKanbanAvatarMap["桑多涅"], closeable: true },
  { key: "zhongli", label: "钟离", iconSrc: demoKanbanAvatarMap["钟离"], busy: true, closeable: true },
];

const demoPanelTabs = ref<PanelTabStripItem[]>([...initialDemoPanelTabs]);

const initialDemoPanelFileTabs: PanelTabStripItem[] = [
  { key: "readme", label: "README.md", icon: "file-text", closeable: true },
  { key: "tabstrip", label: "PanelTabStrip.vue", icon: "file-text", closeable: true },
  { key: "overflow", label: "use-panel-tab-overflow.ts", icon: "file-text", closeable: true },
  { key: "package", label: "package.json", icon: "file-text", closeable: true },
  { key: "cargo", label: "Cargo.toml", icon: "file-text", closeable: true },
  { key: "vite", label: "vite.config.ts", icon: "file-text", closeable: true },
];

const demoPanelFileTabs = ref<PanelTabStripItem[]>([...initialDemoPanelFileTabs]);

function resetDemoPanelTabs() {
  demoPanelTabs.value = [...initialDemoPanelTabs];
  demoPanelActiveKey.value = "herta";
  demoPanelFileTabs.value = [...initialDemoPanelFileTabs];
  demoPanelFileActiveKey.value = "readme";
}

function onDemoClosePanelTab(key: string) {
  demoPanelTabs.value = demoPanelTabs.value.filter((t) => t.key !== key);
  if (demoPanelActiveKey.value === key && demoPanelTabs.value.length > 0) {
    demoPanelActiveKey.value = demoPanelTabs.value[0].key;
  }
}

function onDemoCloseFileTab(key: string) {
  demoPanelFileTabs.value = demoPanelFileTabs.value.filter((t) => t.key !== key);
  if (demoPanelFileActiveKey.value === key && demoPanelFileTabs.value.length > 0) {
    demoPanelFileActiveKey.value = demoPanelFileTabs.value[0].key;
  }
}
</script>

<style scoped>
/* 复刻 ChatHomePanel 的 .ecall-home-flow：列宽 8.75rem、间距 1rem、
   行高下限 6.5rem，磁贴靠 1 / -1 与 span 3 撑成 3×3。 */
.ecall-kanban-demo-flow {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(8.75rem, 1fr));
  grid-auto-rows: minmax(6.5rem, auto);
  gap: 1rem;
  grid-auto-flow: dense;
}

.ecall-kanban-demo-filler {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed color-mix(in oklab, var(--color-base-content) 20%, transparent);
  border-radius: 1rem;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 40%, transparent);
}
</style>
