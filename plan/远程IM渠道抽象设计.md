# 远程IM渠道抽象设计

> 本文档定义远程IM渠道层的数据结构、函数签名和业务流程，不涉及具体实现。

---

## 0. 基础类型别名

```
// 渠道和会话标识
ChannelId = string                // 渠道实例唯一标识

// 消息标识
PlatformMessageId = string        // 平台内消息唯一ID
MessageId = string                // 系统内消息唯一ID

// 文件路径和标识
LocalFilePath = string            // 本地文件系统路径
FilePath = string                 // 文件存储路径（可能是URL或本地路径）
FileKey = string                  // 平台返回的文件标识

// 文件类型
FileType = "image" | "audio" | "video" | "file"
```

---

## 1. 核心数据结构

### 1.1 渠道配置

```
RemoteImChannelConfig {
  id: string                    // 渠道实例唯一标识
  platform: "feishu" | "dingtalk" | "napcat"
  name: string                  // 用户自定义渠道名称
  enabled: boolean

  // 连接配置（按平台不同）
  credentials: FeishuCredentials | DingtalkCredentials | NapcatCredentials

  // 行为策略
  behavior: ChannelBehaviorConfig

  // 权限策略
  permissions: ChannelPermissionConfig
}
```

```
FeishuCredentials {
  app_id: string
  app_secret: string
  bot_name?: string             // 机器人名称，用于识别@自己
}

DingtalkCredentials {
  client_id: string
  client_secret: string
}

NapcatCredentials {
  ws_reverse_host: string       // 监听地址，如 "127.0.0.1"
  ws_reverse_port: number       // 监听端口
  access_token?: string         // 可选的连接凭证
}
```

```
ChannelBehaviorConfig {
  // 是否激活助理
  activate_assistant: boolean

  // 默认回复策略（新远程联系人的初始策略，可按会话覆盖）
  default_reply_mode: "none" | "always" | "on_message"

  // 是否接收文件类消息
  receive_files: boolean

  // 是否流式发送（平台支持时）
  streaming_send: boolean
}

ChannelPermissionConfig {
  // 是否允许LLM主动发消息
  allow_proactive_send: boolean

  // 是否允许LLM发送文件
  allow_send_files: boolean

  // 是否发送工具调用过程
  show_tool_calls: boolean
}
```

### 1.2 远程联系人

```
RemoteContact {
  id: string                    // 映射记录唯一标识

  // 远程联系人标识
  channel_id: string            // 所属渠道ID
  platform: "feishu" | "dingtalk" | "napcat"
  remote_contact_type: "private" | "group"
  remote_contact_id: string     // 平台原始会话ID
  remote_contact_name?: string  // 群名或用户昵称

  // 回复策略
  reply_mode: "none" | "always" | "on_message"

  // 运行时状态
  has_new_message: boolean        // 收到消息时置true，转发后置false
  last_message_at?: string
  last_reply_message_id?: string
}
```

### 1.3 远程消息入队事件

```
RemoteImQueueEvent {
  id: string                    // 事件唯一标识
  channel_id: string            // 来源渠道
  created_at: string            // 入队时间

  // 目标会话
  conversation_id: string
  session_info: {
    api_config_id: string
    agent_id: string
  }

  // 消息内容（转换为内部格式）
  messages: ChatMessage[]

  // 是否激活助理
  activate_assistant: boolean

  // 发送者信息（用于前端展示）
  sender_info: RemoteSenderInfo
}
```

```
RemoteSenderInfo {
  platform: "feishu" | "dingtalk" | "napcat"
  user_id: string               // 平台用户ID
  display_name: string          // 显示名称
  avatar_url?: string           // 头像URL

  // 群聊时的额外信息
  group_id?: string
  group_name?: string
  is_at_bot?: boolean           // 是否@了机器人
}
```

### 1.4 出站消息

```
OutboundMessage {
  channel_id: string            // 目标渠道
  remote_contact_id: string     // 目标远程联系人

  // 消息内容
  content: OutboundContent[]

  // 回复关联
  reply_to_message_id?: string  // 回复的消息ID
}

OutboundContent =
  | { type: "text", text: string }
  | { type: "image", file_path: string, mime?: string }
  | { type: "audio", file_path: string, duration_ms?: number }
  | { type: "file", file_path: string, file_name: string }
```

