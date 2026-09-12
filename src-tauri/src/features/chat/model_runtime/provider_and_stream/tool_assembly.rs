fn tool_manifest_item(
    source: &str,
    name: &str,
    enabled: bool,
    attached: bool,
    reason: Option<String>,
) -> Value {
    serde_json::json!({
        "source": source,
        "name": name,
        "enabled": enabled,
        "attached": attached,
        "reason": reason
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CachedRuntimeToolSource {
    Builtin,
    Mcp {
        server_id: String,
        server_name: String,
        runtime_tool_name: String,
    },
}

#[derive(Debug, Clone)]
struct CachedRuntimeToolSchema {
    source: CachedRuntimeToolSource,
    permission_candidate_names: Vec<String>,
    definition: ProviderToolDefinition,
    compatibility_error: Option<String>,
}

impl CachedRuntimeToolSchema {
    fn builtin(definition: ProviderToolDefinition) -> Self {
        Self {
            permission_candidate_names: vec![definition.name.clone()],
            source: CachedRuntimeToolSource::Builtin,
            definition,
            compatibility_error: None,
        }
    }

    fn mcp(
        server_id: &str,
        server_name: &str,
        runtime_tool_name: &str,
        compatibility_error: Option<String>,
        definition: ProviderToolDefinition,
    ) -> Self {
        let provider_tool_name = definition.name.clone();
        Self {
            source: CachedRuntimeToolSource::Mcp {
                server_id: server_id.to_string(),
                server_name: server_name.to_string(),
                runtime_tool_name: runtime_tool_name.to_string(),
            },
            permission_candidate_names: vec![
                format!("{server_name}::{runtime_tool_name}"),
                format!("{server_id}::{runtime_tool_name}"),
                format!("{server_name}_{runtime_tool_name}"),
                runtime_tool_name.to_string(),
                format!("{server_name}::{provider_tool_name}"),
                format!("{server_id}::{provider_tool_name}"),
                format!("{server_name}_{provider_tool_name}"),
                provider_tool_name,
            ],
            definition,
            compatibility_error,
        }
    }

    fn source_label(&self) -> String {
        match &self.source {
            CachedRuntimeToolSource::Builtin => "builtin".to_string(),
            CachedRuntimeToolSource::Mcp { server_id, .. } => format!("mcp:{server_id}"),
        }
    }
}

fn tool_schema_cache_store() -> &'static Mutex<Option<Vec<CachedRuntimeToolSchema>>> {
    static STORE: OnceLock<Mutex<Option<Vec<CachedRuntimeToolSchema>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn cached_tool_to_manifest_item(tool: &CachedRuntimeToolSchema) -> Value {
    tool_manifest_item(
        &tool.source_label(),
        &tool.definition.name,
        true,
        true,
        None,
    )
}

fn runtime_tool_names_for_log(tool_assembly: &RuntimeToolAssembly) -> Option<Value> {
    let mut names = Vec::<String>::new();
    for item in &tool_assembly.tool_manifest {
        let Some(name) = item
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let attached = item.get("attached").and_then(Value::as_bool).unwrap_or(true);
        if enabled && attached && !names.iter().any(|existing| existing == name) {
            names.push(name.to_string());
        }
    }
    if names.is_empty() {
        return None;
    }
    Some(Value::Array(
        names
            .into_iter()
            .map(|name| serde_json::json!({ "name": name }))
            .collect(),
    ))
}

fn windows_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        WINDOWS_TOOL_NAME,
        "窗口管理工具。入参只有 script:string，一行一个动作。\n可用语法：\nlist windows\nactivate window id=<windowId>\n参数说明：id 支持十进制或 0x 前缀十六进制（与控件树返回的 windowId 一致）。\n规则：list windows 返回全部可见顶层窗口（含当前应用自身，标题/进程ID/位置/最小化/聚焦状态）；activate window 会把目标窗口还原并切换到前台（激活失败时 ok=false 并在 summary 说明），切换后可配合 operate 的 screenshot focused_window 截取该窗口。\n平台说明：Windows 全量枚举可见窗口（windowId 是窗口句柄）；Linux（X11）经 EWMH 枚举、xcb 激活，Wayland 下原生窗口不可见、激活不生效；macOS 经 CGWindowList 枚举、AXUIElement+NSRunningApplication 激活，控件树与激活需在系统设置中授予辅助功能权限，窗口标题需屏幕录制权限。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "窗口管理脚本文本，一行一个动作。"
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 60000,
                    "description": "本次窗口管理工具调用的超时时间，单位毫秒；未指定时默认 60000ms。"
                }
            },
            "required": ["script"]
        }),
    )
}

