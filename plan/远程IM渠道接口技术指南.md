# 远程IM渠道接口技术指南

> 本文档为《20260314_远程IM渠道抽象与主消息流集成设计计划》提供详尽的技术支持，涵盖飞书、钉钉、NapCat(QQ)三个平台的接口对接细节。
>
> 文档来源：官方API文档 + AstrBot项目实现代码分析

---

## 目录

1. [飞书机器人](#1-飞书机器人)
2. [钉钉机器人](#2-钉钉机器人)
3. [NapCat (QQ机器人)](#3-napcat-qq机器人)
4. [平台对比总结](#4-平台对比总结)

---

## 1. 飞书机器人

### 1.1 平台概述

飞书机器人使用**应用机器人**类型：

| 类型 | 特点 | 适用场景 |
|------|------|----------|
| **应用机器人** | 在开发者后台创建应用，开启机器人能力，需管理员审核 | 功能完整的交互式机器人，支持双向消息、群管理、调用服务端API |

> **不支持自定义机器人**: 自定义机器人仅支持Webhook单向推送，需要公网IP接收回调，不符合桌面应用的使用场景。

### 1.2 连接方式

飞书应用机器人支持以下事件订阅方式：

| 方式 | 说明 | 支持情况 |
|------|------|----------|
| **WebSocket长连接** | 通过SDK建立持久连接，实时接收事件 | ✅ 推荐，无需公网IP |

> **为什么不支持 Webhook 方式**
> Webhook 需要配置公网可访问的 HTTP 回调地址，这要求用户拥有公网 IP 或使用内网穿透服务。作为桌面应用，大多数用户不具备公网网络条件，因此不支持此方式。WebSocket 长连接由 SDK 内部建立，应用主动连接飞书服务器，无需暴露本地端口，更适合桌面应用场景。

### 1.3 核心API接口

#### 1.3.1 消息发送

**接口地址**
```
POST https://open.feishu.cn/open-apis/im/v1/messages
```

**请求头**
```
Authorization: Bearer {tenant_access_token}
Content-Type: application/json
```

**查询参数**
| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| receive_id_type | string | 是 | 接收者ID类型：`open_id`/`user_id`/`union_id`/`chat_id`/`email` |

**请求体**
```json
{
  "receive_id": "oc_xxx",           // 接收者ID
  "msg_type": "text",               // 消息类型
  "content": "{\"text\":\"内容\"}"   // JSON字符串格式的消息内容
}
```

**支持的消息类型**

| msg_type | 说明 | content格式 |
|----------|------|-------------|
| `text` | 文本消息 | `{"text": "文本内容"}` |
| `post` | 富文本消息 | `{"zh_cn": {"title": "标题", "content": [[{"tag": "text", "text": "内容"}]]}}` |
| `image` | 图片消息 | `{"image_key": "img_xxx"}` |
| `file` | 文件消息 | `{"file_key": "file_xxx"}` |
| `audio` | 音频消息 | `{"file_key": "file_xxx"}` |
| `media` | 视频消息 | `{"file_key": "file_xxx", "duration": 60000}` |
| `interactive` | 卡片消息 | 卡片JSON结构 |
| `share_chat` | 群名片 | `{"share_chat_id": "oc_xxx"}` |
| `share_user` | 个人名片 | `{"share_user_id": "ou_xxx"}` |

#### 1.3.2 消息回复

**接口地址**
```
POST https://open.feishu.cn/open-apis/im/v1/messages/{message_id}/reply
```

**请求体**
```json
{
  "content": "{\"text\":\"回复内容\"}",
  "msg_type": "text"
}
```

#### 1.3.3 文件上传

**上传图片**
```
POST https://open.feishu.cn/open-apis/im/v1/images
Content-Type: multipart/form-data

参数：
- image_type: "message"
- image: 文件二进制
```

**上传文件**
```
POST https://open.feishu.cn/open-apis/im/v1/files
Content-Type: multipart/form-data

参数：
- file_type: 文件类型 (stream/opus/mp4等)
- file_name: 文件名
- file: 文件二进制
- duration: 媒体时长(毫秒)，可选
```

#### 1.3.4 获取访问凭证

```
POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal

请求体：
{
  "app_id": "cli_xxx",
  "app_secret": "xxx"
}

响应：
{
  "tenant_access_token": "t-xxx",
  "expire": 7200
}
```

### 1.4 事件订阅

#### 1.4.1 事件结构 (v2.0)

```json
{
  "schema": "2.0",
  "header": {
    "event_id": "xxx",                    // 事件唯一标识
    "event_type": "im.message.receive_v1", // 事件类型
    "create_time": "1603977298000000",    // 事件时间
    "token": "xxx",                       // Verification Token
    "app_id": "cli_xxx",
    "tenant_key": "xxx"
  },
  "event": {
    // 事件详细数据
  }
}
```

#### 1.4.2 核心事件类型

| 事件类型 | 说明 |
|----------|------|
| `im.message.receive_v1` | 接收消息 |
| `im.message.message_read_v1` | 消息已读 |
| `im.chat.created_v1` | 群聊创建 |
| `im.chat.member.added_v1` | 群成员增加 |
| `im.chat.member.deleted_v1` | 群成员减少 |

#### 1.4.3 消息接收事件详情

```json
{
  "event": {
    "sender": {
      "sender_id": {
        "open_id": "ou_xxx",
        "user_id": "xxx"
      },
      "sender_type": "user",
      "tenant_key": "xxx"
    },
    "message": {
      "message_id": "om_xxx",
      "root_id": "",
      "parent_id": "",
      "create_time": "1609329484000",
      "chat_id": "oc_xxx",
      "message_type": "text",
      "content": "{\"text\":\"消息内容\"}",
      "mentions": [
        {
          "key": "@_user_1",
          "id": {
            "open_id": "ou_xxx"
          },
          "name": "张三"
        }
      ]
    }
  }
}
```

### 1.5 消息内容解析

#### 1.5.1 文本消息
```json
{"text": "纯文本内容"}
```

#### 1.5.2 富文本消息 (post)
```json
{
  "zh_cn": {
    "title": "标题",
    "content": [
      [
        {"tag": "text", "text": "普通文本"},
        {"tag": "at", "user_id": "ou_xxx"},
        {"tag": "img", "image_key": "img_xxx"}
      ]
    ]
  }
}
```

#### 1.5.3 图片消息
```json
{"image_key": "img_xxx"}
```

#### 1.5.4 文件消息
```json
{"file_key": "file_xxx"}
```

### 1.6 AstrBot 实现参考

**核心文件**
- `astrbot/core/platform/sources/lark/lark_adapter.py` - 平台适配器
- `astrbot/core/platform/sources/lark/lark_event.py` - 事件处理
- `astrbot/core/platform/sources/lark/server.py` - Webhook服务

**关键实现**

```python
# 初始化客户端 (lark_adapter.py)
import lark_oapi as lark

self.client = lark.ws.Client(
    app_id=self.appid,
    app_secret=self.appsecret,
    domain=lark.FEISHU_DOMAIN,
    event_handler=self.event_handler,
)

self.lark_api = lark.Client.builder() \
    .app_id(self.appid) \
    .app_secret(self.appsecret) \
    .build()

# 发送消息 (lark_event.py)
request = ReplyMessageRequest.builder() \
    .message_id(reply_message_id) \
    .request_body(
        ReplyMessageRequestBody.builder()
        .content(content)
        .msg_type(msg_type)
        .build()
    ) \
    .build()
response = await self.lark_api.im.v1.message.areply(request)

# 上传文件
request = CreateFileRequest.builder() \
    .request_body(
        CreateFileRequestBody.builder()
        .file_type(file_type)
        .file_name(file_name)
        .file(file_obj)
        .build()
    ) \
    .build()
response = await self.lark_api.im.v1.file.acreate(request)
```

### 1.7 官方文档来源

| 文档 | 地址 |
|------|------|
| 机器人概述 | https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/bot-v3/bot-overview |
| 发送消息API | https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/im-v1/message/create |
| 事件概述 | https://open.feishu.cn/document/ukTMukTMukTM/uUTNz4SN1MjL1UzM |
| 自定义机器人 | https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot |
| lark-oapi SDK | https://github.com/larksuite/oapi-sdk-python |

---

## 2. 钉钉机器人

### 2.1 平台概述

钉钉机器人使用**企业内部机器人**类型：

| 类型 | 特点 | 适用场景 |
|------|------|----------|
| **企业内部机器人** | 在开发者后台创建应用，开启机器人能力 | 企业内部使用，支持双向消息、Stream模式 |

> **不支持自定义机器人**: 自定义机器人仅支持Webhook单向推送，需要公网IP接收回调，不符合桌面应用的使用场景。

### 2.2 连接方式

钉钉企业内部机器人支持以下连接方式：

| 方式 | 说明 | 支持情况 |
|------|------|----------|
| **Stream模式** | 通过WebSocket长连接接收消息，SDK内部建立连接 | ✅ 推荐，无需公网IP |

```python
import dingtalk_stream

# 创建凭证
credential = dingtalk_stream.Credential(client_id, client_secret)

# 创建客户端
client = dingtalk_stream.DingTalkStreamClient(credential)

# 注册消息处理器
class MyHandler(dingtalk_stream.ChatbotHandler):
    async def process(self, message: dingtalk_stream.CallbackMessage):
        im = dingtalk_stream.ChatbotMessage.from_dict(message.data)
        # 处理消息
        return AckMessage.STATUS_OK, "OK"

client.register_callback_handler(
    dingtalk_stream.ChatbotMessage.TOPIC,
    MyHandler()
)

# 启动
client.start()
```

> **为什么不支持 Webhook 方式**
> 与飞书类似，Webhook 需要公网可访问的 HTTP 端点。桌面应用通常运行在 NAT 网络环境中，不具备公网访问条件。Stream 模式通过 WebSocket 主动连接钉钉服务器，无需用户配置网络，开箱即用。

### 2.3 核心API接口

#### 2.3.1 获取访问凭证

```
POST https://api.dingtalk.com/v1.0/oauth2/accessToken

请求体：
{
  "appKey": "xxx",
  "appSecret": "xxx"
}

响应：
{
  "data": {
    "accessToken": "xxx",
    "expireIn": 7200
  }
}
```

#### 2.3.2 发送群消息

```
POST https://api.dingtalk.com/v1.0/robot/groupMessages/send

请求头：
x-acs-dingtalk-access-token: {access_token}

请求体：
{
  "msgKey": "sampleText",
  "msgParam": "{\"content\":\"消息内容\"}",
  "openConversationId": "cid_xxx",
  "robotCode": "xxx"
}
```

#### 2.3.3 发送单聊消息

```
POST https://api.dingtalk.com/v1.0/robot/oToMessages/batchSend

请求头：
x-acs-dingtalk-access-token: {access_token}

请求体：
{
  "robotCode": "xxx",
  "userIds": ["staffId1"],
  "msgKey": "sampleText",
  "msgParam": "{\"content\":\"消息内容\"}"
}
```

#### 2.3.4 消息类型 (msgKey)

| msgKey | 说明 | msgParam格式 |
|--------|------|-------------|
| `sampleText` | 文本消息 | `{"content": "文本内容"}` |
| `sampleMarkdown` | Markdown消息 | `{"title": "标题", "text": "## Markdown内容"}` |
| `sampleImageMsg` | 图片消息 | `{"photoURL": "图片URL或media_id"}` |
| `sampleAudio` | 音频消息 | `{"mediaId": "xxx", "duration": "1000"}` |
| `sampleFile` | 文件消息 | `{"mediaId": "xxx", "fileName": "xxx.pdf", "fileType": "pdf"}` |
| `sampleLink` | 链接消息 | `{"title": "标题", "text": "描述", "picUrl": "xxx", "messageUrl": "xxx"}` |
| `sampleActionCard` | 卡片消息 | 卡片JSON结构 |

#### 2.3.5 上传媒体文件

```
POST https://oapi.dingtalk.com/media/upload?access_token={token}&type={type}

Content-Type: multipart/form-data

参数：
- media: 文件二进制
- type: 文件类型 (image/voice/file)
```

### 2.4 消息接收

#### 2.4.1 消息结构

通过Stream模式接收的消息对象：

```python
class ChatbotMessage:
    message_id: str              # 消息ID
    conversation_type: str       # 会话类型: "1"=单聊, "2"=群聊
    conversation_id: str         # 会话ID
    sender_id: str               # 发送者ID
    sender_nick: str             # 发送者昵称
    sender_staff_id: str         # 发送者员工ID (私聊)
    chatbot_user_id: str         # 机器人ID
    message_type: str            # 消息类型
    create_at: int               # 创建时间戳
    text: TextContent            # 文本内容
    image_content: ImageContent  # 图片内容
    rich_text_content: RichTextContent  # 富文本内容
    at_users: list               # @用户列表
    robot_code: str              # 机器人码
    extensions: dict             # 扩展信息
```

#### 2.4.2 消息类型解析

| message_type | 说明 | 关键字段 |
|--------------|------|----------|
| `text` | 文本消息 | `text.content` |
| `picture` | 图片消息 | `image_content.download_code` |
| `richText` | 富文本消息 | `rich_text_content.rich_text_list` |
| `audio` / `voice` | 语音消息 | `extensions.downloadCode` |
| `file` | 文件消息 | `extensions.downloadCode`, `extensions.fileName` |

#### 2.4.3 下载文件

```
POST https://api.dingtalk.com/v1.0/robot/messageFiles/download

请求头：
x-acs-dingtalk-access-token: {access_token}

请求体：
{
  "downloadCode": "xxx",
  "robotCode": "xxx"
}

响应：
{
  "downloadUrl": "https://xxx"
}
```

### 2.5 AstrBot 实现参考

**核心文件**
- `astrbot/core/platform/sources/dingtalk/dingtalk_adapter.py` - 平台适配器
- `astrbot/core/platform/sources/dingtalk/dingtalk_event.py` - 事件处理

**关键实现**

```python
# 初始化Stream客户端 (dingtalk_adapter.py)
import dingtalk_stream

class AstrCallbackClient(dingtalk_stream.ChatbotHandler):
    async def process(self, message: dingtalk_stream.CallbackMessage):
        im = dingtalk_stream.ChatbotMessage.from_dict(message.data)
        abm = await self.convert_msg(im)
        await self.handle_msg(abm)
        return AckMessage.STATUS_OK, "OK"

credential = dingtalk_stream.Credential(client_id, client_secret)
client = dingtalk_stream.DingTalkStreamClient(credential)
client.register_callback_handler(
    dingtalk_stream.ChatbotMessage.TOPIC,
    AstrCallbackClient()
)

# 发送群消息
async def _send_group_message(self, open_conversation_id, robot_code, msg_key, msg_param):
    payload = {
        "msgKey": msg_key,
        "msgParam": json.dumps(msg_param, ensure_ascii=False),
        "openConversationId": open_conversation_id,
        "robotCode": robot_code,
    }
    headers = {
        "Content-Type": "application/json",
        "x-acs-dingtalk-access-token": access_token,
    }
    await session.post(
        "https://api.dingtalk.com/v1.0/robot/groupMessages/send",
        headers=headers,
        json=payload
    )

# 上传媒体文件
async def upload_media(self, file_path, media_type):
    form = aiohttp.FormData()
    form.add_field("media", file_bytes, filename=file_name)
    async with session.post(
        f"https://oapi.dingtalk.com/media/upload?access_token={token}&type={media_type}",
        data=form
    ) as resp:
        data = await resp.json()
        return data.get("media_id", "")
```

### 2.6 官方文档来源

| 文档 | 地址 |
|------|------|
| 机器人概述 | https://open.dingtalk.com/document/development/development-robot-overview |
| 自定义机器人接入 | https://open.dingtalk.com/document/robots/custom-robot-access |
| Stream模式介绍 | https://open.dingtalk.com/document/development/introduction-to-stream-mode |
| 消息类型 | https://open.dingtalk.com/document/development/robot-message-type |
| 发送群消息 | https://open.dingtalk.com/document/development/robot-sends-a-group-message |
| dingtalk-stream SDK | https://github.com/open-dingtalk/dingtalk-stream-sdk-python |

---

## 3. NapCat (QQ机器人)

### 3.1 平台概述

NapCat 是基于 NTQQ 的现代化 Bot 协议端实现，实现了 OneBot 11 标准。

**核心特性**
- 完整实现 OneBot 11 API
- 支持多种连接方式
- 支持扩展 API (NapCat特有)

### 3.2 连接方式

| 方式 | 说明 | 支持情况 |
|------|------|----------|
| **反向WebSocket** | NapCat主动连接到应用的WS服务 | ✅ 推荐 |

> **关于公网IP的问题**
> 
> NapCat 是否需要公网IP，取决于部署位置：
> 
> | NapCat部署位置 | 是否需要公网 | 说明 |
> |----------------|-------------|------|
> | 和应用在同一台电脑 | ❌ 不需要 | 通过 `127.0.0.1` 或 `localhost` 通信 |
> | 和应用在同一局域网 | ❌ 不需要 | 通过内网IP通信 |
> | 部署在远程服务器 | ✅ 需要 | 服务器需要能访问你的应用 |
> 
> **推荐做法**: 将 NapCat 和你的应用部署在同一台电脑或同一局域网内，完全不需要公网IP。
> 
> **为什么不支持其他连接方式**
> - **HTTP POST事件上报**: 需要公网可访问的HTTP端点，要求公网IP
> - **正向WebSocket**: 需要应用主动连接NapCat，如果NapCat在远程服务器则同样需要处理网络穿透问题

### 3.3 OneBot 11 核心API

#### 3.3.1 消息发送

**发送私聊消息**
```
POST /send_private_msg

参数：
{
  "user_id": 123456789,     // QQ号
  "message": "消息内容",    // 消息内容(字符串或消息段数组)
  "auto_escape": false      // 是否解析CQ码
}
```

**发送群消息**
```
POST /send_group_msg

参数：
{
  "group_id": 123456789,    // 群号
  "message": "消息内容",
  "auto_escape": false
}
```

**发送消息 (通用)**
```
POST /send_msg

参数：
{
  "message_type": "private", // private/group
  "user_id": 123456789,
  "group_id": 123456789,
  "message": "消息内容"
}
```

#### 3.3.2 消息段格式

OneBot 11 使用消息段数组表示消息：

```json
[
  {"type": "text", "data": {"text": "普通文本"}},
  {"type": "at", "data": {"qq": "123456789"}},
  {"type": "image", "data": {"file": "http://xxx.jpg"}},
  {"type": "record", "data": {"file": "base64://xxx"}},
  {"type": "video", "data": {"file": "file:///path/to/video.mp4"}},
  {"type": "file", "data": {"file": "file:///path/to/file.pdf"}}
]
```

**常用消息段类型**

| type | 说明 | data字段 |
|------|------|----------|
| `text` | 文本 | `text`: 文本内容 |
| `at` | @某人 | `qq`: QQ号, `name`: 名称 |
| `image` | 图片 | `file`: 文件路径/URL/Base64, `url`: 图片URL |
| `record` | 语音 | `file`: 文件路径/URL/Base64 |
| `video` | 视频 | `file`: 文件路径/URL/Base64 |
| `file` | 文件 | `file`: 文件路径/URL |
| `face` | 表情 | `id`: 表情ID |
| `reply` | 回复 | `id`: 消息ID |
| `node` | 合并转发节点 | `id`: 消息ID 或自定义内容 |

#### 3.3.3 获取信息

| API | 说明 | 参数 |
|-----|------|------|
| `get_login_info` | 获取登录号信息 | 无 |
| `get_stranger_info` | 获取陌生人信息 | `user_id` |
| `get_friend_list` | 获取好友列表 | 无 |
| `get_group_info` | 获取群信息 | `group_id` |
| `get_group_list` | 获取群列表 | 无 |
| `get_group_member_info` | 获取群成员信息 | `group_id`, `user_id` |
| `get_group_member_list` | 获取群成员列表 | `group_id` |

#### 3.3.4 群管理

| API | 说明 | 参数 |
|-----|------|------|
| `set_group_kick` | 踢出群成员 | `group_id`, `user_id` |
| `set_group_ban` | 禁言群成员 | `group_id`, `user_id`, `duration` |
| `set_group_whole_ban` | 全员禁言 | `group_id`, `enable` |
| `set_group_admin` | 设置管理员 | `group_id`, `user_id`, `enable` |
| `set_group_card` | 设置群名片 | `group_id`, `user_id`, `card` |
| `set_group_name` | 设置群名 | `group_id`, `group_name` |

#### 3.3.5 文件操作

| API | 说明 | 参数 |
|-----|------|------|
| `upload_group_file` | 上传群文件 | `group_id`, `file`, `name` |
| `upload_private_file` | 上传私聊文件 | `user_id`, `file`, `name` |
| `get_image` | 获取图片 | `file` |
| `get_record` | 获取语音 | `file` |

### 3.4 事件上报

#### 3.4.1 事件结构

```json
{
  "time": 1609329484,
  "self_id": 123456789,
  "post_type": "message",       // message/notice/request
  "message_type": "group",      // private/group
  "sub_type": "normal",
  "user_id": 987654321,
  "group_id": 111111111,
  "message_id": 123456,
  "message": [
    {"type": "text", "data": {"text": "消息内容"}}
  ],
  "raw_message": "消息内容",
  "font": 0,
  "sender": {
    "user_id": 987654321,
    "nickname": "昵称",
    "card": "群名片",
    "role": "member"             // owner/admin/member
  }
}
```

#### 3.4.2 事件类型

| post_type | 子类型 | 说明 |
|-----------|--------|------|
| `message` | `private`/`group` | 消息事件 |
| `notice` | `group_increase` | 群成员增加 |
| `notice` | `group_decrease` | 群成员减少 |
| `notice` | `group_admin` | 群管理员变动 |
| `notice` | `group_ban` | 群禁言 |
| `notice` | `friend_add` | 好友添加 |
| `notice` | `poke` | 戳一戳 |
| `request` | `friend` | 好友请求 |
| `request` | `group` | 入群请求 |

### 3.5 NapCat 扩展API

NapCat 在 OneBot 11 基础上提供了额外API：

| API | 说明 |
|-----|------|
| `send_forward_msg` | 发送合并转发 |
| `get_friend_msg_history` | 获取私聊历史记录 |
| `mark_private_msg_as_read` | 标记私聊已读 |
| `mark_group_msg_as_read` | 标记群聊已读 |
| `get_ai_characters` | 获取AI语音角色 |
| `send_group_ai_record` | 发送AI语音 |
| `friend_poke` | 私聊戳一戳 |
| `group_poke` | 群聊戳一戳 |
| `set_online_status` | 设置在线状态 |
| `set_qq_avatar` | 设置头像 |
| `get_file` | 获取文件信息 |

### 3.6 AstrBot 实现参考

**核心文件**
- `astrbot/core/platform/sources/aiocqhttp/aiocqhttp_platform_adapter.py` - 平台适配器
- `astrbot/core/platform/sources/aiocqhttp/aiocqhttp_message_event.py` - 消息事件

**关键实现**

```python
# 初始化反向WebSocket (aiocqhttp_platform_adapter.py)
from aiocqhttp import CQHttp

self.bot = CQHttp(
    use_ws_reverse=True,
    import_name="aiocqhttp",
    api_timeout_sec=180,
    access_token=token
)

@self.bot.on_message("group")
async def group(event: Event):
    abm = await self.convert_message(event)
    await self.handle_msg(abm)

@self.bot.on_message("private")
async def private(event: Event):
    abm = await self.convert_message(event)
    await self.handle_msg(abm)

# 发送消息 (aiocqhttp_message_event.py)
async def send_message(cls, bot, message_chain, event, is_group, session_id):
    # 解析消息链为OneBot格式
    ret = await cls._parse_onebot_json(message_chain)
    
    if is_group:
        await bot.send_group_msg(group_id=int(session_id), message=ret)
    else:
        await bot.send_private_msg(user_id=int(session_id), message=ret)

# 消息段转换
async def _from_segment_to_dict(segment):
    if isinstance(segment, Image):
        bs64 = await segment.convert_to_base64()
        return {
            "type": "image",
            "data": {"file": f"base64://{bs64}"}
        }
    elif isinstance(segment, Plain):
        return {"type": "text", "data": {"text": segment.text}}
    # ... 其他类型
```

### 3.7 官方文档来源

| 文档 | 地址 |
|------|------|
| NapCat官网 | https://napneko.github.io/ |
| NapCat API文档 | https://napneko.github.io/develop/api/doc |
| NapCat 事件文档 | https://napneko.github.io/develop/event |
| OneBot 11 标准 | https://github.com/botuniverse/onebot-11 |
| OneBot 11 消息段 | https://github.com/botuniverse/onebot-11/blob/master/message/segment.md |
| aiocqhttp SDK | https://github.com/nonebot/aiocqhttp |

---

## 4. 平台对比总结

### 4.1 连接方式对比

| 特性 | 飞书 | 钉钉 | NapCat |
|------|------|------|--------|
| 推荐连接方式 | WebSocket长连接 | Stream模式 | 反向WebSocket |
| 需要公网IP | ❌ 从不需要 | ❌ 从不需要 | ❌ 本地部署不需要 |
| SDK | lark-oapi | dingtalk-stream | aiocqhttp |
| 连接方向 | 应用→平台服务器 | 应用→平台服务器 | NapCat→应用 |

> **统一原则**: 所有平台均支持无需公网IP的本地部署方案。
> - 飞书/钉钉：应用主动连接平台服务器，永远不需要公网
> - NapCat：推荐与应用部署在同一机器或局域网，通过本地/内网通信

### 4.2 消息类型对比

| 消息类型 | 飞书 | 钉钉 | NapCat |
|----------|------|------|--------|
| 文本 | ✅ | ✅ | ✅ |
| 富文本/Markdown | ✅ post | ✅ markdown | ❌ |
| 图片 | ✅ 需上传 | ✅ URL/media_id | ✅ 多种来源 |
| 文件 | ✅ 需上传 | ✅ 需上传 | ✅ |
| 音频 | ✅ 需上传opus | ✅ 需上传ogg/amr | ✅ |
| 视频 | ✅ 需上传mp4 | ❌ | ✅ |
| 卡片 | ✅ 功能强大 | ✅ ActionCard | ❌ |
| 合并转发 | ❌ | ❌ | ✅ |
| @功能 | ✅ 富文本中 | ✅ at参数 | ✅ at消息段 |

### 4.3 事件能力对比

| 事件类型 | 飞书 | 钉钉 | NapCat |
|----------|------|------|--------|
| 消息接收 | ✅ | ✅ | ✅ |
| 群成员变动 | ✅ | ✅ | ✅ |
| 好友请求 | ❌ | ❌ | ✅ |
| 入群请求 | ❌ | ❌ | ✅ |
| 戳一戳 | ❌ | ❌ | ✅ |
| 消息已读 | ✅ | ❌ | ❌ |

### 4.4 文件处理对比

| 特性 | 飞书 | 钉钉 | NapCat |
|------|------|------|--------|
| 文件上传 | 需先上传获取key | 需先上传获取media_id | 直接发送文件路径 |
| 图片格式 | 任意格式 | 任意格式 | 任意格式 |
| 音频格式 | 需转为opus | 需转为ogg/amr | 任意格式 |
| 视频格式 | 需转为mp4 | 不支持 | 任意格式 |
| 文件下载 | 通过API下载 | 通过downloadCode | 通过API下载 |

### 4.5 推荐对接方案

| 平台 | 推荐连接方式 | 推荐SDK | 流式支持 |
|------|--------------|---------|----------|
| 飞书 | WebSocket长连接 | lark-oapi | ✅ CardKit流式卡片 |
| 钉钉 | Stream模式 | dingtalk-stream | ❌ 需缓冲发送 |
| NapCat | 反向WebSocket | aiocqhttp | ❌ 需缓冲发送 |

---

## 附录：配置示例

### A.1 飞书配置示例

```json
{
  "platform": "feishu",
  "app_id": "cli_xxx",
  "app_secret": "xxx",
  "lark_bot_name": "机器人名称",
  "lark_connection_mode": "socket",
  "domain": "https://open.feishu.cn"
}
```

### A.2 钉钉配置示例

```json
{
  "platform": "dingtalk",
  "client_id": "xxx",
  "client_secret": "xxx"
}
```

### A.3 NapCat配置示例

```json
{
  "platform": "napcat",
  "ws_reverse_host": "0.0.0.0",
  "ws_reverse_port": 8080,
  "ws_reverse_token": "可选的token"
}
```

---

*文档生成时间: 2026-03-14*
*参考项目: AstrBot (https://github.com/Soulter/AstrBot)*