---

## 2. 函数签名

### 2.1 渠道管理

```
// 类型别名定义
PlatformCredentials = FeishuCredentials | DingtalkCredentials | NapcatCredentials
ConnectionStatus = ChannelStatus  // 连接状态等同于渠道状态

// 创建渠道配置
create_channel(config: RemoteImChannelConfig): Result<ChannelId, ChannelError>

// 更新渠道配置
update_channel(channel_id: string, config: Partial<RemoteImChannelConfig>): Result<void, ChannelError>

// 删除渠道
delete_channel(channel_id: string): Result<void, ChannelError>

// 获取所有渠道
list_channels(): RemoteImChannelConfig[]

// 启动渠道连接
start_channel(channel_id: string): Result<void, ChannelError>

// 停止渠道连接
stop_channel(channel_id: string): Result<void, ChannelError>

// 获取渠道连接状态
get_channel_status(channel_id: string): ChannelStatus

ChannelStatus = "disconnected" | "connecting" | "connected" | "error"
```

### 2.2 联系人管理

```
// 获取或创建远程联系人映射
get_or_create_remote_contact(
  channel_id: string,
  remote_contact_type: "private" | "group",
  remote_contact_id: string
): Result<RemoteContact, SessionError>

// 获取所有远程联系人
list_all_remote_contacts(): RemoteContact[]

// 获取指定渠道下的远程联系人
list_remote_contacts_by_channel(channel_id: string): RemoteContact[]

// 更新远程联系人的回复策略
update_session_reply_mode(
  session_id: string,
  reply_mode: "none" | "always" | "on_message"
): Result<void, SessionError>

// 标记远程联系人收到新消息
mark_session_has_new_message(session_id: string): void

// 清除远程联系人的新消息标记（转发成功后调用）
clear_session_new_message(session_id: string): void
```

### 2.3 消息入队

```
// 远程消息入队（核心入口）
enqueue_remote_message(
  channel_id: string,
  remote_event: PlatformMessageEvent
): Result<RemoteImQueueEvent, QueueError>

// 获取远程消息队列快照
get_remote_queue_snapshot(): RemoteImQueueEvent[]

// 从远程队列移除事件
remove_remote_queue_event(event_id: string): Result<boolean, QueueError>
```

### 2.4 消息出站

```
// 发送消息到远程渠道
send_to_remote_channel(
  channel_id: string,
  remote_contact_id: string,
  content: OutboundContent[],
  options?: { reply_to?: string }
): Result<PlatformMessageId, SendError>

// 判断是否应该出站（根据远程联系人的回复策略）
should_forward_to_session(
  remote_contact: RemoteContact
): boolean
// 逻辑：
//   none → false
//   always → true
//   on_message → remote_contact.has_new_message
```

### 2.5 用户信息获取

```
// 获取远程用户信息（用于前端展示）
get_remote_user_info(
  channel_id: string,
  user_id: string
): Result<RemoteUserInfo, UserInfoError>

RemoteUserInfo {
  platform: string
  user_id: string
  display_name: string
  avatar_url?: string
}

// 获取远程群组信息
get_remote_group_info(
  channel_id: string,
  group_id: string
): Result<RemoteGroupInfo, GroupInfoError>

RemoteGroupInfo {
  platform: string
  group_id: string
  group_name: string
  member_count?: number
}
```

### 2.6 附件处理

```
// 下载远程附件到本地
download_remote_attachment(
  channel_id: string,
  attachment_meta: PlatformAttachmentMeta
): Result<LocalFilePath, AttachmentError>

PlatformAttachmentMeta {
  message_id: string
  file_type: "image" | "audio" | "video" | "file"
  download_code?: string        // 钉钉专用
  file_key?: string             // 飞书专用
  file_url?: string             // NapCat/通用
  file_name?: string
}

// 上传本地文件到远程平台
upload_to_remote_platform(
  channel_id: string,
  local_path: string,
  file_type: "image" | "audio" | "file"
): Result<PlatformFileKey, UploadError>

PlatformFileKey = string  // 平台返回的文件标识
```