fn operate_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        OPERATE_TOOL_NAME,
        "统一桌面脚本工具。主入参 script:string，一行一个动作；另有可选入参 retry / restore_focus / timeout_ms（见参数说明末尾）。\n【动作速查】可用语法：\nmouse <button> click @x,y [repeat=n] [delay=s] [pre_delay=s] [press=s] [monitor=<id>] [target=<id|\"标题\">] [focus=verify|best_effort|strict] [verify=true]\nmouse <button> drag @x1,y1 @x2,y2 [duration=s] [pre_delay=s] [monitor=<id>] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse <button> down|up [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse scroll_up [repeat=n] [delay=s] [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse scroll_down [repeat=n] [delay=s] [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse scroll_left [repeat=n] [delay=s] [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse scroll_right [repeat=n] [delay=s] [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nmouse move @x,y [pre_delay=s] [monitor=<id>] [target=<id|\"标题\">] [focus=verify|best_effort|strict]\nw = app \"窗口名\" [monitor=<id>]\nw click \"名\"|@x,y [repeat=n] [dblclick=true] [pre_delay=s] [post_delay=s] [monitor=<id>]\nw setvalue \"名\" = \"内容\" [pre_delay=s] [post_delay=s] [verify=true]\nw getvalue \"名\"\nw scroll_up|scroll_down|scroll_left|scroll_right \"名\"|@x,y [repeat=n] [delay=s] [pre_delay=s] [post_delay=s] [monitor=<id>]\nw key <combo> [repeat=n] [delay=s] [pre_delay=s] [post_delay=s]\nkey <combo> [repeat=n] [delay=s] [pre_delay=s] [press=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict] [verify=true]\ntext \"内容\" [repeat=n] [delay=s] [pre_delay=s] [target=<id|\"标题\">] [focus=verify|best_effort|strict] [verify=true]\nwait <seconds>\nwait until window|element \"值\" [timeout=s] [target=<id|\"标题\">]\nwindow list\nwindow activate <id|\"标题\">\nclipboard read\nclipboard write \"内容\"\nscreenshot [focused_window|window_id=<id>|window=\"名\"|monitor=<id>] [region=@x,y,w,h] [elements=true] [text=true] [save=\"绝对路径\"] [quality=1..100] [max_pixels=n]\n【参数说明】button=left|right|middle|back|forward；combo 用 + 连接按键，如 Control+L、Control+Shift+P、Enter；x/y/w/h 为 0~1 百分比坐标；repeat=1~100；delay/pre_delay/post_delay/press/duration=0~300 秒；save 必须是绝对路径；quality 默认 75；windowId 与 window_id= 支持十进制或 0x 前缀十六进制（与控件树返回的 windowId 一致）；monitor=<id> 指定坐标基准显示器（0 起，省略时为主屏），副屏上的窗口和元素必须显式给 monitor，否则坐标会按主屏换算而点错位置；screenshot monitor=<id> 截取该显示器全屏；target=<id|\"标题\"> 声明本条前台动作的目标窗口：十进制或 0x 窗口 id，或带双引号的标题子串（需唯一命中，找不到或命中多个会报错并列出候选窗口）；focus 仅在声明 target 时生效：verify（默认）不激活、当前前台不是目标即失败，best_effort 尝试激活、失败继续并在步骤摘要中报告，strict 必须激活成功、失败即中止并返回 focusFailed（目标是否存活/可见/最小化、前后前台、建议动作）。未声明 target 的前台动作保持历史行为：直接注入当前前台，不校验。用户在设置中配置的禁止操作名单（标题子串）对全部输入动作生效：前台动作、w 后台动作、窗口激活在目标命中名单时直接失败并说明命中的关键词；截图、等待、窗口列举不受影响。目标窗口优先用名不用句柄：w = app \"记事本\"、window activate \"标题\"、screenshot window=\"名\"、前台 target=\"标题\" 都是先按标题子串唯一命中、再按进程名唯一命中；找不到或命中多个会报错并列出候选，句柄只是备选（前台 target 与 window activate 仍可用句柄）。窗口变量 w 为本次调用内有效：须先声明后使用（w = app \"记事本\"），字母开头、仅含字母/数字/下划线、大小写敏感、不得与动作关键字重名，重复声明即覆盖；句柄每次使用时现查不缓存。mouse click / key / text 加 verify=true 会在动作后附带结果确认（当前前台是谁），w setvalue verify=true 会读回实际值比对一致性，只附带到摘要、不改变成败。w click/scroll 的 monitor 缺省时继承声明行 monitor，显式给则优先用动作行。clipboard read 只读不改（超长截断），clipboard write 显式覆盖剪贴板；仅 Windows 可用。screenshot max_pixels=n（40000~100000000）按像素预算等比缩小出图，不给即全尺寸；画面与本次调用的上次截图相同时步骤摘要带 dedup=true 且不再重复传输图片。入参 retry（0~3，默认 0）是输入步骤失败时的重试预算，每次间隔 200ms，参数非法与门控拦截不重试，成功时摘要带重试次数；restore_focus=true 时结束后切回执行前的前台，恢复失败只记 warnings。mouse <button> down|up 只按下或释放鼠标键、不移动光标，用于长按菜单或分步拖拽。wait until window \"标题\" 等到出现标题包含该子串的可见窗口；wait until element \"名称\" 等到目标窗口（未给 target 时为当前前台窗口）内出现名称包含该子串的可交互元素；每 200ms 轮询一次，timeout 默认 30 秒，超时返回失败并提示先 screenshot 观察现场。window list 返回可见顶层窗口列表并写入响应的 windows 字段（每项只有 windowId/title/processName/focused/minimized，坐标尺寸与进程号不下发）；window activate <id|\"标题\"> 把目标窗口切到前台，失败时返回 focusFailed。脚本执行超过入参 timeout_ms 时会在步骤之间中断，已完成的步骤仍然返回。\n【失败与恢复】脚本中途失败时返回 ok=false 与 failure（line/message，抢焦点失败时附 focusFailed），steps 仍包含已完成步骤，可据此从失败行重试；响应中的 warnings 字段（若存在）是不影响执行成功、但会降低结果可信度的环境提示，例如 DPI 感知未生效导致坐标可能偏移。mouse click 步骤摘要会附带该坐标在前台窗口命中的元素（controlType 与 name），用于确认点到了什么。\n【后台动作】w 动作是对已声明窗口变量的后台操作：不抢焦点、不移动真实光标，仅 Windows 可用，用前须先声明（例如 w = app \"记事本\"）。元素按名字寻址、每次使用时新鲜扫描：唯一命中才执行，找不到或命中多个会报错并列出候选，此时换更具体的名或改用坐标兜底（w click @x,y，坐标相对声明/动作行 monitor 指定的显示器归一化）。老写法 app <windowId> 与 el= 已退役，写了会直接报错并给改写示例。w 步骤结果的 method 字段报告实际投递方式（invoke/valuepattern/scrollpattern/postmessage）；method=postmessage 表示元素无对应 UIA 模式、已退化为坐标投递，摘要会显式标注「结果不保证」，此时不能把 ok=true 读成动作已生效（例如 Chrome 标签栏 TabItem 切页必须前台激活，坐标投递无效）；invoke 命中链接时摘要会提示页面可能已跳转或新开标签页，不要按旧页面继续写脚本。w setvalue 为整体替换文本控件内容；w getvalue 通过 ValuePattern 读回文本控件当前值（写后读回可验证写入结果，无需截图），控件不支持 ValuePattern 时明确报错。w click 的 dblclick=true 走后台双击（仅 postmessage 路径，invoke-only 控件无效）；w 动作结果的 focus 字段（focus=Type('name')）报告动作后目标窗口内部焦点控件去向，setvalue/key 后用它确认焦点位置，不必重截；焦点还在窗口本身时报 focus=窗口本身（内部暂无可聚焦控件），不报加载中的临时窗口标题。post_delay 在动作完成后等待 UI 响应（如联想词、弹出菜单）再返回，避免下一步拿到过期元素树。w key 把按键投递到目标窗口的内部焦点控件（适用于回车提交、Esc 关弹窗、Ctrl+A 全选等），但全局快捷键/菜单加速键类按键可能不响应。部分应用（UWP/WebView2 等）不响应 postmessage 兜底，只能依赖 UIA pattern。\n【截图规则】screenshot 对模型只保留最新一张，旧画面视为已经离去。\n【控件树】需要确认准确点击坐标时，用局部截图（region=@x,y,w,h、window_id=<id> 或 focused_window）配合 elements=true，latest_screenshot.tree 会返回该区域内的可交互控件列表（windowId/windowTitle/controlType/name/x/y/width/height/focused，坐标为相对主屏的 0~1 归一化、与 mouse 的 @x,y 同坐标系；区域外控件已过滤，不会返回；w 动作按名操作、不消费序号，照名字写即可；text=true 时额外附带非交互文本标签，便于用「用户名: [输入框]」这类上下文定位目标，但元素上限 500 会更早触顶，建议配合 region 或 window_id 缩小范围）；只查看画面、不需要点击时不要带 elements=true，避免多余的控件扫描开销。focused_window 只返回聚焦窗口的控件，window_id=<id> 截取并扫描指定后台窗口，region 只返回区域内的控件，desktop 返回全部可见窗口的控件（元素多，谨慎使用）。控件树平台说明：Windows 用系统 UI Automation；Linux（X11）用 AT-SPI2，Wayland 或未启用辅助功能时返回空，且窗口枚举与截图同样不可用（仅 XWayland 窗口可见），应改用 key/text 操作当前前台；macOS 用 AXUIElement，需在系统设置中授予辅助功能权限。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "桌面脚本文本，一行一个动作。"
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 300000,
                    "description": "本次桌面脚本工具调用的超时时间，单位毫秒；未指定时默认 300000ms。长时间 wait 或自动化脚本应显式传入足够大的值。"
                },
                "retry": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 3,
                    "default": 0,
                    "description": "单步重试预算：每个输入步骤失败时最多重试次数（0~3，默认0）。瞬态失败（焦点竞争、加载慢）可设 1~3。"
                },
                "restore_focus": {
                    "type": "boolean",
                    "default": false,
                    "description": "执行后焦点恢复：为 true 时脚本结束后把前台切回执行前的窗口（默认 false）。"
                }
            },
            "required": ["script"]
        }),
    )
}

fn read_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        READ_TOOL_NAME,
        "读取本地文档内容。支持文本、代码、PDF 与 Office 文件；path 必须是绝对路径；对文本、代码、Office 等非 PDF 内容，offset 表示跳过行，limit 表示返回行数；对 PDF 则代表页。图片、音频、视频请改用 read_media。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要读取的本地文件绝对路径。"
                },
                "offset": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "跳过数，默认从 0 开始。"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "返回数。"
                }
            },
            "required": ["path"]
        }),
    )
}

fn read_media_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        READ_MEDIA_TOOL_NAME,
        "解析本地图片、音频或视频；仅在当前看不到图片，或需要解析音频、视频时使用。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要解析的本地媒体文件绝对路径。"
                },
                "description": {
                    "type": "string",
                    "description": "解析侧重点，例如要看什么、听什么、提取什么。"
                }
            },
            "required": ["path"]
        }),
    )
}

