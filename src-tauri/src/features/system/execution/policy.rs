/// Windows verbatim 扩展前缀归一化：`\\?\` 与 `\\?\UNC\` 是 canonicalize 的产物，
/// 与普通盘符/UNC 路径在字符串层不可比。在 UTF-16 层面处理，避免 lossy 转换。
#[cfg(target_os = "windows")]
fn normalize_extended_prefix_wide(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;

    let wide = path.as_os_str().encode_wide().collect::<Vec<u16>>();
    const VERBATIM: [u16; 4] = [0x5C, 0x5C, 0x3F, 0x5C]; // \\?\
    const UNC_SEG: [u16; 4] = [0x55, 0x4E, 0x43, 0x5C]; // UNC\
    if wide.starts_with(&VERBATIM) {
        let rest = &wide[4..];
        if rest.starts_with(&UNC_SEG) {
            let mut out = vec![0x5C, 0x5C]; // \\
            out.extend_from_slice(&rest[4..]);
            return out;
        }
        return rest.to_vec();
    }
    wide
}

/// 平台标识符级比较：Windows 用 ordinal ignore-case（正确处理全部 Unicode 大小写，
/// 不做 linguistic 折叠），Unix 用原生 OsStr 相等（大小写敏感）。
fn path_part_eq(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt as _;
        use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

        const IGNORE_CASE_TRUE: i32 = 0x1;
        let a_wide = a.encode_wide().collect::<Vec<u16>>();
        let b_wide = b.encode_wide().collect::<Vec<u16>>();
        let result = unsafe {
            CompareStringOrdinal(
                a_wide.as_ptr(),
                a_wide.len() as i32,
                b_wide.as_ptr(),
                b_wide.len() as i32,
                IGNORE_CASE_TRUE,
            )
        };
        result == CSTR_EQUAL
    }
    #[cfg(not(target_os = "windows"))]
    {
        a == b
    }
}

/// 按 Path components 逐级比较：base 是 target 的祖先（含相等）时为真。
/// 不再用字符串拼接分隔符判断前缀，避免 `C:\foo` vs `C:\foobar` 这类层级误判。
fn exec_path_is_within(base: &std::path::Path, target: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStringExt as _;

        let base_path = std::path::PathBuf::from(std::ffi::OsString::from_wide(
            &normalize_extended_prefix_wide(base),
        ));
        let target_path = std::path::PathBuf::from(std::ffi::OsString::from_wide(
            &normalize_extended_prefix_wide(target),
        ));
        let base_parts = base_path
            .components()
            .map(|comp| comp.as_os_str())
            .collect::<Vec<_>>();
        let target_parts = target_path
            .components()
            .map(|comp| comp.as_os_str())
            .collect::<Vec<_>>();
        if target_parts.len() < base_parts.len() {
            return false;
        }
        base_parts
            .iter()
            .zip(target_parts.iter())
            .all(|(b, t)| path_part_eq(b, t))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let base_parts = base.components().collect::<Vec<_>>();
        let target_parts = target.components().collect::<Vec<_>>();
        if target_parts.len() < base_parts.len() {
            return false;
        }
        base_parts
            .iter()
            .zip(target_parts.iter())
            .all(|(b, t)| b.as_os_str() == t.as_os_str())
    }
}

fn sanitize_normalized_path(path: &std::path::Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()))
            .join(path)
    };

    let mut normalized = PathBuf::new();
    let mut normal_depth = 0usize;
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
            }
            std::path::Component::RootDir => {
                normalized.push(component.as_os_str());
                normal_depth = 0;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if normal_depth > 0 {
                    normalized.pop();
                    normal_depth = normal_depth.saturating_sub(1);
                }
            }
            std::path::Component::Normal(seg) => {
                normalized.push(seg);
                normal_depth = normal_depth.saturating_add(1);
            }
        }
    }
    normalized
}

fn normalize_target_for_access_check(path: &std::path::Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let sanitized = sanitize_normalized_path(path);
    if let Some(parent) = sanitized.parent() {
        if let Ok(parent_canonical) = parent.canonicalize() {
            if let Some(name) = sanitized.file_name() {
                return parent_canonical.join(name);
            }
            return parent_canonical;
        }
    }
    sanitized
}

fn path_allowed(
    state: &AppState,
    session_id: &str,
    target: &std::path::Path,
) -> Result<bool, String> {
    // 与终端 cwd / write / patch 同一白名单：命中本会话权限范围即放行
    Ok(terminal_match_workspace_for_session_target(state, session_id, target)?.is_some())
}