---

## 3. 业务流程

### 3.1 消息入站流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           消息入站流程                                        │
└─────────────────────────────────────────────────────────────────────────────┘

远程平台                渠道层                    主消息流                 前端
   │                     │                         │                      │
   │  1. 消息事件        │                         │                      │
   ├────────────────────>│                         │                      │
   │                     │                         │                      │
   │                     │  2. 解析平台消息        │                      │
   │                     │    - 提取发送者信息     │                      │
   │                     │    - 转换消息格式       │                      │
   │                     │    - 下载附件(如有)     │                      │
   │                     │                         │                      │
   │                     │  3. 查找/创建联系人记录 │                      │
   │                     │                         │                      │
   │                     │  4. 构造 RemoteImQueueEvent                   │
   │                     │                         │                      │
   │                     │  5. 入队                │                      │
   │                     ├────────────────────────>│                      │
   │                     │                         │                      │
   │                     │                         │  6. 批量出队          │
   │                     │                         │                      │
   │                     │                         │  7. 写入历史          │
   │                     │                         │    conversation.messages
   │                     │                         │                      │
   │                     │                         │  8. 推送前端更新      │
   │                     │                         ├─────────────────────>│
   │                     │                         │                      │
   │                     │                         │  9. 检查activate_assistant
   │                     │                         │                      │
   │                     │                         │  [若激活] 10. 启动主助理
   │                     │                         │                      │
   │                     │                         │  [若激活] 11. 流式输出
   │                     │                         ├─────────────────────>│
   │                     │                         │                      │
```

### 3.2 消息出站流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           消息出站流程                                        │
└─────────────────────────────────────────────────────────────────────────────┘

主助理流               渠道出站判断               渠道层                 远程平台
   │                      │                       │                      │
   │  1. 助理产生消息     │                       │                      │
   ├─────────────────────>│                       │                      │
   │                      │                       │                      │
   │                      │  2. 遍历所有启用的渠道 │                      │
   │                      │                       │                      │
   │                      │  3. 对每个渠道判断:    │                      │
   │                      │    - 渠道是否启用?    │                      │
   │                      │    - reply_mode策略?  │                      │
   │                      │    - 是否已回复?      │                      │
   │                      │                       │                      │
   │                      │  [若应转发]           │                      │
   │                      ├──────────────────────>│                      │
   │                      │                       │                      │
   │                      │                       │  4. 转换消息格式      │
   │                      │                       │    - 处理附件上传     │
   │                      │                       │                      │
   │                      │                       │  5. 调用平台API       │
   │                      │                       ├─────────────────────>│
   │                      │                       │                      │
   │                      │                       │  6. 更新联系人状态    │
   │                      │                       │    (has_new_message)  │
   │                      │                       │                      │
```

### 3.3 回复策略判断流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       回复策略判断流程                                        │
└─────────────────────────────────────────────────────────────────────────────┘

输入: remote_contact

                    ┌─────────────────┐
                    │  获取渠道配置   │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ 渠道是否启用?   │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │ 否                          │ 是
              ▼                             ▼
        ┌──────────┐               ┌───────────────────┐
        │ 不转发   │               │ 检查 reply_mode   │
        └──────────┘               └────────┬──────────┘
                                            │
                     ┌──────────────────────┼────────────────────┐
                     │                      │                    │
                     ▼                      ▼                    ▼
              ┌───────────┐          ┌───────────┐        ┌────────────┐
              │   none    │          │  always   │        │ on_message │
              │  不回复   │          │  始终回复  │        │ 有消息才回 │
              └─────┬─────┘          └─────┬─────┘        └─────┬──────┘
                    │                      │                    │
                    ▼                      ▼                    ▼
              ┌──────────┐          ┌──────────┐        ┌────────────────┐
              │ 不转发   │          │ 转发消息 │        │has_new_message?│
              └──────────┘          └──────────┘        └───────┬────────┘
                                                                │
                                                   ┌────────────┴────────────┐
                                                   │ false                   │ true
                                                   ▼                         ▼
                                             ┌──────────┐            ┌──────────────┐
                                             │ 不转发   │            │ 转发消息     │
                                             │          │            │ 置为 false   │
                                             └──────────┘            └──────────────┘