const OPERATE_TOOL_DEFAULT_TIMEOUT_MS: u64 = 300_000;
const READ_MEDIA_IMAGE_TOOL_TIMEOUT_SECS: u64 = 90;
const READ_MEDIA_AUDIO_TOOL_TIMEOUT_SECS: u64 = 4 * 60;
const READ_MEDIA_VIDEO_TOOL_TIMEOUT_SECS: u64 = 10 * 60;

fn operate_tool_timeout_override(args_json: &str) -> std::time::Duration {
    let timeout_ms = parse_runtime_tool_args::<OperateRequest>(args_json)
        .ok()
        .and_then(|args| args.timeout_ms)
        .unwrap_or(OPERATE_TOOL_DEFAULT_TIMEOUT_MS)
        .max(1);
    std::time::Duration::from_millis(timeout_ms)
}

const WINDOWS_TOOL_DEFAULT_TIMEOUT_MS: u64 = 60_000;

fn windows_tool_timeout_override(args_json: &str) -> std::time::Duration {
    let timeout_ms = parse_runtime_tool_args::<WindowsRequest>(args_json)
        .ok()
        .and_then(|args| args.timeout_ms)
        .unwrap_or(WINDOWS_TOOL_DEFAULT_TIMEOUT_MS)
        .max(1);
    std::time::Duration::from_millis(timeout_ms)
}

fn read_media_tool_timeout_override(args_json: &str) -> std::time::Duration {
    let media_type = parse_runtime_tool_args::<ReadMediaToolArgs>(args_json)
        .ok()
        .and_then(|args| detect_read_media_type(std::path::Path::new(args.path.trim())));
    let timeout_secs = match media_type {
        Some(ReadMediaDetectedType::Audio) => READ_MEDIA_AUDIO_TOOL_TIMEOUT_SECS,
        Some(ReadMediaDetectedType::Video) => READ_MEDIA_VIDEO_TOOL_TIMEOUT_SECS,
        Some(ReadMediaDetectedType::Image) | None => READ_MEDIA_IMAGE_TOOL_TIMEOUT_SECS,
    };
    std::time::Duration::from_secs(timeout_secs)
}

fn build_global_tool_schema_cache(state: &AppState) -> Vec<CachedRuntimeToolSchema> {
    let preview_session_id = "__tool_schema_cache__".to_string();
    let _preview_api_id = "__tool_schema_cache__".to_string();
    let preview_agent_id = DEFAULT_AGENT_ID.to_string();
    let preview_memory_context = build_memory_agent_context(&preview_agent_id, false, true)
        .unwrap_or(MemoryAgentContext {
            owner_agent_id: None,
            effective_agent_id: preview_agent_id.clone(),
            private_memory_enabled: false,
            recall_enabled: true,
        });
    let builtin_definitions = vec![
        BuiltinFetchTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinBingSearchTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinRememberTool {
            app_state: state.clone(),
            memory_context: preview_memory_context.clone(),
        }
        .provider_tool_definition(),
        BuiltinRecallTool {
            app_state: state.clone(),
            memory_context: preview_memory_context,
        }
        .provider_tool_definition(),
        operate_provider_tool_definition(),
        windows_provider_tool_definition(),
        read_provider_tool_definition(),
        read_media_provider_tool_definition(),
        BuiltinTerminalExecTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            executor_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinConfigTool {
            app_state: state.clone(),
        }
        .provider_tool_definition(),
        BuiltinWriteFileTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            executor_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinDeleteFileTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            executor_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinUpdateFileTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            executor_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinMoveFileTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            executor_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinPlanTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinTodoTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinCreateGoalTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinUpdateGoalTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinGetSessionTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinBackgroundTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinInformSessionTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinTaskTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            api_config_id: String::new(),
            executor_department_id: String::new(),
            executor_agent_id: preview_agent_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinDeepRecallTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            source_agent_id: preview_agent_id.clone(),
            source_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinDeepRecallSearchTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            agent_id: preview_agent_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinDeepRecallContextTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            agent_id: preview_agent_id.clone(),
        }
        .provider_tool_definition(),
        BuiltinDelegateTool {
            app_state: state.clone(),
            session_id: preview_session_id.clone(),
            source_agent_id: preview_agent_id,
            source_department_id: String::new(),
        }
        .provider_tool_definition(),
        BuiltinMemeTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinImageGenerateTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinImageEditTool { app_state: state.clone() }.provider_tool_definition(),
        BuiltinContactSendFilesTool {
            app_state: state.clone(),
            session_id: preview_session_id,
        }
        .provider_tool_definition(),
    ];
    let mut definitions = builtin_definitions
        .into_iter()
        .map(CachedRuntimeToolSchema::builtin)
        .collect::<Vec<_>>();

    match load_workspace_mcp_servers(state) {
        Ok(servers) => {
            let mcp_tools = servers
                .into_iter()
                .filter(|server| server.enabled)
                .flat_map(|server| {
                    list_tools_from_runtime(&server)
                        .into_iter()
                        .filter(|tool| tool.enabled)
                        .map(move |tool| (server.clone(), tool))
                })
                .collect::<Vec<_>>();
            for (server, tool) in mcp_tools {
                // 工具名（别名）在探测时已按规则生成：原始名带成员前缀则保持，裸名补 {成员}_ 前缀。
                // 注册、展示、AI 调用全部统一用该别名，执行时才反查 raw_tool_name 调远端。
                let provider_tool_name = tool.tool_name.clone();
                definitions.push(CachedRuntimeToolSchema::mcp(
                    &server.id,
                    &server.name,
                    &tool.tool_name,
                    tool.compatibility_error,
                    ProviderToolDefinition::new(
                        provider_tool_name,
                        tool.description,
                        tool.parameters,
                    ),
                ));
            }
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 加载 MCP 配置失败: {err}")),
    }

    definitions
}

fn refresh_global_tool_schema_cache(state: &AppState) -> Vec<CachedRuntimeToolSchema> {
    let definitions = build_global_tool_schema_cache(state);
    match tool_schema_cache_store().lock() {
        Ok(mut guard) => {
            *guard = Some(definitions.clone());
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 刷新失败，缓存锁已损坏: {err}")),
    }
    definitions
}

fn clear_global_tool_schema_cache() {
    match tool_schema_cache_store().lock() {
        Ok(mut guard) => {
            *guard = None;
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 清空失败，缓存锁已损坏: {err}")),
    }
}

fn read_global_tool_schema_cache(_state: Option<&AppState>) -> Vec<CachedRuntimeToolSchema> {
    match tool_schema_cache_store().lock() {
        Ok(guard) => {
            if let Some(definitions) = guard.as_ref() {
                return definitions.clone();
            }
        }
        Err(err) => runtime_log_warn(format!("[工具Schema缓存] 读取失败，缓存锁已损坏: {err}")),
    }
    Vec::new()
}

fn resolve_runtime_tool_current_department<'a>(
    app_config: &'a AppConfig,
    executor_department_id: Option<&str>,
) -> Option<&'a DepartmentConfig> {
    executor_department_id
        .map(str::trim)
        .filter(|department_id| !department_id.is_empty())
        .and_then(|department_id| department_by_id(app_config, department_id))
}

#[derive(Debug, Clone, Default)]
struct RuntimeToolPolicy {
    conversation_resolved: bool,
    local_conversation: bool,
    delegate_conversation: bool,
    remote_reply_delegate: bool,
    contact_send_files_allowed: bool,
    origin_scope: RuntimeToolOriginScope,
    /// 当前会话是否为 deeprecall 发起的深度回忆委托（决定检索工具是否挂载）。
    deep_recall_delegate: bool,
}

impl RuntimeToolPolicy {
    fn from_conversation(conversation: Option<&Conversation>) -> Self {
        let Some(conversation) = conversation else {
            return Self::default();
        };
        Self {
            conversation_resolved: true,
            local_conversation: conversation_is_local_normal_chat(conversation),
            delegate_conversation: conversation_is_delegate(conversation),
            remote_reply_delegate: false,
            contact_send_files_allowed: false,
            origin_scope: if conversation_is_local_normal_chat(conversation) {
                RuntimeToolOriginScope::Local
            } else if conversation_is_remote_im_contact(conversation) {
                RuntimeToolOriginScope::RemoteUnknown
            } else {
                RuntimeToolOriginScope::Unknown
            },
            deep_recall_delegate: false,
        }
    }

