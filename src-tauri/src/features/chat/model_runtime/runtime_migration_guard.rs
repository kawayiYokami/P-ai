#[cfg(test)]
mod runtime_migration_guard_tests {
    use std::{fs, path::Path};

    const DISALLOWED_RUNTIME_MARKERS: [&str; 8] = [
        "rig::",
        "_rig_style",
        "OpenAiRigApiKind",
        "invoke_model_rig_by_format",
        "tool_server_handle",
        "stream_completion(",
        "RigMessage",
        "StreamingCompletion",
    ];

    fn collect_rust_files(root: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(root)
            .map_err(|err| format!("read_dir failed for {}: {err}", root.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, files)?;
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
        Ok(())
    }

    #[test]
    fn runtime_source_should_not_reintroduce_rig_runtime_markers() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::<std::path::PathBuf>::new();
        collect_rust_files(&src_root, &mut files).expect("should enumerate rust source files");

        let mut findings = Vec::<String>::new();
        for file in files {
            if file.file_name().and_then(|name| name.to_str())
                == Some("runtime_migration_guard.rs")
            {
                continue;
            }
            let content =
                fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {} failed: {err}", file.display()));
            for (line_index, line) in content.lines().enumerate() {
                for marker in DISALLOWED_RUNTIME_MARKERS {
                    if line.contains(marker) {
                        let relative = file
                            .strip_prefix(&src_root)
                            .unwrap_or(&file)
                            .display()
                            .to_string();
                        findings.push(format!("{relative}:{} -> {}", line_index + 1, marker));
                    }
                }
            }
        }

        assert!(
            findings.is_empty(),
            "runtime migration guard found disallowed markers:\n{}",
            findings.join("\n")
        );
    }
}