```

### 3.4 远程联系人注册流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       远程联系人注册流程                                      │
└─────────────────────────────────────────────────────────────────────────────┘

首次收到远程消息时:

┌─────────────────┐
│ 收到远程消息    │
└────────┬────────┘
         │
         ▼
┌─────────────────────────┐
│ 查找 RemoteContact      │
│ (channel_id + remote_id)│
└────────┬────────────────┘
         │
    ┌────┴────┐
    │ 存在?   │
    └────┬────┘
         │
    ┌────┴────┐
    │         │
   是        否
    │         │
    │         ▼
    │   ┌──────────────────────────┐
    │   │ 创建新 RemoteContact     │
    │   │ reply_mode = 渠道默认策略 │
    │   │ has_new_message = true   │
    │   └──────────┬───────────────┘
    │              │
    └──────┬───────┘
           │
           ▼
┌──────────────────────────┐
│ 标记 has_new_message     │
│ 消息入队到主消息流       │
└──────────────────────────┘
```

---

## 4. 平台适配接口

### 4.1 平台适配器 Trait

```
PlatformAdapter {
  // 平台标识
  platform: "feishu" | "dingtalk" | "napcat"

  // 连接管理
  connect(credentials: PlatformCredentials): Result<void, ConnectError>
  disconnect(): void
  get_status(): ChannelStatus

  // 消息接收
  on_message(callback: (event: PlatformMessageEvent) => void): void

  // 消息发送
  send_text(session_id: string, text: string): Result<MessageId, SendError>
  send_image(session_id: string, file_key: string): Result<MessageId, SendError>
  send_audio(session_id: string, file_key: string): Result<MessageId, SendError>
  send_file(session_id: string, file_key: string, file_name: string): Result<MessageId, SendError>

  // 文件管理
  upload_file(file_path: string, file_type: FileType): Result<FileKey, UploadError>
  download_file(meta: AttachmentMeta): Result<FilePath, DownloadError>

  // 用户信息
  get_user_info(user_id: string): Result<UserInfo, UserInfoError>
  get_group_info(group_id: string): Result<GroupInfo, GroupInfoError>
}

PlatformMessageEvent {
  message_id: string
  session_type: "private" | "group"
  session_id: string
  sender_id: string
  sender_name: string
  sender_avatar?: string

  content: PlatformContent[]
  is_at_bot: boolean

  timestamp: number
  raw: unknown  // 平台原始事件对象，格式由具体协议定义
}

PlatformContent =
  | { type: "text", text: string }
  | { type: "image", file_key: string, file_url?: string }
  | { type: "audio", file_key: string, duration_ms?: number }
  | { type: "file", file_key: string, file_name: string, file_size?: number }
  | { type: "at", user_id: string }
```

### 4.2 平台差异对照

| 能力 | 飞书 | 钉钉 | NapCat |
|------|------|------|--------|
| 连接方式 | WebSocket长连接 | Stream模式 | 反向WebSocket |
| 流式发送 | ✅ CardKit | ❌ | ❌ |
| 图片发送 | 需先上传获取key | URL或media_id | 直接发送路径/URL |
| 音频格式 | opus | ogg/amr | 任意 |
| 视频发送 | mp4需上传 | ❌ 不支持 | 直接发送 |
| 获取用户信息 | ✅ | ✅ | ✅ |
| 获取群信息 | ✅ | ✅ | ✅ |
| @机制 | 富文本中标记 | at参数 | at消息段 |

---

## 5. 前端设计模板