    fn tool_unavailable_reason(&self, tool_name: &str) -> Option<String> {
        builtin_tool_runtime_unavailable_reason(
            tool_name,
            self.origin_scope,
            self.conversation_resolved,
            self.local_conversation,
            self.delegate_conversation,
            self.remote_reply_delegate,
            self.contact_send_files_allowed,
            self.deep_recall_delegate,
        )
    }
}

/// 判定委托会话是否由 deeprecall 发起：只有深度回忆委托才挂载检索工具。
fn deep_recall_delegate_conversation(state: &AppState, conversation_id: &str) -> bool {
    let delegate_id = conversation_id.trim();
    if delegate_id.is_empty() {
        return false;
    }
    match delegate_store_get_delegate(&state.data_path, delegate_id) {
        Ok(entry) => entry.kind.trim() == DELEGATE_TOOL_KIND_DEEP_RECALL,
        Err(_) => false,
    }
}

fn runtime_tool_policy_from_session(
    app_state: Option<&AppState>,
    tool_session_id: &str,
    resolve_contact_send_files: bool,
) -> RuntimeToolPolicy {
    let Some(state) = app_state else {
        return RuntimeToolPolicy::default();
    };
    let Ok(conversation_id) = goal_tool_conversation_id(tool_session_id) else {
        return RuntimeToolPolicy::default();
    };
    let (mut policy, root_conversation_id) = if let Ok(conversation_meta) =
        conversation_service_v2().get_conversation_meta(state, &conversation_id)
    {
        let conversation_kind = conversation_meta.conversation_kind.trim();
        (
            RuntimeToolPolicy {
                conversation_resolved: true,
                local_conversation: matches!(
                    conversation_kind,
                    CONVERSATION_KIND_CHAT | CONVERSATION_KIND_SIDE_CHAT
                ),
                delegate_conversation: conversation_kind == CONVERSATION_KIND_DELEGATE,
                remote_reply_delegate: delegate_session_is_remote_reply_delegate(tool_session_id)
                    && conversation_kind == CONVERSATION_KIND_REMOTE_IM_CONTACT,
                contact_send_files_allowed: false,
                deep_recall_delegate: false,
                origin_scope: if matches!(
                    conversation_kind,
                    CONVERSATION_KIND_CHAT | CONVERSATION_KIND_SIDE_CHAT
                ) {
                    RuntimeToolOriginScope::Local
                } else if conversation_kind == CONVERSATION_KIND_REMOTE_IM_CONTACT {
                    RuntimeToolOriginScope::RemoteUnknown
                } else {
                    RuntimeToolOriginScope::Unknown
                },
            },
            conversation_meta.root_conversation_id,
        )
    } else {
        let conversation = delegate_runtime_thread_conversation_get(state, &conversation_id)
            .ok()
            .flatten();
        (
            RuntimeToolPolicy::from_conversation(conversation.as_ref()),
            conversation.and_then(|conversation| conversation.root_conversation_id),
        )
    };
    // 委托会话按委托记录 kind 细分：只有 deeprecall 发起的委托才挂载检索工具。
    if policy.delegate_conversation {
        policy.deep_recall_delegate = deep_recall_delegate_conversation(state, &conversation_id);
    }
    let bound_contact = remote_im_bound_contact_context_from_runtime(state, tool_session_id).ok();
    if let Some((_channel, contact)) = bound_contact.as_ref() {
        let resolved_scope = runtime_tool_origin_scope_from_contact_type(&contact.remote_contact_type);
        policy.origin_scope = if resolved_scope == RuntimeToolOriginScope::Unknown
            && policy.origin_scope == RuntimeToolOriginScope::RemoteUnknown
        {
            RuntimeToolOriginScope::RemoteUnknown
        } else {
            resolved_scope
        };
    } else if matches!(
        policy.origin_scope,
        RuntimeToolOriginScope::Unknown | RuntimeToolOriginScope::RemoteUnknown
    ) {
        let root_scope = runtime_tool_origin_scope_from_root_conversation_key(
            root_conversation_id.as_deref(),
        )
        .unwrap_or(RuntimeToolOriginScope::Unknown);
        if root_scope != RuntimeToolOriginScope::Unknown {
            policy.origin_scope = root_scope;
        }
    }
    if resolve_contact_send_files {
        policy.contact_send_files_allowed = bound_contact
            .as_ref()
            .map(|(_channel, contact)| contact.allow_send && contact.allow_send_files)
            .unwrap_or(false);
    }
    policy
}

fn runtime_tool_origin_scope_from_root_conversation_key(
    root_conversation_id: Option<&str>,
) -> Option<RuntimeToolOriginScope> {
    let root = root_conversation_id?.trim();
    let suffix = root.strip_prefix("remote_im_contact:")?;
    let contact_type = suffix.split(':').nth(1)?;
    Some(runtime_tool_origin_scope_from_contact_type(contact_type))
}

fn runtime_tool_origin_scope_from_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> RuntimeToolOriginScope {
    let session_id = format!("{}::{}", conversation.agent_id, conversation.id);
    let policy = runtime_tool_policy_from_session(Some(state), &session_id, false);
    if policy.origin_scope != RuntimeToolOriginScope::Unknown {
        return policy.origin_scope;
    }
    runtime_tool_origin_scope_from_root_conversation_key(
        conversation.root_conversation_id.as_deref(),
    )
    .unwrap_or(RuntimeToolOriginScope::Unknown)
}

#[derive(Debug, Clone)]
struct ResolvedLegalRuntimeTools {
    attached: Vec<CachedRuntimeToolSchema>,
    manifest: Vec<Value>,
}

fn runtime_tool_denied_reason(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    current_department: Option<&DepartmentConfig>,
    runtime_policy: &RuntimeToolPolicy,
    memory_context: Option<&MemoryAgentContext>,
    tool: &CachedRuntimeToolSchema,
) -> Option<String> {
    let tool_name = tool.definition.name.trim();
    if let Some(reason) = headless_cli_tool_denied_reason(tool_name) {
        return Some(reason);
    }
    if !selected_api.enable_tools {
        return Some("当前模型未启用工具调用".to_string());
    }
    if let Some(reason) = runtime_policy.tool_unavailable_reason(tool_name) {
        return Some(reason);
    }
    match &tool.source {
        CachedRuntimeToolSource::Builtin => {
            if matches!(tool_name, "image_generate" | "image_edit")
                && app_config
                    .image_generation_model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
            {
                return Some("未选择默认生图模型，生图工具不挂载".to_string());
            }
            if tool_name == "read_media"
                && app_config
                    .vision_api_config_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
            {
                return Some("未选择多模态分析模型，read_media 工具不挂载".to_string());
            }
            if matches!(tool_name, "remember" | "recall")
                && !memory_context.map(|context| context.recall_enabled).unwrap_or(false)
            {
                return Some("当前人格记忆功能已关闭".to_string());
            }
            if matches!(tool_name, "remember" | "recall") && memory_context.is_none() {
                return Some("当前人格记忆上下文不可用".to_string());
            }
            if tool_name == "task" && current_department.is_none() {
                return Some("缺少当前执行部门，无法使用任务工具".to_string());
            }
            if tool_name == "delegate" {
                if let Some(reason) =
                    delegate_builtin_tool_unavailable_reason(app_config, current_department)
                {
                    return Some(reason);
                }
            }
            if builtin_tool_is_fixed_system(tool_name)
                || builtin_tool_is_local_conversation_fixed(tool_name)
                || builtin_tool_is_contact_only_hidden(tool_name)
            {
                return None;
            }
            let Some(department) = current_department else {
                return Some("缺少当前执行部门，部门受控工具已降级为不可用".to_string());
            };
            tool_restricted_by_department(Some(department), tool_name)
        }
        CachedRuntimeToolSource::Mcp { .. } => {
            let Some(department) = current_department else {
                return Some("缺少当前执行部门，MCP 工具已降级为不可用".to_string());
            };
            let candidate_names = tool
                .permission_candidate_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            if department_permission_allows_any_name(
                Some(department),
                DepartmentPermissionCategory::McpTool,
                &candidate_names,
            ) {
                None
            } else {
                Some(format!(
                    "当前部门权限不允许 MCP 工具 `{}`",
                    tool.permission_candidate_names
                        .first()
                        .map(String::as_str)
                        .unwrap_or(tool_name)
                ))
            }
        }
    }
}

