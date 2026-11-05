// 图标取图：把条目里的图标地址换成 data URL 交给前端。
//
// 为什么不在前端直连：VS Code 侧边栏 webview 的 CSP 是 `img-src ${webview.cspSource} data:`，
// cspSource 只覆盖扩展自己的 dist 目录，远程 https 与 asset:// 都会被挡掉，只有 data: 能过。
// 桌面窗口 csp 为 null 本来可以直连，但两端行为必须一致，所以统一走后端取图。
//
// 这不是通用取图接口：只有当前目录结果里出现过的地址才允许取，其余一律拒绝。

const CATALOG_ICON_MAX_BYTES: usize = 1024 * 1024;
const CATALOG_ICON_CACHE_DIR: &str = "catalog-icons";
const CATALOG_ICON_TIMEOUT_SECS: u64 = 8;
/// 重定向最多跟 3 跳。GitHub 头像要经一次 302 到 avatars.githubusercontent.com。
const CATALOG_ICON_MAX_REDIRECTS: usize = 3;
/// 白名单上限：翻页、切源会持续登记新地址，只增不减会一直涨。
/// 取 512 ≈ 三十条一页翻十几轮的覆盖量；超出按登记顺序淘汰最旧的。
/// 不能每轮清空重来——那样翻回上一页时地址已被移出白名单，图标会重新被拒。
const CATALOG_ICON_ALLOWED_LIMIT: usize = 512;

/// 白名单本体。
/// 注意它是**进程级共享**的：多个窗口共用这一份，A 窗口浏览过的地址 B 窗口也能取。
/// 影响很小（取到的仍只是目录里出现过的公网图片），但不要以为它是按会话或按窗口隔离的。
#[derive(Default)]
struct CatalogIconAllowlist {
    order: std::collections::VecDeque<String>,
    set: std::collections::HashSet<String>,
}

impl CatalogIconAllowlist {
    fn remember(&mut self, address: &str) {
        if !self.set.insert(address.to_string()) {
            return;
        }
        self.order.push_back(address.to_string());
        while self.order.len() > CATALOG_ICON_ALLOWED_LIMIT {
            if let Some(evicted) = self.order.pop_front() {
                self.set.remove(&evicted);
            }
        }
    }

    fn contains(&self, address: &str) -> bool {
        self.set.contains(address)
    }
}

static CATALOG_ICON_ALLOWED: std::sync::OnceLock<
    std::sync::Mutex<CatalogIconAllowlist>,
> = std::sync::OnceLock::new();

fn catalog_icon_allowed_set() -> &'static std::sync::Mutex<CatalogIconAllowlist> {
    CATALOG_ICON_ALLOWED.get_or_init(|| std::sync::Mutex::new(CatalogIconAllowlist::default()))
}

/// 把本轮目录结果里的图标地址登记进白名单。
/// 只有登记过的地址才允许取图，避免这一层变成任意出站请求的入口。
fn catalog_remember_icon_addresses(entries: &[CatalogEntry]) {
    let Ok(mut allowed) = catalog_icon_allowed_set().lock() else {
        runtime_log_warn("[能力商店] 图标白名单加锁失败，本轮跳过登记".to_string());
        return;
    };
    for entry in entries {
        let icon = entry.icon.trim();
        if !icon.is_empty() {
            allowed.remember(icon);
        }
    }
}

/// 地址是否可取的完整判定：先看白名单，再看地址本身是否是一个公网 https 地址。
fn catalog_icon_address_is_allowed(address: &str) -> bool {
    let address = address.trim();
    if address.is_empty() {
        return false;
    }
    let in_allowlist = match catalog_icon_allowed_set().lock() {
        Ok(allowed) => allowed.contains(address),
        Err(_) => return false,
    };
    in_allowlist && catalog_icon_url_is_public_https(address)
}

/// 只允许 https、不带账号密码、443 端口、非本机非内网。
fn catalog_icon_url_is_public_https(address: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(address) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    if parsed.port_or_known_default() != Some(443) {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return false;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()),
            std::net::IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified()),
        };
    }
    true
}

/// 按魔数判断是不是图片，返回对应 MIME；不是图片返回 None。
/// 只认这几种：PNG / JPEG / GIF / WebP / ICO / SVG。
///
/// 为什么必须是魔数、不能信 `Content-Type`：魔搭图标的响应头统一给
/// `application/octet-stream`，即使字节头是标准 PNG 签名（实测五张封面全如此）。
/// 一旦改成信 `Content-Type`，魔搭两个源（MCP 的 `FromSiteIcon`、技能的 `SourceAvatar`）
/// 的图标会整片退回首字母方块。
fn catalog_icon_mime_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    // SVG 是文本：只看头部，并且带脚本的一律不收。
    let head = &bytes[..bytes.len().min(512)];
    if let Ok(text) = std::str::from_utf8(head) {
        let lower = text.to_ascii_lowercase();
        if lower.contains("<svg") {
            if lower.contains("<script") {
                return None;
            }
            return Some("image/svg+xml");
        }
    }
    None
}

