// 通用「远端清单拉取 + 本地缓存」设施。
// 供能力商店与模型元数据（models.dev）共用，替代原先写死的单例实现。

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteCatalogCacheFile {
    updated_at: String,
    fetched_at_ms: i64,
    payload: Value,
}

fn remote_catalog_cache_dir(state: &AppState) -> std::path::PathBuf {
    state
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn remote_catalog_cache_path(state: &AppState, file_name: &str) -> std::path::PathBuf {
    remote_catalog_cache_dir(state).join(file_name)
}

fn read_remote_catalog_cache(
    state: &AppState,
    file_name: &str,
) -> Result<Option<RemoteCatalogCacheFile>, String> {
    let path = remote_catalog_cache_path(state, file_name);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(&path)
        .map_err(|err| format!("读取远端清单缓存失败（{}）：{err}", path.display()))?;
    let cache = serde_json::from_slice::<RemoteCatalogCacheFile>(&raw)
        .map_err(|err| format!("解析远端清单缓存失败（{}）：{err}", path.display()))?;
    Ok(Some(cache))
}

fn write_remote_catalog_cache(
    state: &AppState,
    file_name: &str,
    payload: &Value,
) -> Result<RemoteCatalogCacheFile, String> {
    let path = remote_catalog_cache_path(state, file_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "创建远端清单缓存目录失败（{}）：{err}",
                parent.display()
            )
        })?;
    }
    let cache = RemoteCatalogCacheFile {
        updated_at: now_iso(),
        fetched_at_ms: chrono::Utc::now().timestamp_millis(),
        payload: payload.clone(),
    };
    let raw = serde_json::to_vec_pretty(&cache)
        .map_err(|err| format!("序列化远端清单缓存失败：{err}"))?;
    std::fs::write(&path, raw)
        .map_err(|err| format!("写入远端清单缓存失败（{}）：{err}", path.display()))?;
    Ok(cache)
}

fn remote_catalog_cache_is_stale(cache: &RemoteCatalogCacheFile, ttl_ms: i64) -> bool {
    let age_ms = chrono::Utc::now().timestamp_millis() - cache.fetched_at_ms;
    age_ms > ttl_ms
}

/// 按 TTL 取用缓存：未过期直接返回；过期则重新拉取，拉取失败回退旧缓存。
/// 无缓存时首次拉取必须成功。
async fn ensure_remote_catalog_cache<F, Fut>(
    state: &AppState,
    file_name: &str,
    ttl_ms: i64,
    log_tag: &str,
    fetch: F,
) -> Result<RemoteCatalogCacheFile, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    let cached = read_remote_catalog_cache(state, file_name)?;
    match cached {
        Some(cache) if !remote_catalog_cache_is_stale(&cache, ttl_ms) => Ok(cache),
        Some(cache) => match fetch().await {
            Ok(payload) => write_remote_catalog_cache(state, file_name, &payload),
            Err(err) => {
                runtime_log_warn(format!(
                    "[{log_tag}] 刷新失败，回退旧缓存：error={err:?}, updated_at={}, fetched_at_ms={}",
                    cache.updated_at, cache.fetched_at_ms
                ));
                Ok(cache)
            }
        },
        None => {
            let payload = fetch().await?;
            write_remote_catalog_cache(state, file_name, &payload)
        }
    }
}