fn resolve_legal_runtime_tools_for_department(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    current_department: Option<&DepartmentConfig>,
    runtime_policy: &RuntimeToolPolicy,
    memory_context: Option<&MemoryAgentContext>,
    discoverable_tools: &[CachedRuntimeToolSchema],
) -> ResolvedLegalRuntimeTools {
    let mut attached = Vec::<CachedRuntimeToolSchema>::new();
    let mut manifest = Vec::<Value>::new();
    let mut attached_provider_names = HashSet::<String>::new();
    for tool in discoverable_tools {
        if let Some(reason) = tool.compatibility_error.clone() {
            if let CachedRuntimeToolSource::Mcp {
                server_id,
                server_name,
                ..
            } = &tool.source
            {
                runtime_log_warn(format!(
                    "[MCP工具装配] 跳过，server_id={}，server_name={}，tool_name={}，原因={}",
                    server_id,
                    server_name,
                    tool.definition.name,
                    reason
                ));
            }
            manifest.push(tool_manifest_item(
                &tool.source_label(),
                &tool.definition.name,
                true,
                false,
                Some(reason),
            ));
            continue;
        }
        if let Some(reason) = runtime_tool_denied_reason(
            app_config,
            selected_api,
            current_department,
            runtime_policy,
            memory_context,
            tool,
        ) {
            let manifest_source = if tool.definition.name == "delegate"
                && delegate_builtin_tool_unavailable_reason(app_config, current_department).is_some()
            {
                "runtime_policy".to_string()
            } else {
                tool.source_label()
            };
            manifest.push(tool_manifest_item(
                &manifest_source,
                &tool.definition.name,
                false,
                false,
                Some(reason),
            ));
            continue;
        }
        if !attached_provider_names.insert(tool.definition.name.clone()) {
            manifest.push(tool_manifest_item(
                &tool.source_label(),
                &tool.definition.name,
                true,
                false,
                Some("同名工具已由更高优先级来源挂载".to_string()),
            ));
            continue;
        }
        attached.push(tool.clone());
    }
    ResolvedLegalRuntimeTools { attached, manifest }
}

struct AuthorizationCheckedRuntimeTool {
    inner: Box<dyn RuntimeToolDyn>,
    app_state: AppState,
    tool_name: String,
    tool_session_id: String,
    executor_department_id: String,
}

fn runtime_builtin_tool_authorization_error(
    state: &AppState,
    tool_name: &str,
    tool_session_id: &str,
    executor_department_id: &str,
) -> Option<String> {
    let runtime_policy = runtime_tool_policy_from_session(
        Some(state),
        tool_session_id,
        tool_name == "contact_send_files",
    );
    if let Some(reason) = runtime_policy.tool_unavailable_reason(tool_name) {
        return Some(reason);
    }
    if !builtin_tool_is_department_controlled(tool_name) {
        return None;
    }
    let app_config = match state_read_config_cached(state) {
        Ok(config) => config,
        Err(err) => return Some(format!("读取最新权限失败，已跳过本工具：{err}")),
    };
    let current_department = department_by_id(&app_config, executor_department_id);
    if tool_name == "delegate" {
        if let Some(reason) =
            delegate_builtin_tool_unavailable_reason(&app_config, current_department)
        {
            return Some(reason);
        }
    }
    tool_restricted_by_department(current_department, tool_name)
}

impl RuntimeToolDyn for AuthorizationCheckedRuntimeTool {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn timeout_override(&self, args_json: &str) -> Option<std::time::Duration> {
        self.inner.timeout_override(args_json)
    }

    fn is_mcp_tool(&self) -> bool {
        self.inner.is_mcp_tool()
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        if let Some(reason) = runtime_builtin_tool_authorization_error(
            &self.app_state,
            &self.tool_name,
            &self.tool_session_id,
            &self.executor_department_id,
        ) {
            let tool_name = self.tool_name.clone();
            return Box::pin(async move {
                Ok(ProviderToolResult::error(format!(
                    "工具 `{tool_name}` 当前不可用：{reason}"
                )))
            });
        }
        self.inner.call_json(args_json)
    }
}

fn empty_runtime_tool_assembly(tool_manifest: Vec<Value>) -> RuntimeToolAssembly {
    RuntimeToolAssembly {
        tools: Vec::new(),
        tool_definitions: Vec::new(),
        tool_manifest,
        unavailable_tool_notices: Vec::new(),
    }
}

pub(crate) async fn assemble_runtime_tools(
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    app_state: Option<&AppState>,
    tool_session_id: &str,
    executor_department_id: Option<&str>,
) -> RuntimeToolAssembly {
    if !selected_api.enable_tools {
        return empty_runtime_tool_assembly(Vec::new());
    }
    let Some(state) = app_state else {
        runtime_log_warn("[工具装配] 跳过，原因=缺少AppState，聊天继续但本轮不挂载工具".to_string());
        return empty_runtime_tool_assembly(Vec::new());
    };
    let current_department = resolve_runtime_tool_current_department(app_config, executor_department_id);
    if current_department.is_none() {
        runtime_log_warn(format!(
            "[工具装配] 降级，原因=执行部门不存在，department_id={}，部门受控工具与MCP将跳过",
            executor_department_id.unwrap_or_default()
        ));
    }
    let runtime_tool_policy = runtime_tool_policy_from_session(app_state, tool_session_id, true);
    let memory_context = match memory_agent_context_from_agent(agent) {
        Ok(context) => Some(context),
        Err(err) => {
            runtime_log_warn(format!(
                "[工具装配] 记忆工具降级，agent_id={}，error={err}",
                agent.id
            ));
            None
        }
    };
    let mut discoverable_tools = read_global_tool_schema_cache(app_state);
    if discoverable_tools.is_empty() {
        runtime_log_warn("[工具装配] Schema缓存为空，尝试按当前可发现能力重建".to_string());
        discoverable_tools = refresh_global_tool_schema_cache(state);
    }
    // 电脑使用总开关：关闭时不提供 operate 工具（模型看不到、调不了）
    let desktop_operate_enabled = state_read_config_cached(state)
        .map(|config| config.desktop_operate_enabled)
        .unwrap_or(true);
    if !desktop_operate_enabled {
        discoverable_tools.retain(|tool| tool.definition.name != OPERATE_TOOL_NAME);
        runtime_log_info("[工具装配] 电脑使用已关闭，operate 工具不挂载".to_string());
    }
    let resolved = resolve_legal_runtime_tools_for_department(
        app_config,
        selected_api,
        current_department,
        &runtime_tool_policy,
        memory_context.as_ref(),
        &discoverable_tools,
    );
    let mut tools: Vec<Box<dyn RuntimeToolDyn>> = Vec::new();
    let mut tool_definitions = Vec::<ProviderToolDefinition>::new();
    let mut tool_manifest = resolved.manifest;
    for descriptor in resolved.attached {
        let executor = match &descriptor.source {
            CachedRuntimeToolSource::Builtin => build_builtin_runtime_tool_executor(
                state,
                selected_api,
                agent,
                memory_context.as_ref(),
                tool_session_id,
                executor_department_id.unwrap_or_default(),
                &descriptor.definition.name,
            ),
            CachedRuntimeToolSource::Mcp {
                server_id,
                runtime_tool_name,
                ..
            } => {
                build_cached_mcp_runtime_tool_executor(
                    state,
                    server_id,
                    executor_department_id.unwrap_or_default(),
                    runtime_tool_name,
                    &descriptor.definition,
                )
            }
        };
        match executor {
            Ok(executor) => {
                let executor_name = executor.name();
                if executor_name != descriptor.definition.name {
                    runtime_log_warn(format!(
                        "[工具装配] 单工具降级，tool={}，source={}，error=执行器名称不一致（executor={}）",
                        descriptor.definition.name,
                        descriptor.source_label(),
                        executor_name
                    ));
                    tool_manifest.push(tool_manifest_item(
                        &descriptor.source_label(),
                        &descriptor.definition.name,
                        true,
                        false,
                        Some(format!("执行器名称不一致，已跳过：{executor_name}")),
                    ));
                    continue;
                }
                tool_definitions.push(descriptor.definition.clone());
                tool_manifest.push(cached_tool_to_manifest_item(&descriptor));
                tools.push(executor);
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[工具装配] 单工具降级，tool={}，source={}，error={err}",
                    descriptor.definition.name,
                    descriptor.source_label()
                ));
                tool_manifest.push(tool_manifest_item(
                    &descriptor.source_label(),
                    &descriptor.definition.name,
                    true,
                    false,
                    Some(format!("执行器不可用，已跳过：{err}")),
                ));
            }
        }
    }
    RuntimeToolAssembly {
        tools,
        tool_definitions,
        tool_manifest,
        unavailable_tool_notices: Vec::new(),
    }
}

