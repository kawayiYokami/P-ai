use std::path::Path;

use litchi::sheet::Workbook;
use litchi::{detect_file_format, Document, FileFormat, Presentation};

const PREVIEW_CHARS: usize = 1200;

fn build_preview(text: &str, max_chars: usize) -> String {
    let normalized = text.replace('\r', "");
    let total = normalized.chars().count();
    if total <= max_chars {
        return normalized;
    }
    let head = normalized.chars().take(max_chars).collect::<String>();
    format!("{head}\n\n...[truncated, total_chars={total}]")
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args().skip(1);
    let file_path = args.next().ok_or_else(|| {
        "usage: cargo run --bin litchi_probe -- <absolute-path-to-office-file>".to_string()
    })?;

    if args.next().is_some() {
        return Err("only one file path argument is supported".into());
    }

    let path = Path::new(&file_path);
    if !path.is_absolute() {
        return Err("absolute path is required".into());
    }
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()).into());
    }

    let format = detect_file_format(path)
        .ok_or_else(|| format!("failed to detect file format for {}", path.display()))?;
    println!("detected_format={format:?}");

    let text = match format {
        FileFormat::Doc
        | FileFormat::Docx
        | FileFormat::Rtf
        | FileFormat::Odt
        | FileFormat::Pages => {
            let document = Document::open(path)?;
            document.text()?
        }
        FileFormat::Ppt | FileFormat::Pptx | FileFormat::Odp | FileFormat::Keynote => {
            let presentation = Presentation::open(path)?;
            presentation.text()?
        }
        FileFormat::Xls
        | FileFormat::Xlsx
        | FileFormat::Xlsb
        | FileFormat::Ods
        | FileFormat::Numbers => {
            let workbook = Workbook::open(path)?;
            workbook.text()?
        }
    };

    println!("total_chars={}", text.chars().count());
    println!("----- preview -----");
    println!("{}", build_preview(&text, PREVIEW_CHARS));

    Ok(())
}
