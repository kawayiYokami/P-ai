const DELEGATE_DB_FILE_NAME: &str = "delegate_store.db";
const DELEGATE_STATUS_RUNNING: &str = "running";
const DELEGATE_STATUS_DELIVERED: &str = "delivered";
const DELEGATE_STATUS_COMPLETED: &str = "completed";
const DELEGATE_STATUS_FAILED: &str = "failed";

const DELEGATE_WORK_TYPE_EXPLORE: &str = "explore";
const DELEGATE_WORK_TYPE_REVIEW: &str = "review";
const DELEGATE_WORK_TYPE_WORK: &str = "work";

/// 委托是哪一类活。
///
/// 唯一作用是决定给子智能体注入哪段内置提示词：类型不参与任何权限判断，
/// 三类权限完全一样、一律继承上游；也不存在按类型增减的能力。
/// 类型不落库，只在创建子会话这一次决定注入内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegateWorkType {
    Explore,
    Review,
    Work,
}

impl DelegateWorkType {
    /// 模型入口必填：漏填（空串）与无法识别的取值都拒。
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            DELEGATE_WORK_TYPE_EXPLORE => Ok(Self::Explore),
            DELEGATE_WORK_TYPE_REVIEW => Ok(Self::Review),
            DELEGATE_WORK_TYPE_WORK => Ok(Self::Work),
            "" => Err("delegate.type is required".to_string()),
            other => Err(format!(
                "delegate.type 必须是 `explore` / `review` / `work`，当前收到：{other}"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Explore => DELEGATE_WORK_TYPE_EXPLORE,
            Self::Review => DELEGATE_WORK_TYPE_REVIEW,
            Self::Work => DELEGATE_WORK_TYPE_WORK,
        }
    }

    fn type_prompt(self) -> &'static str {
        match self {
            Self::Explore => "你是文件搜索专家，擅长探索代码库。快速找文件、用正则搜代码、读文件并分析内容。\n按调用方要求的详略程度工作，结论里给绝对路径。不要创建文件，不要改动系统状态。\n高效完成搜索，清楚报告发现。",
            Self::Review => "你是代码审查员，对象是一次改动。优先找 bug、风险、行为回归、缺测试。\n问题按严重度排序，每条给文件:行号，行范围尽量短；先列问题，再写简短摘要。\n没查出问题就明说，并指出残留风险或测试缺口。不改任何文件。",
            Self::Work => "你按既定流程处理这一块。目标和流程固定，每一项按实际情况处理。\n遇到处理不了的那一项，不猜、不改、不发明规则：跳过并记账，把剩下的做完，不要中途停下。\n交回「改了多少 / 跳过多少 / 逐条跳过原因」。",
        }
    }
}

/// 三类共用的底座：说清它是被分出来的子智能体、没有别的上下文、不要发散。
fn delegate_work_type_common_prompt() -> &'static str {
    "你是被分出来跑这一件事的子智能体。背景写在下面这段任务里，你没有别的上下文，聊天记录不会保留，\n跑完这个会话就销毁，所以结论要自带上下文、一次说全，说不全等于没说。不要发问——你问了也没人回。\n不要顺手优化，不要扩大范围。做不到就直接说做不到。"
}

/// 子会话系统提示里追加的那一段：通用底座 + 当前类型的提示词。
fn delegate_system_prompt_block(work_type: DelegateWorkType) -> String {
    format!(
        "{}\n\n{}",
        delegate_work_type_common_prompt(),
        work_type.type_prompt()
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelegateEntry {
    delegate_id: String,
    kind: String,
    conversation_id: String,
    #[serde(default)]
    parent_delegate_id: Option<String>,
    source_agent_id: String,
    target_agent_id: String,
    title: String,
    #[serde(default)]
    why: String,
    #[serde(default)]
    goal: String,
    #[serde(default)]
    todo: String,
    /// 委托正文：模型写好的 prompt 原样落这一列。
    /// 旧行（本列还不存在时建的）由 store 层用 `goal` 回填，上层只认本字段。
    #[serde(default)]
    prompt: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
    status: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    delivered_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

#[derive(Debug, Clone)]
struct DelegateCreateInput {
    kind: String,
    conversation_id: String,
    parent_delegate_id: Option<String>,
    source_agent_id: String,
    target_agent_id: String,
    title: String,
    why: String,
    goal: String,
    todo: String,
    /// 模型链写这里；用户链（写 why / goal / todo 的那六处）留空，靠读层用 `goal` 回填。
    prompt: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
}

#[cfg(test)]
mod delegate_work_type_tests {
    use super::*;

    /// 类型只决定注入哪段提示词：底座三类共用，类型段三类互不相同。
    #[test]
    fn system_prompt_block_should_share_common_base_and_differ_by_type() {
        let common = delegate_work_type_common_prompt();
        let explore = delegate_system_prompt_block(DelegateWorkType::Explore);
        let review = delegate_system_prompt_block(DelegateWorkType::Review);
        let work = delegate_system_prompt_block(DelegateWorkType::Work);

        for block in [&explore, &review, &work] {
            assert!(block.starts_with(common), "底座必须三类共用：{block}");
        }
        assert_ne!(explore, review);
        assert_ne!(review, work);
        assert_ne!(explore, work);
        assert!(explore.contains("文件搜索"));
        assert!(review.contains("代码审查"));
    }

    #[test]
    fn work_type_parse_should_round_trip_and_reject_unknown() {
        for work_type in [
            DelegateWorkType::Explore,
            DelegateWorkType::Review,
            DelegateWorkType::Work,
        ] {
            assert_eq!(
                DelegateWorkType::parse(work_type.as_str()).expect("parse back"),
                work_type
            );
        }
        assert_eq!(
            DelegateWorkType::parse("  ").unwrap_err(),
            "delegate.type is required"
        );
        assert!(DelegateWorkType::parse("search").is_err());
    }
}

