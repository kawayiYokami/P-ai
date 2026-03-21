---
name: agent-office
description: 当需要读取或提取 PDF、Word、Excel、PPT 的文本内容时，必须立刻阅读我。适用于文档问答、摘要、检索前处理等场景；我会告诉你该先看哪些项目资料和入口。
---

# Agent Office

## 项目来源（必须先看）
- 主项目（Gitee）：`https://gitee.com/CoderWanFeng/python-office/`
- 主项目（GitHub）：`https://github.com/CoderWanFeng/python-office`
- 项目文档索引（RepoWiki）：`https://github.com/CoderWanFeng/python-office/tree/develop/.qoder/repowiki/zh/content`

## 使用前要求
- 先阅读上面的项目说明与文档，再决定调用方式，不要凭空假设 API。
- 优先以项目文档中给出的能力边界和示例为准。
- 若文档与本 Skill 冲突，以项目文档为准，并在输出中说明差异。

## 规则
- 只做文档文本提取与结构化整理，不虚构原文中不存在的信息。
- 超长文档优先分段提取，避免一次性塞满上下文。
- 扫描版 PDF（图片 PDF）默认不做 OCR；需要 OCR 时应明确告知限制。
- 输出时标注来源文件名与处理范围（例如页码范围、sheet 名称）。

## 依赖安装（uv）
```bash
# 1) 创建并进入项目目录
mkdir office-processing
cd office-processing

# 2) 初始化
uv init

# 3) 安装依赖
uv add pymupdf python-docx pandas openpyxl python-pptx

# 4) 锁定依赖
uv lock
```

## 使用方法

### PDF 文本提取
```python
import fitz

def extract_pdf_text(pdf_path: str, max_length: int | None = None) -> str:
    doc = fitz.open(pdf_path)
    text = ""
    for page in doc:
        text += page.get_text() + "\n\n"
        if max_length and len(text) >= max_length:
            break
    return text[:max_length] if max_length else text
```

### Word 文本提取
```python
from docx import Document

def extract_docx_text(docx_path: str) -> str:
    doc = Document(docx_path)
    return "\n".join(p.text for p in doc.paragraphs if p.text)
```

### Excel 文本提取
```python
import pandas as pd

def extract_excel_text(xlsx_path: str) -> str:
    sheets = pd.read_excel(xlsx_path, sheet_name=None)
    out = []
    for name, df in sheets.items():
        out.append(f"=== Sheet: {name} ===")
        out.append(df.fillna("").to_string(index=False))
        out.append("")
    return "\n".join(out)
```

### PPT 文本提取
```python
from pptx import Presentation

def extract_ppt_text(pptx_path: str) -> str:
    prs = Presentation(pptx_path)
    out = []
    for idx, slide in enumerate(prs.slides, start=1):
        out.append(f"=== Slide {idx} ===")
        for shape in slide.shapes:
            if hasattr(shape, "text") and shape.text:
                out.append(shape.text)
        out.append("")
    return "\n".join(out)
```

## 维护
```bash
uv sync --upgrade
```

## 输出要求
- 说明处理了哪些文件。
- 说明提取方式与范围（例如页数、sheet）。
- 说明是否有未处理内容（如扫描件 OCR 未启用）。
