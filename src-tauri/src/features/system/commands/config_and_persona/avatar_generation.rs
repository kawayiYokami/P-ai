// ==================== 人格头像生成 ====================
// 意图：人格没有头像时，用快速模型按人设生成一张 SVG 头像并落盘复用；失败就继续用品牌图标。
// 安全前提：头像 SVG 只通过 <img src="data:image/svg+xml;base64,…"> 显示，
// 任何地方都不许把它内联进 DOM 或交给 v-html；落盘前的清洗是第二道防线，两者都不能省。

/// 头像生成结果广播：配置窗口与聊天窗口都靠它当场换图，或让用户看见失败。
const AGENT_AVATAR_GENERATED_EVENT: &str = "easy-call:persona-avatar-generated";

const AGENT_AVATAR_SVG_MAX_BYTES: usize = 32 * 1024;
/// 队列是串行的，超时太长会把后面排队的头像一起卡死；实测最慢一次 51s。
const AGENT_AVATAR_SVG_TIMEOUT_SECS: u64 = 60;

/// 引导段：红豆与队长逐字定稿的一句话，不要改写、不要增删要点。
/// 关键在「不用画人脸」——早先那版要求画发型、瞳色、半侧脸，等于让它画脸，与要求相反。
const AGENT_AVATAR_SVG_CONTENT_PROMPT: &str = "请为自己画一张头像，用它来代表你。不用画人脸——一片叶子、一团光、一个符号都行，挑一个你觉得最像自己的东西，画到让人一眼能感到你的性情。";

/// 格式段：同样逐字定稿。三条硬约束分别对应圆形裁切、40 像素显示和实心填充。
const AGENT_AVATAR_SVG_FORMAT_PROMPT: &str = "输出格式（严格遵守）：\n\
1. 只输出一个 <svg> 元素，不要 markdown 代码围栏，不要任何解释文字。\n\
2. viewBox=\"0 0 120 120\"；这是圆形头像，所有图形必须完整落在以 (60,60) 为圆心、半径 56 的圆内，圆外不得有内容。\n\
3. 这张头像在界面上以 40 像素显示，请按 40 像素的观感来决定形状与笔画粗细，缩小后仍要一眼认得出主体。\n\
4. 图形必须实心填充，不得只用描边线条。\n\
5. 不使用文字、不引用外部资源、不包含 script。\n\
6. 总长度不超过 8KB。";

/// 整段丢弃的元素：脚本、能内嵌 HTML 的外来对象，以及可携带外部 url() 的样式表。
const AGENT_AVATAR_SVG_DROPPED_ELEMENTS: &[&str] = &["script", "foreignobject", "style"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateAgentAvatarInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateAgentAvatarOutput {
    data_url: String,
    path: String,
    updated_at: String,
}

/// 这次生成是谁触发的：决定失败卡片弹在哪个窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentAvatarGenerationOrigin {
    /// 聊天里招募/新建人格（config 工具）。
    Chat,
    /// 配置窗口保存人格。
    Config,
}

impl AgentAvatarGenerationOrigin {
    /// 只在后台任务里序列化进失败事件；test 编译单元的 spawn 是空实现，用不到它。
    #[cfg(not(test))]
    fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Config => "config",
        }
    }
}

/// 一次头像生成的结果：成功带图路径，失败带原因。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentAvatarGeneratedPayload {
    agent_id: String,
    /// 仅失败事件带：决定失败卡片弹在哪个窗口；成功事件两个窗口都该更新，无需区分。
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 广播一次生成结果。取不到 AppHandle 时静默跳过：生成本身已经完成，窗口下次加载会自愈。
fn emit_agent_avatar_generated(state: &AppState, payload: AgentAvatarGeneratedPayload) {
    let agent_id = payload.agent_id.clone();
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    if let Err(err) = app_handle.emit(AGENT_AVATAR_GENERATED_EVENT, &payload) {
        runtime_log_warn(format!(
            "[头像生成] 失败，agentId={agent_id}，stage=广播，error={err}"
        ));
    }
}