fn build_builtin_runtime_tool_executor(
    state: &AppState,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    memory_context: Option<&MemoryAgentContext>,
    tool_session_id: &str,
    executor_department_id: &str,
    tool_name: &str,
) -> Result<Box<dyn RuntimeToolDyn>, String> {
    let state = state.clone();
    let memory_context = memory_context.cloned();
    let tool: Box<dyn RuntimeToolDyn> = match tool_name {
        "fetch" => Box::new(BuiltinFetchTool { app_state: state.clone() }),
        "websearch" => Box::new(BuiltinBingSearchTool { app_state: state.clone() }),
        "remember" => Box::new(BuiltinRememberTool {
            app_state: state.clone(),
            memory_context: memory_context
                .clone()
                .ok_or_else(|| "记忆上下文不可用".to_string())?,
        }),
        "recall" => Box::new(BuiltinRecallTool {
            app_state: state.clone(),
            memory_context: memory_context.ok_or_else(|| "记忆上下文不可用".to_string())?,
        }),
        "operate" => Box::new(BuiltinOperateTool {
            app_state: state.clone(),
            model_supports_image: selected_api.enable_image,
            session_id: tool_session_id.to_string(),
        }),
        "windows" => Box::new(BuiltinWindowsTool {}),
        "read" => Box::new(BuiltinReadFileTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            api_config_id: selected_api.id.clone(),
        }),
        "read_media" => Box::new(BuiltinReadMediaTool { app_state: state.clone() }),
        "exec" => Box::new(BuiltinTerminalExecTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }),
        "config" => Box::new(BuiltinConfigTool { app_state: state.clone() }),
        "write" => Box::new(BuiltinWriteFileTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }),
        "delete" => Box::new(BuiltinDeleteFileTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }),
        "update" => Box::new(BuiltinUpdateFileTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }),
        "move" => Box::new(BuiltinMoveFileTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }),
        "plan" => Box::new(BuiltinPlanTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "todo" => Box::new(BuiltinTodoTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "create_goal" => Box::new(BuiltinCreateGoalTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "update_goal" => Box::new(BuiltinUpdateGoalTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "get_session" => Box::new(BuiltinGetSessionTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "background" => Box::new(BuiltinBackgroundTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "inform_session" => Box::new(BuiltinInformSessionTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        "task" => Box::new(BuiltinTaskTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            api_config_id: selected_api.id.clone(),
            executor_department_id: executor_department_id.to_string(),
            executor_agent_id: agent.id.trim().to_string(),
        }),
        "delegate" => Box::new(BuiltinDelegateTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            source_agent_id: agent.id.trim().to_string(),
            source_department_id: executor_department_id.to_string(),
        }),
        "deeprecall" => Box::new(BuiltinDeepRecallTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            source_agent_id: agent.id.trim().to_string(),
            source_department_id: executor_department_id.to_string(),
        }),
        "deeprecall_search" => Box::new(BuiltinDeepRecallSearchTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            agent_id: agent.id.trim().to_string(),
        }),
        "deeprecall_context" => Box::new(BuiltinDeepRecallContextTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
            agent_id: agent.id.trim().to_string(),
        }),
        "meme" => Box::new(BuiltinMemeTool { app_state: state.clone() }),
        "image_generate" => Box::new(BuiltinImageGenerateTool { app_state: state.clone() }),
        "image_edit" => Box::new(BuiltinImageEditTool { app_state: state.clone() }),
        "contact_send_files" => Box::new(BuiltinContactSendFilesTool {
            app_state: state.clone(),
            session_id: tool_session_id.to_string(),
        }),
        _ => return Err(format!("未知内置工具：{tool_name}")),
    };
    if builtin_tool_requires_execution_reauthorization(tool_name) {
        return Ok(Box::new(AuthorizationCheckedRuntimeTool {
            inner: tool,
            app_state: state,
            tool_name: tool_name.to_string(),
            tool_session_id: tool_session_id.to_string(),
            executor_department_id: executor_department_id.to_string(),
        }));
    }
    Ok(tool)
}

fn build_cached_mcp_runtime_tool_executor(
    state: &AppState,
    server_id: &str,
    executor_department_id: &str,
    runtime_tool_name: &str,
    definition: &ProviderToolDefinition,
) -> Result<Box<dyn RuntimeToolDyn>, String> {
    let server = load_server_by_id(state, server_id)?;
    if !server.enabled {
        return Err(format!("MCP 服务器当前未启用：{}", server.id));
    }
    let current_tool = list_tools_from_runtime(&server)
        .into_iter()
        .find(|tool| tool.tool_name == runtime_tool_name && tool.enabled)
        .ok_or_else(|| format!("MCP 工具当前未启用：{}::{}", server.id, runtime_tool_name))?;
    let input_schema = Arc::new(match current_tool.parameters {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    });
    let runtime_definition = rmcp::model::Tool::new(
        definition.name.clone(),
        current_tool.description,
        input_schema,
    );
    Ok(Box::new(CachedMcpRuntimeTool {
        app_state: state.clone(),
        server_id: server.id,
        executor_department_id: executor_department_id.to_string(),
        runtime_tool_name: runtime_tool_name.to_string(),
        definition: runtime_definition,
    }))
}

/// 模型即将操作电脑（operate 工具）时，发送一条系统通知提醒用户。
/// 每轮调度内最多提醒一次由调度器（tool_loop）控制，本函数只负责发通知；
/// 通知异步发出，不等待提交确认，不阻塞工具执行。
fn notify_desktop_operation_started(state: &AppState, script: &str) {
    let enabled = match state_read_config_cached(state) {
        Ok(config) => config.desktop_operation_notice_enabled,
        Err(err) => {
            runtime_log_warn(format!(
                "[桌面操作提醒] 跳过，任务=读取通知设置失败，error={err}"
            ));
            return;
        }
    };
    if !enabled {
        return;
    }
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(poisoned) => poisoned.into_inner().as_ref().cloned(),
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    let action_count = script
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let body = format!("模型即将模拟鼠标/键盘操作你的电脑（脚本共 {action_count} 步），请注意不要与它同时操作。");
    // 异步发送，不等待通知提交，不阻塞工具执行；发送失败仅记录日志。
    tauri::async_runtime::spawn(async move {
        if let Err(err) = send_native_notification(&app_handle, "PAI 正在操作你的电脑", &body, false)
        {
            runtime_log_warn(format!("[桌面操作提醒] 通知发送失败：{err}"));
        }
    });
}

