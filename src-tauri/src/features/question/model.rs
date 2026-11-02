// ==================== 会话提问：实体与校验 ====================

const QUESTION_DB_FILE_NAME: &str = "question_store.db";
/// 单会话同时最多保留的问题数：面板在输入框上方，题多了会把对话区挤没。
const QUESTION_MAX_PER_CONVERSATION: usize = 8;
const QUESTION_TITLE_MAX_CHARS: usize = 120;
const QUESTION_BACKGROUND_MAX_CHARS: usize = 1000;
const QUESTION_OPTION_MAX_CHARS: usize = 60;
/// 用户自己写的答案上限：比选项宽得多，但必须有一道闸，避免把整段上下文塞进答复消息。
const QUESTION_CUSTOM_ANSWER_MAX_CHARS: usize = 1000;
const QUESTION_OPTIONS_MAX: usize = 6;

/// 存储实体：`id` 仅作主键，不进工具参数、不上界面；对外一律按 `number` 引用。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConversationQuestion {
    id: String,
    conversation_id: String,
    number: u32,
    title: String,
    background: String,
    options: Vec<String>,
    created_at: String,
    updated_at: String,
}

impl ConversationQuestion {
    fn to_view(&self) -> ConversationQuestionView {
        ConversationQuestionView {
            number: self.number,
            title: self.title.clone(),
            background: self.background.clone(),
            options: self.options.clone(),
        }
    }
}

/// 对外视图：只出现编号，不出现内部 id。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationQuestionView {
    number: u32,
    title: String,
    #[serde(default)]
    background: String,
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionUpdateItem {
    /// 带编号 = 更新或删除；不带编号 = 新建。
    #[serde(default)]
    number: Option<u32>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionToolRequest {
    #[serde(default)]
    action: String,
    #[serde(default)]
    items: Vec<QuestionUpdateItem>,
}

/// 一次 `update` 调用里对单个条目的处置意图。
#[derive(Debug, Clone, PartialEq, Eq)]
enum QuestionMutation {
    Create {
        title: String,
        background: String,
        options: Vec<String>,
    },
    Update {
        number: u32,
        title: String,
        background: String,
        options: Vec<String>,
    },
    Delete {
        number: u32,
    },
}

fn question_trimmed_opt(raw: &Option<String>) -> String {
    raw.as_deref().map(str::trim).unwrap_or("").to_string()
}

/// 选项统一清洗：去空白、丢空项、保留顺序。
fn question_options_normalized(raw: &Option<Vec<String>>) -> Vec<String> {
    raw.as_deref()
        .unwrap_or_default()
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn question_char_count(value: &str) -> usize {
    value.chars().count()
}

fn question_validate_title(title: &str) -> Result<(), String> {
    if question_char_count(title) > QUESTION_TITLE_MAX_CHARS {
        return Err(format!(
            "问题超过 {} 字上限，请精简后重试",
            QUESTION_TITLE_MAX_CHARS
        ));
    }
    Ok(())
}

fn question_validate_background(background: &str) -> Result<(), String> {
    if question_char_count(background) > QUESTION_BACKGROUND_MAX_CHARS {
        return Err(format!(
            "问题背景超过 {} 字上限，请精简后重试",
            QUESTION_BACKGROUND_MAX_CHARS
        ));
    }
    Ok(())
}

fn question_validate_options(options: &[String]) -> Result<(), String> {
    if options.is_empty() {
        return Err("问题至少要有一个选项".to_string());
    }
    if options.len() > QUESTION_OPTIONS_MAX {
        return Err(format!(
            "单个问题最多 {} 个选项，请合并或精简",
            QUESTION_OPTIONS_MAX
        ));
    }
    for option in options {
        if question_char_count(option) > QUESTION_OPTION_MAX_CHARS {
            return Err(format!(
                "选项「{}」超过 {} 字上限，请精简后重试",
                option, QUESTION_OPTION_MAX_CHARS
            ));
        }
    }
    Ok(())
}

/// 把一条入参解释成处置意图。判定顺序写死，避免实现漂移：
/// 1) 有编号 + 三项全空 → 删除；2) 有编号 + 有内容 → 覆盖更新；
/// 3) 无编号 + 有内容 → 新建；4) 无编号 + 全空 → 参数错误。
fn question_mutation_from_item(item: &QuestionUpdateItem) -> Result<QuestionMutation, String> {
    let title = question_trimmed_opt(&item.title);
    let background = question_trimmed_opt(&item.background);
    let options = question_options_normalized(&item.options);
    let is_blank = title.is_empty() && background.is_empty() && options.is_empty();

    match item.number {
        Some(number) if number == 0 => Err("问题编号从 1 开始，0 不是合法编号".to_string()),
        Some(number) if is_blank => Ok(QuestionMutation::Delete { number }),
        Some(number) => {
            if title.is_empty() {
                return Err(format!(
                    "#{number} 的更新缺少问题内容：清空问题应同时清空背景与选项"
                ));
            }
            question_validate_title(&title)?;
            question_validate_background(&background)?;
            question_validate_options(&options)?;
            Ok(QuestionMutation::Update {
                number,
                title,
                background,
                options,
            })
        }
        None if is_blank => Err("缺少问题内容：新建问题需要提供问题与选项".to_string()),
        None => {
            if title.is_empty() {
                return Err("新建问题必须提供问题内容".to_string());
            }
            question_validate_title(&title)?;
            question_validate_background(&background)?;
            question_validate_options(&options)?;
            Ok(QuestionMutation::Create {
                title,
                background,
                options,
            })
        }
    }
}

/// 工具层文本输出：与 `list` 同形，队长一次调用即可看到最新状态。
fn question_list_text(views: &[ConversationQuestionView]) -> String {
    if views.is_empty() {
        return "当前会话没有问题。".to_string();
    }
    let mut lines = vec![format!("当前会话问题（共 {} 条）：", views.len())];
    for view in views {
        lines.push(String::new());
        lines.push(format!("#{} {}", view.number, view.title));
        if !view.background.trim().is_empty() {
            lines.push(format!("背景：{}", view.background.trim()));
        }
        for (index, option) in view.options.iter().enumerate() {
            lines.push(format!("{}. {}", index + 1, option));
        }
    }
    lines.join("\n")
}