fn agent_avatar_generation_inflight() -> &'static Mutex<std::collections::HashSet<String>> {
    static KEYS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    KEYS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// 入队判重表：已经在排队或正在生成的人格。
///
/// 只解决「同一时刻重复入队」——招募时工具排一次、随后的保存又排一次，第二次必须丢掉，
/// 否则它会在生成入口撞上在途凭证，被当成一次「生成失败」推给界面。
/// 生成结束（成功、失败、跳过）立刻释放：这不是拉黑，失败之后用户再保存仍然照常重跑。
fn agent_avatar_generation_queue() -> &'static Mutex<std::collections::HashSet<String>> {
    static KEYS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    KEYS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// 占一个入队名额；同一个人格已经在场时返回 false，调用方直接丢弃这次入队。
fn reserve_avatar_generation_queue(key: &str) -> bool {
    match agent_avatar_generation_queue().lock() {
        Ok(mut keys) => keys.insert(key.to_string()),
        // 表坏了就别丢任务：宁可多跑一张，也不要连头像都生成不出来。
        Err(_) => true,
    }
}

fn release_avatar_generation_queue(key: &str) {
    if let Ok(mut keys) = agent_avatar_generation_queue().lock() {
        keys.remove(key);
    }
}

/// 全局串行闸门：一次只发一张，前一张结束（无论成败）才轮到下一张。
/// 供应商会返 503，同时打五六个必然有几个失败，所以不并发。
#[cfg(not(test))]
fn agent_avatar_generation_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// 这次保存该排队生成头像的人格：头像为空，且要么是新增人格、要么是头像被清空的老人格。
///
/// 前端提交的是全量人格列表，不能按「谁没头像」排——否则保存任意一个人格，
/// 库里所有缺图的老人格都会被拉起来逐个生成，卡片一张盖一张。
fn agent_avatar_generation_pending_ids<'a>(
    agents: impl IntoIterator<Item = &'a AgentProfile>,
    previous_by_id: &std::collections::HashMap<String, &AgentProfile>,
) -> Vec<String> {
    agents
        .into_iter()
        .filter(|agent| !agent.is_built_in_user && agent.id != USER_PERSONA_ID)
        .filter(|agent| {
            if agent_has_avatar(agent) {
                return false;
            }
            match previous_by_id.get(agent.id.as_str()) {
                // 新增人格：这次才第一次有机会拿到头像。
                None => true,
                // 老人格：只有头像被清空过才重新生成，原本就没头像的不该被他这一次保存拉起来。
                Some(previous) => agent_has_avatar(previous),
            }
        })
        .map(|agent| agent.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

fn agent_has_avatar(agent: &AgentProfile) -> bool {
    agent
        .avatar_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

/// 入队名额的持有者：任务结束（成功、失败、提前返回、panic）都自动归还。
#[cfg(not(test))]
struct AgentAvatarGenerationQueueSlot {
    key: String,
}

#[cfg(not(test))]
impl Drop for AgentAvatarGenerationQueueSlot {
    fn drop(&mut self) {
        release_avatar_generation_queue(&self.key);
    }
}

#[cfg(not(test))]
fn spawn_agent_avatar_generation(
    state: AppState,
    agent_id: String,
    origin: AgentAvatarGenerationOrigin,
) {
    tauri::async_runtime::spawn(async move {
        // 名额跟着任务走：任务一结束就还回去，失败不是终点，下次保存还得能重跑。
        // 任务若根本没被调度（应用正在退出），名额会留在内存里——重启即清空，不影响下次运行。
        let _slot = AgentAvatarGenerationQueueSlot {
            key: agent_id.clone(),
        };
        // 等到前一张结束再发；锁在任务里取，enqueue 调用方立刻返回。
        let _permit = agent_avatar_generation_gate().lock().await;
        let input = GenerateAgentAvatarInput {
            agent_id: agent_id.clone(),
        };
        match generate_agent_avatar_inner(input, &state).await {
            // 未配置快速模型＝用户主动禁用：静默跳过，不弹卡、不记失败、不消耗额度。
            Ok(None) => {
                runtime_log_info(format!(
                    "[头像生成] 跳过，agentId={agent_id}，stage=未配置快速模型"
                ));
            }
            Ok(Some(_)) => {}
            Err(err) => {
                runtime_log_warn(format!(
                    "[头像生成] 失败，agentId={agent_id}，stage=生成，error={err}"
                ));
                // 失败必须让界面看得见：用户据此去换一个可用的快速模型。
                emit_agent_avatar_generated(
                    &state,
                    AgentAvatarGeneratedPayload {
                        agent_id: agent_id.clone(),
                        origin: Some(origin.as_str().to_string()),
                        ok: false,
                        path: None,
                        updated_at: None,
                        error: Some(err),
                    },
                );
            }
        }
    });
}

/// 测试探针：记录本次测试进程内经 enqueue 排队的 agent id。
/// 只在 test 编译单元存在，生产路径不读它，也不参与任何「要不要重试」的判断。
#[cfg(test)]
fn agent_avatar_generation_test_queued() -> &'static Mutex<Vec<String>> {
    static QUEUED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    QUEUED.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(test)]
fn spawn_agent_avatar_generation(
    _state: AppState,
    agent_id: String,
    _origin: AgentAvatarGenerationOrigin,
) {
    if let Ok(mut queued) = agent_avatar_generation_test_queued().lock() {
        queued.push(agent_id);
    }
    // 测试里不自动归还在队名额：那张「图」永远不会跑完，由用例显式调 finish 模拟结束。
}

/// 测试钩子：模拟「这一张已经跑完（成功/失败/跳过都算）」，把入队名额还回去。
#[cfg(test)]
fn agent_avatar_generation_test_finish(agent_id: &str) {
    release_avatar_generation_queue(agent_id);
}

/// 给一个人格排队生成头像：只入队，不等生成，调用方（工具执行、人格保存）立刻返回。
/// 不做「本次运行只试一次」的拉黑：失败原因是模型不可用这类外部状态，
/// 该由调用方下次保存人格时重试，而不是永久放弃这个人的头像。
/// 未配置快速模型时会照常入队，但在生成入口判空并静默跳过：那是禁用，不是失败。
fn enqueue_agent_avatar_generation(
    state: &AppState,
    agent_id: &str,
    origin: AgentAvatarGenerationOrigin,
) {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return;
    }
    // 同一时刻只排一张：招募时工具排一次、随后的保存又排一次，第二次丢掉。
    if !reserve_avatar_generation_queue(agent_id) {
        runtime_log_info(format!(
            "[头像生成] 跳过，agentId={agent_id}，stage=已在队列或生成中"
        ));
        return;
    }
    spawn_agent_avatar_generation(state.clone(), agent_id.to_string(), origin);
}

/// 在途去重凭证：持有期间代表该人格正在生成，Drop 时自动释放。
struct AgentAvatarGenerationTicket {
    key: String,
}

impl AgentAvatarGenerationTicket {
    fn acquire(key: &str) -> Result<Self, String> {
        let mut inflight = agent_avatar_generation_inflight()
            .lock()
            .map_err(|_| "头像生成任务表已损坏".to_string())?;
        if !inflight.insert(key.to_string()) {
            return Err(format!("Agent '{key}' 的头像正在生成中"));
        }
        Ok(Self {
            key: key.to_string(),
        })
    }
}

impl Drop for AgentAvatarGenerationTicket {
    fn drop(&mut self) {
        if let Ok(mut inflight) = agent_avatar_generation_inflight().lock() {
            inflight.remove(&self.key);
        }
    }
}

fn agent_avatar_svg_prepared_prompt(agent: &AgentProfile) -> PreparedPrompt {
    // 人格自己就是 system：全文照搬，不截断、不加前后缀，否则长人设后半段会被丢掉。
    // user 段只放画图请求：引导在前、格式在后，中间留一个空行。
    let latest_user_text = format!(
        "{}\n\n{}",
        AGENT_AVATAR_SVG_CONTENT_PROMPT, AGENT_AVATAR_SVG_FORMAT_PROMPT,
    );
    PreparedPrompt {
        preamble: agent.system_prompt.clone(),
        history_messages: Vec::new(),
        latest_user_text,
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_user_trailing_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    }
}

/// 从模型输出里抠出 `<svg …</svg>`：模型常把结果塞进代码围栏或前后加解释。
fn extract_agent_avatar_svg_markup(raw: &str) -> Option<&str> {
    // to_ascii_lowercase 只改 ASCII，字节长度不变，索引可直接用于原串切片。
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("<svg")?;
    let end = lower.rfind("</svg>")? + "</svg>".len();
    if end <= start {
        return None;
    }
    Some(&raw[start..end])
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AvatarSvgTag {
    /// 标签名，已转小写。
    name: String,
    /// 已过滤过的属性，名字保留原始大小写。
    attributes: Vec<(String, String)>,
    closing: bool,
    self_closing: bool,
}

impl AvatarSvgTag {
    fn render_open(&self) -> String {
        let mut out = String::with_capacity(32);
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attributes {
            // 值来自单/双引号包裹的原文，本身不可能同时含两种引号。
            let quote = if value.contains('"') { '\'' } else { '"' };
            out.push(' ');
            out.push_str(name);
            out.push('=');
            out.push(quote);
            out.push_str(value);
            out.push(quote);
        }
        if self.self_closing {
            out.push_str("/>");
        } else {
            out.push('>');
        }
        out
    }

    fn render_close(&self) -> String {
        format!("</{}>", self.name)
    }
}

/// 校验属性是否可以留下：事件属性、脚本协议、外部引用一律剥掉。
fn avatar_svg_attribute_allowed(name: &str, value: &str) -> bool {
    let lower_name = name.to_ascii_lowercase();
    if lower_name.starts_with("on") {
        return false;
    }
    let lower_value = value.trim().to_ascii_lowercase();
    if lower_value.contains("javascript:") {
        return false;
    }
    // href 只保留页内锚点：渐变、裁剪等还要靠 #id 引用。
    if lower_name == "href" || lower_name.ends_with(":href") {
        return lower_value.starts_with('#');
    }
    // 样式里不允许指向外部的 url()，页内 url(#id) 保留。
    if lower_name == "style" && lower_value.contains("url(") {
        return lower_value.contains("url(#");
    }
    true
}

/// 解析 `<` 开头的一个标签，返回标签本身和它之后的字节位置。
fn parse_avatar_svg_tag(source: &str, start: usize) -> Result<(AvatarSvgTag, usize), String> {
    let bytes = source.as_bytes();
    let mut index = start + 1;
    let mut closing = false;
    if bytes.get(index) == Some(&b'/') {
        closing = true;
        index += 1;
    }
    let name_start = index;
    while let Some(&byte) = bytes.get(index) {
        if byte.is_ascii_whitespace() || byte == b'>' || byte == b'/' {
            break;
        }
        index += 1;
    }
    let name = source[name_start..index].to_ascii_lowercase();
    if name.is_empty() {
        return Err("头像 SVG 标签缺少名字".to_string());
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        return Err(format!("头像 SVG 标签名不合法：{name}"));
    }

    let mut attributes = Vec::<(String, String)>::new();
    let mut self_closing = false;
    loop {
        while let Some(&byte) = bytes.get(index) {
            if !byte.is_ascii_whitespace() {
                break;
            }
            index += 1;
        }
        match bytes.get(index) {
            None => return Err(format!("头像 SVG 标签未闭合：{name}")),
            Some(&b'>') => {
                index += 1;
                break;
            }
            Some(&b'/') => {
                if bytes.get(index + 1) != Some(&b'>') {
                    return Err(format!("头像 SVG 标签里的 / 位置不合法：{name}"));
                }
                self_closing = true;
                index += 2;
                break;
            }
            Some(_) => {
                if closing {
                    return Err(format!("头像 SVG 结束标签不应带属性：{name}"));
                }
                let attr_start = index;
                while let Some(&byte) = bytes.get(index) {
                    if byte.is_ascii_whitespace() || byte == b'=' || byte == b'>' || byte == b'/' {
                        break;
                    }
                    index += 1;
                }
                let attr_name = source[attr_start..index].to_string();
                if attr_name.is_empty() {
                    return Err(format!("头像 SVG 属性名缺失：{name}"));
                }
                while let Some(&byte) = bytes.get(index) {
                    if !byte.is_ascii_whitespace() {
                        break;
                    }
                    index += 1;
                }
                if bytes.get(index) != Some(&b'=') {
                    return Err(format!("头像 SVG 属性缺少取值：{attr_name}"));
                }
                index += 1;
                while let Some(&byte) = bytes.get(index) {
                    if !byte.is_ascii_whitespace() {
                        break;
                    }
                    index += 1;
                }
                let quote = match bytes.get(index) {
                    Some(&byte) if byte == b'"' || byte == b'\'' => byte,
                    _ => return Err(format!("头像 SVG 属性值必须用引号包裹：{attr_name}")),
                };
                index += 1;
                let value_start = index;
                while let Some(&byte) = bytes.get(index) {
                    if byte == quote {
                        break;
                    }
                    index += 1;
                }
                if bytes.get(index) != Some(&quote) {
                    return Err(format!("头像 SVG 属性值引号未闭合：{attr_name}"));
                }
                let value = source[value_start..index].to_string();
                index += 1;
                if avatar_svg_attribute_allowed(&attr_name, &value) {
                    attributes.push((attr_name, value));
                }
            }
        }
    }

    Ok((
        AvatarSvgTag {
            name,
            attributes,
            closing,
            self_closing,
        },
        index,
    ))
}

fn push_avatar_svg_text(
    out: &mut String,
    text: &str,
    open_elements: &[String],
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }
    if open_elements.is_empty() {
        return Err("头像 SVG 根元素外出现文本".to_string());
    }
    out.push_str(text);
    Ok(())
}

/// 清洗并校验模型产出的 SVG：剥掉危险内容、确认结构完好、卡住体积上限。
/// 只剥危险内容，不改画风；剥完不是一张合法 SVG 就整张丢弃。
fn sanitize_agent_avatar_svg(raw: &str) -> Result<String, String> {
    let body = extract_agent_avatar_svg_markup(raw)
        .ok_or_else(|| "模型返回的内容里没有完整的 <svg> 元素".to_string())?;
    let mut out = String::with_capacity(body.len());
    let mut open_elements = Vec::<String>::new();
    let mut skip_element: Option<(String, usize)> = None;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut index = 0usize;
    while index < body.len() {
        let Some(offset) = body[index..].find('<') else {
            if skip_element.is_none() {
                push_avatar_svg_text(&mut out, &body[index..], &open_elements)?;
            }
            break;
        };
        let tag_start = index + offset;
        if tag_start > index && skip_element.is_none() {
            push_avatar_svg_text(&mut out, &body[index..tag_start], &open_elements)?;
        }
        index = tag_start;
        // 注释、CDATA、DOCTYPE、处理指令：一律丢掉。
        if body[index..].starts_with("<!--") {
            match body[index..].find("-->") {
                Some(end) => index += end + 3,
                None => return Err("头像 SVG 注释没有闭合".to_string()),
            }
            continue;
        }
        if body[index..].starts_with("<![CDATA[") {
            match body[index..].find("]]>") {
                Some(end) => index += end + 3,
                None => return Err("头像 SVG CDATA 没有闭合".to_string()),
            }
            continue;
        }
        if body[index..].starts_with("<!") || body[index..].starts_with("<?") {
            match body[index..].find('>') {
                Some(end) => index += end + 1,
                None => return Err("头像 SVG 声明没有闭合".to_string()),
            }
            continue;
        }
        let (mut tag, next_index) = parse_avatar_svg_tag(body, index)?;
        index = next_index;
        // 被丢弃的元素内部：只跟踪同名嵌套深度，内容一个字都不输出。
        if let Some((skip_name, depth)) = skip_element.as_mut() {
            if tag.name == *skip_name {
                if tag.closing {
                    *depth -= 1;
                    if *depth == 0 {
                        skip_element = None;
                    }
                } else if !tag.self_closing {
                    *depth += 1;
                }
            }
            continue;
        }
        if AGENT_AVATAR_SVG_DROPPED_ELEMENTS.contains(&tag.name.as_str()) {
            if !tag.closing && !tag.self_closing {
                skip_element = Some((tag.name.clone(), 1));
            }
            continue;
        }
        if tag.closing {
            match open_elements.last() {
                Some(name) if name == &tag.name => {}
                _ => return Err(format!("头像 SVG 标签没有正确闭合：{}", tag.name)),
            }
            open_elements.pop();
            out.push_str(&tag.render_close());
            if open_elements.is_empty() {
                root_closed = true;
            }
            continue;
        }
        if open_elements.is_empty() {
            if root_seen {
                return Err("头像 SVG 出现了多个根元素".to_string());
            }
            if tag.name != "svg" {
                return Err(format!("头像 SVG 的根元素必须是 <svg>：{}", tag.name));
            }
            // 缺命名空间的 SVG 在 <img> 里渲染不出来，补上；模型给了就尊重原值。
            if !tag
                .attributes
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("xmlns"))
            {
                tag.attributes.push((
                    "xmlns".to_string(),
                    "http://www.w3.org/2000/svg".to_string(),
                ));
            }
            root_seen = true;
        }
        out.push_str(&tag.render_open());
        if tag.self_closing {
            if open_elements.is_empty() {
                root_closed = true;
            }
        } else {
            open_elements.push(tag.name.clone());
        }
    }
    if !root_seen {
        return Err("头像 SVG 缺少 <svg> 根元素".to_string());
    }
    if !open_elements.is_empty() || !root_closed {
        return Err("头像 SVG 没有完整闭合".to_string());
    }
    if out.len() > AGENT_AVATAR_SVG_MAX_BYTES {
        return Err(format!(
            "头像 SVG 超过 {AGENT_AVATAR_SVG_MAX_BYTES} 字节上限（实际 {} 字节）",
            out.len()
        ));
    }
    Ok(out)
}