const OPERATE_TOOL_NAME: &str = "operate";
const WINDOWS_TOOL_NAME: &str = "windows";

#[derive(Debug, Clone)]
struct BuiltinOperateTool {
    app_state: AppState,
    model_supports_image: bool,
    session_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinWindowsTool {}

#[derive(Debug, Clone)]
struct BuiltinReadFileTool {
    app_state: AppState,
    session_id: String,
    api_config_id: String,
}

#[derive(Debug, Clone)]
struct BuiltinReadMediaTool {
    app_state: AppState,
}

impl RuntimeToolMetadata for BuiltinOperateTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        operate_provider_tool_definition()
    }
}

impl RuntimeValueTool for BuiltinOperateTool {
    const NAME: &'static str = OPERATE_TOOL_NAME;
    type Args = OperateRequest;
    type Error = ToolInvokeError;

    fn timeout_override(args_json: &str) -> Option<std::time::Duration> {
        Some(operate_tool_timeout_override(args_json))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        let model_supports_image = self.model_supports_image;
        let app_state = self.app_state.clone();
        // 截图按会话建目录：解析 session_id（agent_id::conversation_id）取 conversation_id，
        // 解析失败时用完整 session_id 兜底，保证不同会话目录天然隔离。
        let conversation_id = delegate_session_conversation_id(&self.session_id)
            .unwrap_or_else(|| self.session_id.clone());
        let screenshots_root = app_root_from_data_path(&self.app_state.data_path)
            .join("temp")
            .join("screenshots")
            .join(conversation_id);
        Box::pin(async move {
            // 电脑使用总开关兜底：装配后开关被关闭时，执行前再拦一次
            let operate_enabled = state_read_config_cached(&app_state)
                .map(|config| config.desktop_operate_enabled)
                .unwrap_or(true);
            if !operate_enabled {
                let err = ToolInvokeError::from("电脑使用已关闭，无法操作电脑".to_string());
                runtime_log_warn(format!("[工具执行] operate 被电脑使用总开关拦截: 错误={err}"));
                return Err(err);
            }
            // 截图始终可执行：驱动模型不支持图片时仍返回保存路径，
            // 是否携带 base64 由模型能力决定（不支持时跳过编码省 CPU）。
            let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=operate args={}",
                debug_value_snippet(&args_value, 240)
            ));
            let result = run_operate_tool(args, &screenshots_root, model_supports_image)
                .await
                .map_err(|err| ToolInvokeError::from(err.message))
                .and_then(|output| {
                    serde_json::to_value(output)
                        .map_err(|err| ToolInvokeError::from(format!("Serialize operate output failed: {err}")))
                });
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=operate result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 operate 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinWindowsTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        windows_provider_tool_definition()
    }
}

impl RuntimeValueTool for BuiltinWindowsTool {
    const NAME: &'static str = WINDOWS_TOOL_NAME;
    type Args = WindowsRequest;
    type Error = ToolInvokeError;

    fn timeout_override(args_json: &str) -> Option<std::time::Duration> {
        Some(windows_tool_timeout_override(args_json))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
        runtime_log_debug(format!(
            "[工具调试] 内置工具执行开始 name=windows args={}",
            debug_value_snippet(&args_value, 240)
        ));
        Box::pin(async move {
            // windows 工具是同步阻塞调用（EnumWindows/UIA 遍历可能数百 ms），
            // 放到阻塞线程池执行，避免占用 Tokio 工作线程（与 operate 控件树扫描一致）。
            let result = tokio::task::spawn_blocking(move || run_windows_tool(args))
                .await
                .map_err(|err| ToolInvokeError::from(format!("windows 工具任务异常: {err}")))
                .and_then(|inner| inner.map_err(|err| ToolInvokeError::from(err.message)))
                .and_then(|output| {
                    serde_json::to_value(output)
                        .map_err(|err| ToolInvokeError::from(format!("Serialize windows output failed: {err}")))
                });
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=windows result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 windows 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinReadFileTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        read_provider_tool_definition()
    }
}

impl RuntimeValueTool for BuiltinReadFileTool {
    const NAME: &'static str = READ_TOOL_NAME;
    type Args = ReadFileRequest;
    type Error = ToolInvokeError;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(300))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=read args={}",
                debug_value_snippet(&args_value, 240)
            ));
            let result = builtin_read_file(
                &self.app_state,
                &self.session_id,
                &self.api_config_id,
                args,
            )
            .await
            .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=read result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 read 执行失败: 错误={err}")),
            }
            result
        })
    }
}

impl RuntimeToolMetadata for BuiltinReadMediaTool {
    fn provider_tool_definition(&self) -> ProviderToolDefinition {
        read_media_provider_tool_definition()
    }
}

impl RuntimeValueTool for BuiltinReadMediaTool {
    const NAME: &'static str = READ_MEDIA_TOOL_NAME;
    type Args = ReadMediaToolArgs;
    type Error = ToolInvokeError;

    fn timeout_override(args_json: &str) -> Option<std::time::Duration> {
        Some(read_media_tool_timeout_override(args_json))
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error> {
        Box::pin(async move {
            let args_value = serde_json::to_value(&args).unwrap_or(Value::Null);
            runtime_log_debug(format!(
                "[工具调试] 内置工具执行开始 name=read_media args={}",
                debug_value_snippet(&args_value, 240)
            ));
            let result = builtin_read_media(
                &self.app_state,
                ReadMediaRequest {
                    path: args.path,
                    description: args.description,
                },
            )
            .await
            .map_err(ToolInvokeError::from);
            match &result {
                Ok(v) => runtime_log_debug(format!(
                    "[工具调试] 内置工具执行完成 name=read_media result={}",
                    debug_value_snippet(v, 240)
                )),
                Err(err) => runtime_log_error(format!("[工具执行] 内置工具 read_media 执行失败: 错误={err}")),
            }
            result
        })
    }
}

#[cfg(test)]
mod tool_assembly_permission_tests {
    use super::*;

    fn test_definition(name: &str) -> ProviderToolDefinition {
        ProviderToolDefinition::new(
            name,
            format!("{name} test tool"),
            serde_json::json!({"type": "object", "properties": {}}),
        )
    }

    fn test_memory_context(recall_enabled: bool) -> MemoryAgentContext {
        MemoryAgentContext {
            owner_agent_id: None,
            effective_agent_id: "agent-a".to_string(),
            private_memory_enabled: false,
            recall_enabled,
        }
    }

    fn whitelist_department(names: &[&str]) -> DepartmentConfig {
        let mut department = default_assistant_department("api-a");
        department.id = "department-a".to_string();
        department.is_built_in_assistant = false;
        department.permission_control = DepartmentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: names.iter().map(|name| (*name).to_string()).collect(),
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        };
        department
    }

    fn test_api() -> ApiConfig {
        let mut api = ApiConfig::default();
        api.id = "api-a".to_string();
        api.enable_tools = true;
        api
    }

