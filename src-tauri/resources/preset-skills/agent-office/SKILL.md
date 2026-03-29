---
name: agent-office
description: 当需要代办 Word、Excel、PPT、PDF 的新建、修改、重写、转换、批处理、导出与交付时，必须立刻阅读我。
---

# Agent Office

## 任务定位
- 目标是替用户完成 Office 工作。
- 先区分任务属于哪一类：新建、修改、重写、批处理、转换、导出、校对。
- 能直接交付成品时，不要只停留在摘要或提取结果。

## 项目来源（必须先看）
- 主项目（Gitee）：`https://gitee.com/CoderWanFeng/python-office/`
- 主项目（GitHub）：`https://github.com/CoderWanFeng/python-office`
- 项目文档索引（RepoWiki）：`https://github.com/CoderWanFeng/python-office/tree/develop/.qoder/repowiki/zh/content`

## 事实边界
- 先看项目文档，再决定调用方式，不要臆测 API。
- 若仓库文档与本 Skill 冲突，以仓库文档为准，并向用户说明差异。
- 目前有明确证据的能力，主要集中在：转换、合并、拆分、查询、水印、加解密、导出。
- 目前没有充分证据表明 `python-office` 的公开 API 已提供通用的 Word/PPT 段落级创建与编辑能力。
- 因此，涉及精细新建或修改 `docx/xlsx/pptx` 时，优先直接使用底层库，而不是强行套 `python-office`。

## 工具选型
### 1. 只读场景
- 文档问答、摘要、检索前处理：优先 `read_file`
- PDF 文本分段提取：可配合 `pymupdf`（`fitz`）

### 2. 一行式批处理 / 转换 / 导出
- 优先 `python-office`
- 适合：批量合并、批量拆分、格式转换、导出 PDF、PDF 水印、PDF 加解密

### 3. 精细创建 / 修改 Office 文件
- Word：`python-docx`
- Excel：`openpyxl`
- PPT：`python-pptx`
- 这类任务包括：新建文档、改标题层级、改段落、改表格、改单元格、改幻灯片结构

## 工作原则
- 先确认用户要的成品类型，再动手。
- 优先保留原始结构、原始语义和可编辑性。
- 不虚构原文没有的信息。
- 用户没要求时，不擅自重写全部内容或重排结构。
- 修改现有文件时，优先输出新文件，避免覆盖原件。
- 汇报时必须说明：生成了什么、改了什么、产物在哪里、还有什么限制。

## 推荐工作流
1. 判断目标文件类型和交付物。
2. 判断是“读取”还是“真正办事”。
3. 选择最轻的可行工具链。
4. 先做最小样本验证，再处理正式文件。
5. 生成新文件并回报关键变更点。

## 常见任务分流
### 新建
- 从空白生成周报、方案、纪要、清单、统计表、汇报 PPT
- 优先用 `python-docx` / `openpyxl` / `python-pptx`

### 修改
- 改 Word 标题、段落、列表、表格
- 改 Excel 表头、单元格、sheet、汇总区
- 改 PPT 标题、正文、页序、提纲
- 优先用底层编辑库直接改结构

### 批处理
- 合并多个同类文件
- 按列拆分 Excel
- 批量导出 PDF / 图片
- 优先用 `python-office`

### 转换 / 导出
- Word 转 PDF
- Excel 转 PDF
- PPT 转 PDF / 图片
- PDF 转 docx / 图片
- 优先用 `python-office`

## 已确认的 python-office 高频能力
以下能力有明确仓库资料支持，适合优先考虑。

### Excel
- `fake2excel`：生成示例数据 Excel
- `merge2excel`：多个 Excel 合并为一个工作簿
- `sheet2excel`：将一个工作簿的多个 sheet 拆成多个文件
- `merge2sheet`：将多份数据合并到一个 sheet
- `split_excel_by_column`：按列拆分 Excel
- `find_excel_data`：检索 Excel 内容
- `query4excel`：按条件提取结果
- `excel2pdf`：Excel 导出 PDF
- `count4page`：统计打印页数

### Word
- `merge4docx`：合并多个 docx
- `docx2pdf`：docx 转 PDF
- `doc2docx`：doc 转 docx
- `docx2doc`：docx 转 doc
- `docx4imgs`：提取 docx 中的图片

### PPT
- `ppt2pdf`：PPT 转 PDF
- `ppt2img`：PPT 转图片
- `merge4ppt`：合并多个 PPT

### PDF
- `add_watermark`：PDF 加文字或图片水印
- `txt2pdf`：文本转 PDF
- `encrypt4pdf`：PDF 加密
- `decrypt4pdf`：PDF 解密
- `merge2pdf`：合并多个 PDF
- `pdf2docx`：PDF 转 docx
- `pdf2imgs`：PDF 转图片

## 没有足够证据的能力
以下能力不要直接假定 `python-office` 公开 API 已支持：
- 通用 Word 新建整份报告
- 通用 Word 段落级编辑
- 通用 PPT 新建整套演示文稿
- 通用 PPT 幻灯片级精细编辑
- 通用 PDF 段落级编辑

遇到这些需求时，改用底层编辑库处理。

## 分类型办事指南
### Word 办公
适合：周报、方案、纪要、说明文、通知。

优先策略：
- 新建或重写：`python-docx`
- 合并或转格式：`python-office`
- 提图：`python-office`

常见动作：
- 建标题层级
- 写正文和列表
- 插入表格
- 根据材料改写成正式文档
- 输出 docx，必要时再导出 PDF

### Excel 办公
适合：台账、清单、统计表、汇总表。

优先策略：
- 新建和改单元格：`openpyxl`
- 合并、拆分、查询、导出 PDF：`python-office`

常见动作：
- 新建工作簿和 sheet
- 写表头和数据行
- 补统计 sheet
- 按列拆表
- 合并多份表

### PPT 办公
适合：汇报稿、提案、培训材料、路演稿。

优先策略：
- 新建和改页内容：`python-pptx`
- 导出 PDF / 图片、合并 PPT：`python-office`

常见动作：
- 根据提纲生成封面页、要点页、结论页
- 把长文压缩成演示要点
- 重写标题与 bullet
- 导出 PDF 作为汇报附件

### PDF 办公
适合：归档、加水印、加解密、合并、格式转换。

优先策略：
- 文本查看：`read_file` 或 `pymupdf`
- 水印、加解密、合并、转 docx / 图片：`python-office`

注意：
- 扫描版 PDF 默认不做 OCR
- 复杂 PDF 转 docx 可能丢版式
- 用户要“修改 PDF 内容”时，先判断是否应转 docx 后编辑再导出

## 依赖建议（缺了再装）
```bash
uv add python-office python-docx openpyxl python-pptx pymupdf