#[tauri::command]
async fn generate_agent_avatar(
    input: GenerateAgentAvatarInput,
    state: State<'_, AppState>,
) -> Result<GenerateAgentAvatarOutput, String> {
    let agent_id = input.agent_id.trim().to_string();
    match generate_agent_avatar_inner(input, state.inner()).await {
        Ok(Some(output)) => Ok(output),
        // 用户主动点生成时要明说；后台那条路把 None 当静默跳过。
        Ok(None) => Err("未配置快速模型，无法生成人格头像".to_string()),
        Err(err) => {
            runtime_log_warn(format!("[头像生成] 失败，agentId={agent_id}，error={err}"));
            Err(err)
        }
    }
}

/// 返回 `Ok(None)` 表示未配置快速模型、按禁用跳过，区别于「已有头像」的回读。
async fn generate_agent_avatar_inner(
    input: GenerateAgentAvatarInput,
    state: &AppState,
) -> Result<Option<GenerateAgentAvatarOutput>, String> {
    let agent_id = input.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }
    let agents = state_read_agents_cached(state)?;
    let base_config = read_config(&state.config_path)?;
    let runtime_agents = runtime_agents_with_private_organization(
        state,
        &base_config,
        &AppData {
            agents: agents.clone(),
            ..Default::default()
        },
    )?;
    let target = runtime_agents
        .iter()
        .find(|agent| agent.id == agent_id)
        .cloned()
        .ok_or_else(|| format!("Agent '{agent_id}' not found."))?;
    if target.is_built_in_user || target.id == USER_PERSONA_ID {
        return Err("内置用户人格不生成头像".to_string());
    }
    // 已经有头像就直接回读，不覆盖用户自己设的图。
    let existing_path = target
        .avatar_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(path) = existing_path {
        // 不覆盖用户自己设的图，但要留一条痕：否则「点了没反应」永远查不清是跳过了还是没跑。
        runtime_log_info(format!(
            "[头像生成] 跳过，agentId={agent_id}，stage=已有头像，path={path}"
        ));
        let existing =
            read_avatar_data_url_inner(AvatarDataPathInput { path: path.clone() }, state)?;
        return Ok(Some(GenerateAgentAvatarOutput {
            data_url: existing.data_url,
            path,
            updated_at: target.avatar_updated_at.clone().unwrap_or_default(),
        }));
    }

    // 同一人格同时只跑一次生成，避免前端重复触发时重复消耗额度。
    let _ticket = AgentAvatarGenerationTicket::acquire(&agent_id)?;
    let app_config = state_read_config_cached(state)?;
    let Some(api_config_id) =
        resolve_model_role_api_config_id(&app_config, MODEL_ROLE_QUICK_API_CONFIG_ID)
    else {
        // 未配置＝用户禁用快速请求，不是错误：交给调用方决定静默跳过还是明说。
        return Ok(None);
    };
    let started_at = std::time::Instant::now();
    let prepared = agent_avatar_svg_prepared_prompt(&target);
    let reply = invoke_quick_model_reply_with_prepared_prompt(
        state,
        &api_config_id,
        prepared,
        Some(AGENT_AVATAR_SVG_TIMEOUT_SECS),
    )
    .await?;
    let svg = sanitize_agent_avatar_svg(&fast_request_response_text_from_reply(&reply))?;

    let dir = avatar_storage_dir(state)?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建头像目录失败：{err}"))?;
    let safe_id = sanitize_avatar_key(&agent_id);
    let file_name = format!("agent-{safe_id}.svg");
    fs::write(dir.join(&file_name), svg.as_bytes())
        .map_err(|err| format!("写入头像文件失败：{err}"))?;

    let now = now_iso();
    // 存相对数据根的路径，数据根整体搬迁后仍可解析。
    let avatar_path = avatar_relative_path(&file_name);
    if is_private_workspace_source(&target.source) {
        let mut next_runtime_agents = runtime_agents.clone();
        let index = next_runtime_agents
            .iter()
            .position(|agent| agent.id == agent_id)
            .ok_or_else(|| format!("Agent '{agent_id}' not found."))?;
        next_runtime_agents[index].avatar_path = Some(avatar_path.clone());
        next_runtime_agents[index].avatar_updated_at = Some(now.clone());
        next_runtime_agents[index].updated_at = now.clone();
        sync_private_agents_to_workspace(&state.data_path, &base_config, &next_runtime_agents)?;
    } else {
        let mut next_agents = agents;
        let index = next_agents
            .iter()
            .position(|agent| agent.id == agent_id)
            .ok_or_else(|| format!("Agent '{agent_id}' not found."))?;
        next_agents[index].avatar_path = Some(avatar_path.clone());
        next_agents[index].avatar_updated_at = Some(now.clone());
        next_agents[index].updated_at = now.clone();
        state_write_agents_cached(state, &next_agents)?;
    }

    runtime_log_info(format!(
        "[头像生成] 完成，agentId={}，字节={}，耗时={}ms",
        agent_id,
        svg.len(),
        started_at.elapsed().as_millis()
    ));
    // 侧边栏走 IDE 通知；桌面窗口与 Web 端靠下面这条事件当场换图。
    broadcast_sidebar_persona_changed();
    emit_agent_avatar_generated(
        state,
        AgentAvatarGeneratedPayload {
            agent_id: agent_id.clone(),
            origin: None,
            ok: true,
            path: Some(avatar_path.clone()),
            updated_at: Some(now.clone()),
            error: None,
        },
    );
    Ok(Some(GenerateAgentAvatarOutput {
        data_url: format!(
            "data:image/svg+xml;base64,{}",
            B64.encode(svg.as_bytes())
        ),
        path: avatar_path,
        updated_at: now,
    }))
}