/// 校验 cwd 位于本会话允许的执行根内（主工作区或已绑定的工作树）。
/// canonicalize 是同步阻塞 I/O，统一放到 spawn_blocking 中执行，
/// 避免在 async 后端路径上阻塞 runtime 工作线程。
async fn assert_cwd_allowed(
    state: AppState,
    session_id: String,
    cwd: std::path::PathBuf,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if path_allowed(&state, &session_id, &cwd)? {
            return Ok(());
        }
        Err(format!(
            "Working directory is outside current shell root: {}. 本会话仅允许在主工作区或已绑定的工作树内执行命令；请改用这些目录，或在会话工作区配置中调整工作树绑定。",
            cwd.to_string_lossy()
        ))
    })
    .await
    .map_err(|err| format!("cwd 校验任务执行失败：{err}"))?
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    struct TempDirGuard(std::path::PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exec_path_is_within_should_check_path_containment() {
        assert!(exec_path_is_within(
            std::path::Path::new("/a/b"),
            std::path::Path::new("/a/b/c")
        ));
        assert!(exec_path_is_within(
            std::path::Path::new("/a/b"),
            std::path::Path::new("/a/b")
        ));
        assert!(!exec_path_is_within(
            std::path::Path::new("/a/b"),
            std::path::Path::new("/a/c")
        ));
        // 前缀相似但不是子路径：/a/bad 不能误判为 /a/b 的子路径
        assert!(!exec_path_is_within(
            std::path::Path::new("/a/b"),
            std::path::Path::new("/a/bad")
        ));
        #[cfg(target_os = "windows")]
        {
            // 跨盘
            assert!(!exec_path_is_within(
                std::path::Path::new(r"C:\a"),
                std::path::Path::new(r"D:\b")
            ));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn exec_path_is_within_should_treat_verbatim_prefix_as_equivalent() {
        // canonicalize 产物 \\?\C:\... 与普通盘符路径必须等价
        let base_verbatim = std::path::Path::new(r"\\?\C:\a");
        let target_plain = std::path::Path::new(r"C:\a\b\c");
        assert!(exec_path_is_within(base_verbatim, target_plain));
        assert!(exec_path_is_within(
            std::path::Path::new(r"C:\a"),
            std::path::Path::new(r"\\?\C:\a\b")
        ));
        // UNC：\\?\UNC\server\share 与 \\server\share 等价；只匹配同服务器，不串盘
        assert!(exec_path_is_within(
            std::path::Path::new(r"\\?\UNC\server\share"),
            std::path::Path::new(r"\\server\share\dir\file")
        ));
        assert!(!exec_path_is_within(
            std::path::Path::new(r"\\?\UNC\server\share"),
            std::path::Path::new(r"\\other\share\dir")
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn exec_path_is_within_should_compare_case_insensitively() {
        assert!(exec_path_is_within(
            std::path::Path::new(r"C:\A\B"),
            std::path::Path::new(r"c:\a\b\file.txt")
        ));
        // 目录名大小写不同但仍为祖先
        assert!(exec_path_is_within(
            std::path::Path::new(r"C:\Repo\Src"),
            std::path::Path::new(r"C:\repo\SRC\main.rs")
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalize_target_should_resolve_symlink_into_canonical() {
        // Windows 创建符号链接需要开发者模式/管理员权限，无权限时跳过
        let tmp = std::env::temp_dir().join(format!(
            "pai-policy-symlink-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t")
        ));
        let _guard = TempDirGuard(tmp.clone());
        let real = tmp.join("real");
        if std::fs::create_dir_all(&real).is_err() {
            return;
        }
        let link = tmp.join("link");
        if std::os::windows::fs::symlink_dir(&real, &link).is_err() {
            return;
        }
        // symlink 目标统一解析到 real 的 canonical 路径：比较结果等价
        let normalized = normalize_target_for_access_check(&link.join("file.txt"));
        assert!(exec_path_is_within(&tmp, &normalized));
        assert!(!exec_path_is_within(&real, &link));
        let normalized_link = normalize_target_for_access_check(&link);
        assert!(exec_path_is_within(&tmp, &normalized_link));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn normalize_target_should_resolve_symlink_into_canonical() {
        let tmp = std::env::temp_dir().join(format!(
            "pai-policy-symlink-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t")
        ));
        let _guard = TempDirGuard(tmp.clone());
        let real = tmp.join("real");
        if std::fs::create_dir_all(&real).is_err() {
            return;
        }
        let link = tmp.join("link");
        if std::os::unix::fs::symlink(&real, &link).is_err() {
            return;
        }
        let normalized = normalize_target_for_access_check(&link.join("file.txt"));
        assert!(exec_path_is_within(&tmp, &normalized));
        let normalized_link = normalize_target_for_access_check(&link);
        assert!(exec_path_is_within(&tmp, &normalized_link));
    }
}