// 单次流式请求的原始块留档。
// 只有流式异常——流已经建立、正在收，然后中途断了或数据解析不出来——才写盘。
// 正常结束、429、对端返回标准错误、空回、用户打断，都算请求成功，不写。
// 每次流式请求各用一份 sink，互不影响；谁报错谁写，成功就丢。

static STREAM_FAILURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct StreamFailureGuard {
    sink: Arc<genai::chat::CollectorSink>,
    data_path: Option<PathBuf>,
    conversation_id: Option<String>,
}

impl StreamFailureGuard {
    fn new(data_path: Option<PathBuf>, conversation_id: Option<String>) -> Self {
        Self {
            sink: Arc::new(genai::chat::CollectorSink::new()),
            data_path,
            conversation_id,
        }
    }

    /// 把这次请求的 sink 挂到它的 ChatOptions 上。
    fn attach(&self, options: genai::chat::ChatOptions) -> genai::chat::ChatOptions {
        options.with_raw_frame_sink_arc(self.sink.clone())
    }

    /// 流式收集报错时调用。不是流式异常、没留下块、或不知道写哪儿，都直接跳过。
    fn note_stream_error(&self, err: &genai::Error) {
        if !is_stream_anomaly(err) {
            return;
        }
        let frames = self.sink.take();
        if frames.is_empty() {
            return;
        }
        let Some(data_path) = self.data_path.as_ref() else {
            return;
        };
        let conversation_id = self.conversation_id.as_deref().unwrap_or("unknown");
        match write_stream_failure_record(data_path, conversation_id, &err.to_string(), &frames) {
            Ok(path) => runtime_log_warn(format!(
                "[聊天] 流式异常已记录: conversation_id={}，path={}",
                conversation_id,
                path.display()
            )),
            Err(error) => runtime_log_warn(format!(
                "[聊天] 流式异常记录保存失败: conversation_id={}，err={}",
                conversation_id, error
            )),
        }
    }
}

/// 流式异常只有两种：传输中断（reset / unexpected eof / 中途超时）和块解析失败。
/// 对端在流里返回 error、HTTP 状态错误、429 这类，都算请求拿到了答复，不算异常。
fn is_stream_anomaly(err: &genai::Error) -> bool {
    matches!(
        err,
        genai::Error::StreamParse { .. } | genai::Error::WebStream { .. }
    )
}

fn write_stream_failure_record(
    data_path: &std::path::Path,
    conversation_id: &str,
    error: &str,
    frames: &[genai::chat::RawFrame],
) -> Result<PathBuf, String> {
    let dir = app_root_from_data_path(&data_path.to_path_buf())
        .join("stream-failures")
        .join(sanitize_stream_failure_id(conversation_id));
    std::fs::create_dir_all(&dir).map_err(|err| format!("创建流式失败目录失败: {err}"))?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let seq = STREAM_FAILURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("{millis}-{seq}.json"));
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "conversation_id": conversation_id,
        "error": error,
        "frames": frames,
    }))
    .map_err(|err| format!("序列化流式失败记录失败: {err}"))?;
    std::fs::write(&path, body).map_err(|err| format!("写入流式失败记录失败: {err}"))?;
    Ok(path)
}

fn sanitize_stream_failure_id(conversation_id: &str) -> String {
    let sanitized: String = conversation_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod stream_failure_capture_tests {
    use super::*;

    fn sample_frame(data: &str) -> genai::chat::RawFrame {
        genai::chat::RawFrame {
            index: 0,
            event: Some("message".to_string()),
            data: genai::chat::RawFrameData::Text(data.to_string()),
            elapsed_us: 1,
        }
    }

    fn sample_model_iden() -> genai::ModelIden {
        genai::ModelIden::new(genai::adapter::AdapterKind::OpenAI, "gpt-test")
    }

    #[test]
    fn only_transport_and_parse_failures_count_as_stream_anomaly() {
        let parse = genai::Error::StreamParse {
            model_iden: sample_model_iden(),
            serde_error: serde_json::from_str::<i32>("not-json").expect_err("invalid json"),
        };
        assert!(is_stream_anomaly(&parse));

        let provider = genai::Error::ChatResponse {
            model_iden: sample_model_iden(),
            body: serde_json::json!({ "error": "rate limited" }),
        };
        assert!(!is_stream_anomaly(&provider));
        assert!(!is_stream_anomaly(&genai::Error::Internal("boom".to_string())));
    }

    #[test]
    fn record_is_written_with_error_and_frames() {
        let root = std::env::temp_dir().join(format!(
            "pai-stream-failure-{}",
            std::process::id()
        ));
        let data_path = root.join("config");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&data_path).expect("temp data dir");

        let written = write_stream_failure_record(
            &data_path,
            "conv/1",
            "Web stream error",
            &[sample_frame("{\"delta\":\"hi\"}")],
        )
        .expect("write");
        let body = std::fs::read_to_string(&written).expect("read");
        assert!(body.contains("Web stream error"));
        assert!(body.contains("delta"));
        assert!(written.starts_with(
            app_root_from_data_path(&data_path)
                .join("stream-failures")
                .join("conv_1")
        ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
