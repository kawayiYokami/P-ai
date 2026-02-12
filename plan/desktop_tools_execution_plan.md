# 桌面自动化三工具执行计划（Screenshot / Operate / Wait）

## 1. 项目目标

在 Windows 桌面环境实现并上线 3 个可编排工具：

1. `screenshot`：默认全桌面截图，支持多显示器与区域截图。
2. `operate`：统一封装键盘、鼠标、滚轮、文本粘贴等操作能力。
3. `wait`：统一等待能力，支持显式延迟，作为编排稳定器。

最终满足“LLM 可根据文本目标优先点击 + 操作后可配置延迟与重截图”的闭环。

---

## 2. 已对齐约束

1. 截图方案固定：`xcap`。
2. OCR 不在本阶段实现（已有成熟方案），但操作工具必须预留 `text_target` 接口。
3. 操作工具必须覆盖：
   - 键盘按键、组合键、长按
   - 鼠标左/右/中键点击与长按
   - 滚轮上/下滚动
   - 文本粘贴
4. 点击目标支持文本，且“优先文本目标”；除非明确指定为按钮目标。
5. 每次操作后支持：
   - `post_delay_ms`（操作后等待）
   - `rescreenshot`（是否自动重截图）

---

## 3. 工具边界与职责

### 3.1 screenshot（截图工具）

职责：

1. 捕获全桌面（默认）或指定显示器/区域。
2. 生成标准输出（图片路径、尺寸、边界、耗时、时间戳）。
3. 输出坐标必须与 `operate` 使用同一物理像素坐标体系。

不负责：

1. OCR 文本识别本身。
2. 点击决策逻辑。

### 3.2 operate（操作工具）

职责：

1. 执行输入行为：键盘、鼠标、滚轮、粘贴。
2. 支持目标定位策略：`text` / `button` / `point`。
3. 统一“操作后等待 + 可选重截图”策略。
4. 对外返回结构化执行结果（动作、参数、耗时、截图信息）。

不负责：

1. OCR 模型推理细节（仅接收上游文本定位结果或目标文本）。

### 3.3 wait（等待工具）

职责：

1. 显式等待：`sleep(ms)`。
2. 作为编排步骤保证操作节奏稳定。
3. 返回标准耗时与超时错误信息。

---

## 4. 目标定位策略（operate）

优先级规则：

1. 当 `target.type = text` 时，优先按文本定位点击。
2. 当 `target.type = button` 时，按按钮定位策略点击。
3. 当 `target.type = point` 时，按坐标直接点击。

文本点击约束：

1. 支持同名文本候选列表。
2. 默认按“最高置信度 + 最近空间邻近”选点。
3. 无唯一目标时返回 `AMBIGUOUS_TARGET`，由上层决定重试策略。

---

## 5. 接口草案（MVP）

## 5.1 screenshot.request

```json
{
  "mode": "desktop|monitor|region",
  "monitor_id": 65537,
  "region": { "x": 0, "y": 0, "width": 1280, "height": 720 },
  "save_path": "optional"
}
```

## 5.2 screenshot.response

```json
{
  "ok": true,
  "path": "runtime/screenshots/xxx.png",
  "width": 2560,
  "height": 1440,
  "bounds": { "x": 0, "y": 0, "width": 2560, "height": 1440 },
  "elapsed_ms": 42,
  "timestamp": "2026-02-12T00:00:00Z"
}
```

## 5.3 operate.request

```json
{
  "action": "click|double_click|mouse_down|mouse_up|scroll|drag|key_tap|key_down|key_up|hotkey|paste_text",
  "target": {
    "type": "text|button|point",
    "text": "确定",
    "point": { "x": 100, "y": 200 }
  },
  "mouse": {
    "button": "left|right|middle",
    "hold_ms": 0,
    "scroll_delta": 120
  },
  "keyboard": {
    "keys": ["ctrl", "v"],
    "hold_ms": 0
  },
  "text": "要粘贴的内容",
  "post_delay_ms": 300,
  "rescreenshot": true
}
```

## 5.4 operate.response

```json
{
  "ok": true,
  "action": "click",
  "resolved_target": { "x": 320, "y": 560, "source": "text" },
  "elapsed_ms": 18,
  "post_wait_ms": 300,
  "screenshot": {
    "path": "runtime/screenshots/after_xxx.png",
    "width": 2560,
    "height": 1440
  }
}
```

## 5.5 wait.request / response

```json
{
  "mode": "sleep",
  "ms": 500
}
```

```json
{
  "ok": true,
  "waited_ms": 500,
  "elapsed_ms": 501
}
```

---

## 6. 技术选型与模块落位

依赖：

1. `xcap`：截图
2. `enigo`：输入注入（键鼠）
3. `tokio`：等待与异步执行
4. `serde/serde_json`：参数序列化

建议模块：

1. `src-tauri/src/features/system/tools/types.rs`
2. `src-tauri/src/features/system/tools/screenshot.rs`
3. `src-tauri/src/features/system/tools/operate.rs`
4. `src-tauri/src/features/system/tools/wait.rs`
5. `src-tauri/src/features/system/tools/mod.rs`

---

## 7. 里程碑与工期

M1（0.5 天）：基础类型与错误码

1. 请求/响应结构体
2. 错误码枚举与统一返回体
3. 基础日志字段规范

M2（0.5 天）：`screenshot` 完整实现

1. 全桌面截图
2. 显示器/区域截图
3. 文件落盘与元数据返回

M3（1 天）：`operate` 完整实现

1. 键盘动作：按键、组合键、长按
2. 鼠标动作：左/右/中点击、长按、滚轮、拖拽
3. 文本粘贴
4. 目标解析（text/button/point）
5. 操作后延迟 + 可选重截图

M4（0.5 天）：`wait` + 编排联调

1. `sleep(ms)` 工具
2. 三工具链路测试：`screenshot -> operate -> wait`

M5（0.5 天）：稳定性测试与文档

1. 长轮次稳定性
2. 常见故障说明（权限、DPI、焦点丢失）
3. 使用示例与调试指引

---

## 8. 验收标准

1. `screenshot` 连续 100 次成功率 >= 99%。
2. `operate` 所有动作类型均有可运行示例与至少 1 条测试用例。
3. `operate` 支持：
   - 组合键
   - 长按
   - 滚轮上下
   - 文本粘贴
4. 每次操作可配置 `post_delay_ms` 与 `rescreenshot`。
5. 点击目标策略符合规则：文本优先，按钮为显式分支。
6. 三工具编排链路稳定可复现。

---

## 9. 风险与应对

1. DPI 缩放导致点击偏移
   - 应对：进程开启 DPI aware，全链路物理像素坐标。
2. 多屏坐标负值导致区域错误
   - 应对：统一虚拟桌面坐标，边界检查前置。
3. 焦点窗口变化导致键盘操作落空
   - 应对：操作前可选聚焦步骤，失败返回可读错误。
4. 文本目标歧义导致误点
   - 应对：返回候选并暴露 `AMBIGUOUS_TARGET`。

---

## 10. 测试计划（首版）

1. 功能测试：
   - 鼠标左/右/中点击、长按、拖拽、滚轮
   - 键盘单键、组合键、长按
   - 文本粘贴
2. 性能测试：
   - 截图 `avg/p50/p95/fps`
3. 稳定性测试：
   - 三工具连续编排 200 轮
4. 回归测试：
   - 常见分辨率与 125%/150% 缩放

---

## 11. 交付物

1. 工具实现代码（3 个工具 + 类型 + 错误）
2. 联调示例与测试用例
3. 文档：
   - 本执行计划
   - 使用指南
   - 常见问题清单