    #[test]
    fn runtime_builtin_schema_registry_should_have_explicit_policy_entries() {
        let state = AppState::new().expect("create state for builtin policy coverage");
        let missing = build_global_tool_schema_cache(&state)
            .iter()
            .filter(|tool| matches!(tool.source, CachedRuntimeToolSource::Builtin))
            .map(|tool| tool.definition.name.clone())
            .filter(|tool_name| !builtin_tool_policy_is_explicit(tool_name))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "runtime builtins must have explicit policy entries: {missing:?}"
        );
    }

    #[test]
    fn legal_tool_resolver_should_not_attach_unchecked_config_in_whitelist() {
        let mut department = whitelist_department(&["delegate"]);
        department.child_department_ids = vec!["department-child".to_string()];
        let mut child = default_assistant_department("api-a");
        child.id = "department-child".to_string();
        child.is_built_in_assistant = false;
        let config = AppConfig {
            departments: vec![department.clone(), child],
            ..AppConfig::default()
        };
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("config")),
            CachedRuntimeToolSchema::builtin(test_definition("delegate")),
            CachedRuntimeToolSchema::builtin(test_definition("todo")),
        ];
        let memory = test_memory_context(true);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["delegate", "todo"]);
        assert!(resolved.manifest.iter().any(|item| {
            item.get("name").and_then(Value::as_str) == Some("config")
                && item.get("attached").and_then(Value::as_bool) == Some(false)
        }));
    }

    #[test]
    fn legal_tool_resolver_should_apply_default_preset_department_whitelists() {
        let mut config = AppConfig::default();
        config.vision_api_config_id = Some("vision-a".to_string());
        config.image_generation_model_id = Some("provider-a::model-a".to_string());
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("read")),
            CachedRuntimeToolSchema::builtin(test_definition("read_media")),
            CachedRuntimeToolSchema::builtin(test_definition("exec")),
            CachedRuntimeToolSchema::builtin(test_definition("fetch")),
            CachedRuntimeToolSchema::builtin(test_definition("websearch")),
            CachedRuntimeToolSchema::builtin(test_definition("write")),
            CachedRuntimeToolSchema::builtin(test_definition("update")),
            CachedRuntimeToolSchema::builtin(test_definition("delete")),
            CachedRuntimeToolSchema::builtin(test_definition("delegate")),
            CachedRuntimeToolSchema::builtin(test_definition("operate")),
            CachedRuntimeToolSchema::builtin(test_definition("meme")),
            CachedRuntimeToolSchema::builtin(test_definition("image_generate")),
            CachedRuntimeToolSchema::builtin(test_definition("image_edit")),
        ];
        let memory = test_memory_context(true);

        let explorer = config
            .departments
            .iter()
            .find(|department| department.id == DEPUTY_DEPARTMENT_ID)
            .expect("explorer department");
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(explorer),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["read", "read_media", "exec", "fetch", "websearch"]);

        let leader = config
            .departments
            .iter()
            .find(|department| department.id == LEADER_DEPARTMENT_ID)
            .expect("leader department");
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(leader),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["read", "read_media", "exec", "fetch", "websearch", "delegate"]
        );
        assert!(department_permission_allows_any_name(
            Some(leader),
            DepartmentPermissionCategory::Skill,
            &["memory-generation"],
        ));
        assert!(!department_permission_allows_any_name(
            Some(leader),
            DepartmentPermissionCategory::Skill,
            &["news-analyst"],
        ));

        let reviewer = config
            .departments
            .iter()
            .find(|department| department.id == REVIEWER_DEPARTMENT_ID)
            .expect("reviewer department");
        assert!(department_permission_allows_any_name(
            Some(reviewer),
            DepartmentPermissionCategory::Skill,
            &["code-review"],
        ));
        assert!(!department_permission_allows_any_name(
            Some(reviewer),
            DepartmentPermissionCategory::Skill,
            &["assistant-space-guide"],
        ));
        assert!(department_permission_allows_any_name(
            Some(reviewer),
            DepartmentPermissionCategory::Skill,
            &["memory-generation"],
        ));

        let saddler = config
            .departments
            .iter()
            .find(|department| department.id == SADDLER_DEPARTMENT_ID)
            .expect("saddler department");
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(saddler),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["read", "exec", "write", "update"]);

        let remote_customer_service = config
            .departments
            .iter()
            .find(|department| department.id == REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID)
            .expect("remote customer service department");
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(remote_customer_service),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "read",
                "read_media",
                "fetch",
                "websearch",
                "meme",
                "image_generate",
                "image_edit",
            ]
        );
        assert!(department_permission_allows_any_name(
            Some(remote_customer_service),
            DepartmentPermissionCategory::Skill,
            &["news-analyst"],
        ));
        assert!(department_permission_allows_any_name(
            Some(remote_customer_service),
            DepartmentPermissionCategory::Skill,
            &["memory-generation"],
        ));
    }

    #[test]
    fn legal_tool_resolver_should_degrade_missing_department_without_stopping_fixed_tools() {
        let config = AppConfig::default();
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("config")),
            CachedRuntimeToolSchema::builtin(test_definition("todo")),
            CachedRuntimeToolSchema::mcp(
                "server-id",
                "server-name",
                "server-name_search",
                None,
                test_definition("search"),
            ),
        ];
        let memory = test_memory_context(true);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            None,
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["todo"]);
    }

    #[test]
    fn legal_tool_resolver_should_keep_mcp_permission_compatibility_names() {
        let mut department = whitelist_department(&[]);
        department.permission_control.mcp_tool_names = vec!["server-id::search".to_string()];
        let config = AppConfig {
            departments: vec![department.clone()],
            ..AppConfig::default()
        };
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![CachedRuntimeToolSchema::mcp(
            "server-id",
            "server-name",
            "server-name_search",
            None,
            test_definition("search"),
        )];
        let memory = test_memory_context(true);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        assert_eq!(resolved.attached.len(), 1);

        department.permission_control.mcp_tool_names = vec!["search".to_string()];
        let config = AppConfig {
            departments: vec![department.clone()],
            ..AppConfig::default()
        };
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        assert_eq!(resolved.attached.len(), 1);
    }

    #[test]
    fn legal_tool_resolver_skips_mcp_with_a_runtime_compatibility_error() {
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![CachedRuntimeToolSchema::mcp(
            "server-id",
            "server-name",
            "中文成员_search",
            Some("MCP 组成员名规范化后没有可用字符，工具无法挂载".to_string()),
            test_definition("search"),
        )];
        let resolved = resolve_legal_runtime_tools_for_department(
            &AppConfig::default(),
            &test_api(),
            None,
            &policy,
            Some(&test_memory_context(true)),
            &tools,
        );

        assert!(resolved.attached.is_empty());
        assert_eq!(
            resolved.manifest[0].get("reason").and_then(Value::as_str),
            Some("MCP 组成员名规范化后没有可用字符，工具无法挂载")
        );
    }

    #[test]
    fn legal_tool_resolver_should_remove_memory_tools_when_agent_memory_is_disabled() {
        let department = whitelist_department(&[]);
        let config = AppConfig {
            departments: vec![department.clone()],
            ..AppConfig::default()
        };
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("remember")),
            CachedRuntimeToolSchema::builtin(test_definition("recall")),
        ];
        let memory = test_memory_context(false);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.is_empty());
    }

    #[test]
    fn legal_tool_resolver_should_skip_media_tools_without_default_models() {
        let department = whitelist_department(&["image_generate", "image_edit", "read_media"]);
        let mut config = AppConfig {
            departments: vec![department.clone()],
            ..AppConfig::default()
        };
        config.image_generation_model_id = None;
        config.vision_api_config_id = None;
        let policy = RuntimeToolPolicy {
            conversation_resolved: true,
            local_conversation: true,
            ..RuntimeToolPolicy::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("image_generate")),
            CachedRuntimeToolSchema::builtin(test_definition("image_edit")),
            CachedRuntimeToolSchema::builtin(test_definition("read_media")),
        ];
        let memory = test_memory_context(true);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        assert!(resolved.attached.is_empty());
        assert_eq!(resolved.manifest.len(), 3);

        config.image_generation_model_id = Some("provider-a::model-a".to_string());
        config.vision_api_config_id = Some("vision-a".to_string());
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &policy,
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["image_generate", "image_edit", "read_media"]);
    }

    #[test]
    fn legal_tool_resolver_should_skip_context_sensitive_tools_when_conversation_read_fails() {
        let department = whitelist_department(&[]);
        let config = AppConfig {
            departments: vec![department.clone()],
            ..AppConfig::default()
        };
        let tools = vec![
            CachedRuntimeToolSchema::builtin(test_definition("task")),
            CachedRuntimeToolSchema::builtin(test_definition("plan")),
            CachedRuntimeToolSchema::builtin(test_definition("todo")),
        ];
        let memory = test_memory_context(true);
        let resolved = resolve_legal_runtime_tools_for_department(
            &config,
            &test_api(),
            Some(&department),
            &RuntimeToolPolicy::default(),
            Some(&memory),
            &tools,
        );
        let names = resolved
            .attached
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["todo"]);
    }
}