### 5.1 设置页 - 渠道管理入口

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ⚙ 设置                                                              ×      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 📡 远程IM渠道                                              [+ 添加渠道]│   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │ 🟢 飞书工作群                           [编辑] [删除]        │   │   │
│  │  │ 飞书 · 已连接 · 群聊: 3个                                   │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │ 🟢 钉钉通知机器人                         [编辑] [删除]      │   │   │
│  │  │ 钉钉 · 已连接 · 私聊: 5个                                   │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │ ⚪ QQ群机器人                              [编辑] [删除]      │   │   │
│  │  │ NapCat · 未连接 · 点击编辑配置                              │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 设置页 - 添加/编辑渠道

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  添加远程IM渠道                                                      ×      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  基本信息                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 渠道名称  [                                              ]          │   │
│  │ 平台类型  [ 飞书 ▼ ]                                                 │   │
│  │ 启用渠道  [✓]                                                        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  连接配置 - 飞书                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ App ID      [                                              ]        │   │
│  │ App Secret  [                                              ]        │   │
│  │ 机器人名称  [                                    ] (用于识别@)       │   │
│  │                                                                     │   │
│  │ ℹ 在飞书开放平台创建应用，开启机器人能力后获取以上信息              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  行为策略                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 激活助理    [✓] 收到消息后激活主助理                                 │   │
│  │ 默认回复策略  [ 有消息才回复 ▼ ]                                     │   │
│  │              ○ 不回复          - 仅作为消息来源                      │   │
│  │              ○ 始终回复        - 助理每次发言都转发                  │   │
│  │              ● 有消息才回复    - 对方说过话才转发回复                │   │
│  │ 接收文件    [✓] 接收图片、语音、文件等附件                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  权限策略                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 允许主动发送  [✓] 允许助理主动向该渠道发消息                         │   │
│  │ 允许发送文件  [ ] 允许助理发送文件到该渠道                           │   │
│  │ 显示工具调用  [ ] 在消息中展示工具调用过程                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│                                              [ 取消 ]  [ 保存并连接 ]       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 主聊天界面 - 远程消息展示

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  💬 主助理                                                          ─ □ ×   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        (历史消息区域)                                │   │
│  │                                                                     │   │
│  │  ┌───────────────────────────────────────────────────────────┐     │   │
│  │  │ [飞书] 张三                                    10:30      │     │   │
│  │  │     📱 来源: 飞书 · 工作群                                 │     │   │
│  │  │                                                           │     │   │
│  │  │     这里的报告怎么看？@助理                                │     │   │
│  │  └───────────────────────────────────────────────────────────┘     │   │
│  │                                                                     │   │
│  │  ┌───────────────────────────────────────────────────────────┐     │   │
│  │  │ 助理                                            10:31      │     │   │
│  │  │                                                           │     │   │
│  │  │     报告在左侧导航栏的"数据分析"模块中查看。               │     │   │
│  │  │     ↓ 已同步到: 飞书·工作群                               │     │   │
│  │  └───────────────────────────────────────────────────────────┘     │   │
│  │                                                                     │   │
│  │  ┌───────────────────────────────────────────────────────────┐     │   │
│  │  │ [钉钉] 李四                                    10:35      │     │   │
│  │  │     📱 来源: 钉钉 · 私聊                                   │     │   │
│  │  │                                                           │     │   │
│  │  │     🖼 [图片]                                              │     │   │
│  │  │     这个截图帮我看看                                       │     │   │
│  │  └───────────────────────────────────────────────────────────┘     │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ [🎤] [📎]  输入消息...                                      [发送] │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.4 消息来源标识设计

```
远程消息展示规范:

1. 用户消息头
   ┌─────────────────────────────────────────┐
   │ [平台图标] 用户昵称          时间        │
   │     📱 来源: 平台名 · 会话名             │
   │                                         │
   │     消息内容...                          │
   └─────────────────────────────────────────┘

2. 助理消息同步标记
   ┌─────────────────────────────────────────┐
   │ 助理                          时间      │
   │                                         │
   │     消息内容...                          │
   │     ↓ 已同步到: 平台名·会话名            │
   └─────────────────────────────────────────┘

3. 平台图标
   飞书: 📋 或自定义图标
   钉钉: 💼 或自定义图标
   NapCat: 🐧 或自定义图标

4. 头像处理
   - 远程用户: 使用平台提供的头像URL
   - 无头像时: 显示平台图标 + 昵称首字
```

### 5.5 渠道状态指示器

```
系统托盘/状态栏显示:

┌─────────────────────────────┐
│ 🟢 飞书工作群               │
│ 🟢 钉钉通知机器人           │
│ ⚪ QQ群机器人 (未连接)      │
└─────────────────────────────┘

状态颜色:
  🟢 已连接
  🟡 连接中
  🔴 连接错误
  ⚪ 已禁用
```