/// 图标缓存路径：`{地址哈希}.bin` 存字节，`{地址哈希}.type` 存 MIME。
/// 目录跟运行目录一致放在 `app_root/temp/` 下，不写进配置文件所在的那层。
fn catalog_icon_cache_paths(
    state: &AppState,
    address: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(address.as_bytes());
    let key = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let dir = app_root_from_data_path(&state.data_path)
        .join("temp")
        .join(CATALOG_ICON_CACHE_DIR);
    (dir.join(format!("{key}.bin")), dir.join(format!("{key}.type")))
}

fn catalog_icon_encode_data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", B64.encode(bytes))
}

/// 读磁盘缓存；两个文件齐了才算命中。
fn catalog_icon_cached_data_url(state: &AppState, address: &str) -> Option<String> {
    let (bin_path, type_path) = catalog_icon_cache_paths(state, address);
    let mime = std::fs::read_to_string(&type_path).ok()?;
    let bytes = std::fs::read(&bin_path).ok()?;
    let mime = mime.trim();
    if mime.is_empty() || bytes.is_empty() || bytes.len() > CATALOG_ICON_MAX_BYTES {
        return None;
    }
    Some(catalog_icon_encode_data_url(mime, &bytes))
}

fn catalog_write_icon_cache(state: &AppState, address: &str, mime: &str, bytes: &[u8]) {
    let (bin_path, type_path) = catalog_icon_cache_paths(state, address);
    let Some(dir) = bin_path.parent() else {
        return;
    };
    if let Err(err) = std::fs::create_dir_all(dir) {
        runtime_log_warn(format!("[能力商店] 创建图标缓存目录失败：{err}"));
        return;
    }
    if let Err(err) = std::fs::write(&bin_path, bytes) {
        runtime_log_warn(format!("[能力商店] 写入图标缓存失败：{err}"));
        return;
    }
    if let Err(err) = std::fs::write(&type_path, mime) {
        runtime_log_warn(format!("[能力商店] 写入图标类型失败：{err}"));
    }
}

/// 图标专用 HTTP 客户端：跟随重定向，但每一跳都要过公网 https 校验。
/// 白名单只挡得住初始地址，一串 302 就能把请求带到别处，所以不能沿用共享客户端的默认策略。
static CATALOG_ICON_CLIENT: std::sync::OnceLock<Option<reqwest::Client>> =
    std::sync::OnceLock::new();

fn catalog_icon_http_client() -> Option<&'static reqwest::Client> {
    CATALOG_ICON_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(app_http_user_agent())
                .default_headers(app_identity_headers())
                .timeout(std::time::Duration::from_secs(CATALOG_ICON_TIMEOUT_SECS))
                .connect_timeout(std::time::Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    if attempt.previous().len() >= CATALOG_ICON_MAX_REDIRECTS {
                        return attempt.error(std::io::Error::other("图标重定向跳数超限"));
                    }
                    if !catalog_icon_url_is_public_https(attempt.url().as_str()) {
                        return attempt.error(std::io::Error::other("图标重定向目标不是公网 https"));
                    }
                    attempt.follow()
                }))
                .build()
                .ok()
        })
        .as_ref()
}

/// 按地址取一张图标并转成 data URL。
/// 任何一步不成立都返回空串：前端据此退首字母方块，不需要区分错误类型。
async fn catalog_icon_data_url_from_remote(state: &AppState, address: &str) -> String {
    let Some(client) = catalog_icon_http_client() else {
        runtime_log_warn("[能力商店] 图标 HTTP 客户端构建失败，本轮一律退首字母".to_string());
        return String::new();
    };
    let resp = client
        .get(address)
        .header("Accept", "image/*")
        .send()
        .await;
    let Ok(resp) = resp else {
        runtime_log_debug(format!("[能力商店] 图标请求失败，退首字母：{address}"));
        return String::new();
    };
    if !resp.status().is_success() {
        runtime_log_debug(format!(
            "[能力商店] 图标请求返回 {}，退首字母：{address}",
            resp.status()
        ));
        return String::new();
    }
    if let Some(length) = resp.content_length() {
        if length as usize > CATALOG_ICON_MAX_BYTES {
            runtime_log_debug(format!("[能力商店] 图标超过 1MB，丢弃：{address}"));
            return String::new();
        }
    }
    let Ok(bytes) = resp.bytes().await else {
        return String::new();
    };
    if bytes.is_empty() || bytes.len() > CATALOG_ICON_MAX_BYTES {
        runtime_log_debug(format!("[能力商店] 图标字节数不合格，丢弃：{address}"));
        return String::new();
    }
    let Some(mime) = catalog_icon_mime_from_bytes(&bytes) else {
        runtime_log_debug(format!("[能力商店] 取到的不是图片，丢弃：{address}"));
        return String::new();
    };
    catalog_write_icon_cache(state, address, mime, &bytes);
    catalog_icon_encode_data_url(mime, &bytes)
}

async fn catalog_icon_data_url_inner(state: &AppState, address: &str) -> Result<String, String> {
    let address = address.trim();
    if !catalog_icon_address_is_allowed(address) {
        return Ok(String::new());
    }
    if let Some(cached) = catalog_icon_cached_data_url(state, address) {
        return Ok(cached);
    }
    Ok(catalog_icon_data_url_from_remote(state, address).await)
}