---

## 6. 与现有系统的集成点

### 6.1 ChatEventSource 扩展

现有:
```
ChatEventSource = User | Task | Delegate | System
```

扩展:
```
ChatEventSource = User | Task | Delegate | System | RemoteIm
```

### 6.2 ChatPendingEvent 扩展

现有:
```
ChatPendingEvent {
  id: string
  conversation_id: string
  created_at: string
  source: ChatEventSource
  messages: ChatMessage[]
  activate_assistant: boolean
  session_info: ChatSessionInfo
}
```

扩展:
```
ChatPendingEvent {
  ...现有字段
  sender_info?: RemoteSenderInfo  // 新增：远程发送者信息
}
```

### 6.3 出站判断钩子

在助理流式输出完成后，需要调用出站判断:

```
// 在现有流式输出完成后
on_assistant_message_complete(message: ChatMessage) {
  // 获取所有远程联系人
  all_sessions = list_all_remote_contacts()

  for remote_contact in all_sessions {
    channel = get_channel(remote_contact.channel_id)

    // 跳过已禁用的渠道
    if not channel.enabled {
      continue
    }

    // 判断是否应该转发
    if should_forward_to_session(remote_contact) {
      result = send_to_remote_channel(
        remote_contact.channel_id,
        remote_contact.remote_contact_id,
        convert(message)
      )

      // 转发成功后，重置标记
      if result.is_ok() and remote_contact.reply_mode == "on_message" {
        remote_contact.has_new_message = false
      }
    }
  }
}
```

---

## 7. 存储设计

### 7.1 配置存储位置

```
AppData {
  ...现有字段

  // 新增
  remote_im_channels: RemoteImChannelConfig[]
}
```

### 7.2 联系人存储

```
RemoteContactStore {
  // 文件: remote_contacts.json
  contacts: RemoteContact[]

  // 索引
  by_channel: Map<channel_id, RemoteContact[]>
  by_remote: Map<channel_id + remote_contact_id, RemoteContact>
}
```

### 7.3 运行时状态存储

```
ChannelRuntimeState {
  channel_id: string
  status: ConnectionStatus
  last_error?: string
  connected_at?: string
  active_sessions: Set<remote_contact_id>
}
```

---

## 8. 错误处理

### 8.1 错误类型

```
ChannelError {
  type: "config_invalid" | "connect_failed" | "auth_failed" | "rate_limited"
  message: string
  platform?: string
  retry_after?: number
}

SessionError {
  type: "not_found" | "bind_failed" | "invalid_remote_id"
  message: string
}

SendError {
  type: "channel_offline" | "permission_denied" | "file_too_large" | "content_blocked"
  message: string
  platform_error?: unknown
}

QueueError {
  type: "queue_full" | "enqueue_failed" | "invalid_event"
  message: string
}

UserInfoError {
  type: "user_not_found" | "permission_denied" | "api_error"
  message: string
  platform?: string
  platform_error?: unknown
}

GroupInfoError {
  type: "group_not_found" | "permission_denied" | "api_error"
  message: string
  platform?: string
  platform_error?: unknown
}

AttachmentError {
  type: "file_not_found" | "download_failed" | "unsupported_format" | "quota_exceeded"
  message: string
  platform?: string
  platform_error?: unknown
}

UploadError {
  type: "upload_failed" | "file_too_large" | "unsupported_format" | "quota_exceeded" | "permission_denied"
  message: string
  max_size?: number
  platform?: string
  platform_error?: unknown
}

ConnectError {
  type: "connection_failed" | "auth_failed" | "config_invalid" | "timeout" | "network_error"
  message: string
  platform?: string
  retry_after?: number
  platform_error?: unknown
}
```

### 8.2 重连策略

```
ReconnectPolicy {
  max_retries: 5
  base_delay_ms: 1000
  max_delay_ms: 60000
  backoff_multiplier: 2

  // 飞书/钉钉: 由SDK内部处理
  // NapCat: 应用层处理
}
```

---

*文档版本: 1.0*
*对应设计计划: 20260314_远程IM渠道抽象与主消息流集成设计计划.md*
*接口参考: 远程IM渠道接口技术指南.md*
