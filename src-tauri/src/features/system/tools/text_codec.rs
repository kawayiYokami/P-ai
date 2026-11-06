#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextFileBom {
    None,
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// 文件原本用的行尾写法。
///
/// 解码时统一看成 LF，编码时再还原——匹配只在一套视图上进行，行尾差异不会
/// 走到匹配层。文件里一次都没出现 CR 时按 LF 处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextLineEnding {
    Lf,
    CrLf,
    Cr,
}

impl TextLineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// 上报给模型的行尾标签，沿用 read_file 原有的 crlf/lf/cr 口径。
    fn source_label(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::CrLf => "crlf",
            Self::Cr => "cr",
        }
    }

    /// 把 LF 视图还原成这个行尾。文本里若混入了别的行尾（模型自己带的 `\r\n`），
    /// 先归一化再统一转换，否则会拼出 `\r\r\n`。
    fn restore(self, text: &str) -> std::borrow::Cow<'_, str> {
        if self == Self::Lf {
            return std::borrow::Cow::Borrowed(text);
        }
        let unified = text.replace("\r\n", "\n").replace('\r', "\n");
        std::borrow::Cow::Owned(unified.replace('\n', self.as_str()))
    }
}

#[derive(Debug, Clone)]
struct DecodedTextFile {
    text: String,
    encoding: &'static encoding_rs::Encoding,
    bom: TextFileBom,
    line_ending: TextLineEnding,
}

impl DecodedTextFile {
    fn encode_like_original(&self, text: &str) -> Result<Vec<u8>, String> {
        let restored = self.line_ending.restore(text);
        encode_text_with_detected_encoding(&restored, self.encoding, self.bom)
    }
}

/// 把 CRLF / CR 统一成 LF，并记住出现的第一种行尾。
///
/// 行尾差异是正常输入，不是错误：模型读到的是 LF，磁盘上可能是 CRLF，两边直接
/// 比较永远对不上。在这里一次统一，比等到匹配失败再补救便宜——后者要先扫全文
/// 找候选，代价随文件大小暴涨。
fn normalize_line_endings(text: &str) -> (String, TextLineEnding) {
    if !text.contains('\r') {
        return (text.to_string(), TextLineEnding::Lf);
    }
    let mut ending: Option<TextLineEnding> = None;
    let mut normalized = String::with_capacity(text.len());
    let mut copied = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '\r' {
            continue;
        }
        normalized.push_str(&text[copied..index]);
        let is_crlf = text[index + 1..].starts_with('\n');
        ending.get_or_insert(if is_crlf {
            TextLineEnding::CrLf
        } else {
            TextLineEnding::Cr
        });
        normalized.push('\n');
        copied = index + if is_crlf { 2 } else { 1 };
        if is_crlf {
            chars.next();
        }
    }
    normalized.push_str(&text[copied..]);
    (normalized, ending.unwrap_or(TextLineEnding::Lf))
}

fn decode_text_file_bytes(bytes: &[u8]) -> Result<DecodedTextFile, String> {
    if bytes.is_empty() {
        return Ok(DecodedTextFile {
            text: String::new(),
            encoding: encoding_rs::UTF_8,
            bom: TextFileBom::None,
            line_ending: TextLineEnding::Lf,
        });
    }

    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return decode_with_encoding(&bytes[3..], encoding_rs::UTF_8, TextFileBom::Utf8);
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        return decode_with_encoding(&bytes[2..], encoding_rs::UTF_16LE, TextFileBom::Utf16Le);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return decode_with_encoding(&bytes[2..], encoding_rs::UTF_16BE, TextFileBom::Utf16Be);
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        let (text, line_ending) = normalize_line_endings(text);
        return Ok(DecodedTextFile {
            text,
            encoding: encoding_rs::UTF_8,
            bom: TextFileBom::None,
            line_ending,
        });
    }

    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    let guessed = detector.guess(None, chardetng::Utf8Detection::Allow);
    if let Ok(decoded) = decode_with_encoding(bytes, guessed, TextFileBom::None) {
        return Ok(decoded);
    }

    #[cfg(target_os = "windows")]
    if let Ok(decoded) = decode_with_encoding(bytes, encoding_rs::GBK, TextFileBom::None) {
        return Ok(decoded);
    }

    Err("无法识别文本编码，已拒绝按有损方式读取".to_string())
}

fn decode_text_file_from_path(path: &std::path::Path) -> Result<DecodedTextFile, String> {
    let bytes =
        std::fs::read(path).map_err(|err| format!("读取文件失败（{}）：{err}", terminal_path_for_user(path)))?;
    decode_text_file_bytes(&bytes).map_err(|err| {
        format!(
            "{}：{}",
            err,
            terminal_path_for_user(path)
        )
    })
}

fn decode_with_encoding(
    bytes: &[u8],
    encoding: &'static encoding_rs::Encoding,
    bom: TextFileBom,
) -> Result<DecodedTextFile, String> {
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(format!("{} 解码失败", encoding.name()));
    }
    let (text, line_ending) = normalize_line_endings(&decoded);
    Ok(DecodedTextFile {
        text,
        encoding,
        bom,
        line_ending,
    })
}

fn encode_text_with_detected_encoding(
    text: &str,
    encoding: &'static encoding_rs::Encoding,
    bom: TextFileBom,
) -> Result<Vec<u8>, String> {
    if encoding == encoding_rs::UTF_16LE {
        let mut out = Vec::with_capacity(text.len().saturating_mul(2).saturating_add(2));
        if bom == TextFileBom::Utf16Le {
            out.extend_from_slice(&[0xff, 0xfe]);
        }
        for unit in text.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        return Ok(out);
    }
    if encoding == encoding_rs::UTF_16BE {
        let mut out = Vec::with_capacity(text.len().saturating_mul(2).saturating_add(2));
        if bom == TextFileBom::Utf16Be {
            out.extend_from_slice(&[0xfe, 0xff]);
        }
        for unit in text.encode_utf16() {
            out.extend_from_slice(&unit.to_be_bytes());
        }
        return Ok(out);
    }

    let mut encoder = encoding.new_encoder();
    let capacity = encoder
        .max_buffer_length_from_utf8_without_replacement(text.len())
        .ok_or_else(|| format!("{} 编码输出过大", encoding.name()))?;
    let mut bytes = Vec::with_capacity(capacity);
    let (result, read) = encoder.encode_from_utf8_to_vec_without_replacement(text, &mut bytes, true);
    match result {
        encoding_rs::EncoderResult::InputEmpty if read == text.len() => {}
        encoding_rs::EncoderResult::Unmappable(ch) => {
            return Err(format!(
                "字符 '{}' 无法用原文件编码 {} 表示，已拒绝写入",
                ch,
                encoding.name()
            ));
        }
        encoding_rs::EncoderResult::OutputFull => {
            return Err(format!("{} 编码缓冲区不足，已拒绝写入", encoding.name()));
        }
        encoding_rs::EncoderResult::InputEmpty => {
            return Err(format!("{} 编码未完整消费输入，已拒绝写入", encoding.name()));
        }
    }

    let mut out = Vec::with_capacity(bytes.len() + 3);
    match bom {
        TextFileBom::None => {}
        TextFileBom::Utf8 => out.extend_from_slice(&[0xef, 0xbb, 0xbf]),
        TextFileBom::Utf16Le => out.extend_from_slice(&[0xff, 0xfe]),
        TextFileBom::Utf16Be => out.extend_from_slice(&[0xfe, 0xff]),
    }
    out.extend_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod text_codec_tests {
    use super::*;

    #[test]
    fn decode_should_prefer_strict_utf8_for_ascii() {
        let decoded = decode_text_file_bytes(b"hello").expect("decode ascii");
        assert_eq!(decoded.text, "hello");
        assert_eq!(decoded.encoding, encoding_rs::UTF_8);
    }

    #[test]
    fn decode_should_strip_and_preserve_utf8_bom() {
        let decoded = decode_text_file_bytes(&[0xef, 0xbb, 0xbf, b'a']).expect("decode bom");
        assert_eq!(decoded.text, "a");
        assert_eq!(decoded.bom, TextFileBom::Utf8);
        assert_eq!(decoded.encode_like_original("b").expect("encode"), vec![0xef, 0xbb, 0xbf, b'b']);
    }

    #[test]
    fn decode_should_support_utf16le_bom() {
        let decoded = decode_text_file_bytes(&[0xff, 0xfe, 0x2d, 0x4e, 0x87, 0x65])
            .expect("decode utf16le");
        assert_eq!(decoded.text, "中文");
        assert_eq!(decoded.bom, TextFileBom::Utf16Le);
        assert_eq!(
            decoded.encode_like_original("中").expect("encode"),
            vec![0xff, 0xfe, 0x2d, 0x4e]
        );
    }

    #[test]
    fn decode_should_support_gbk_chinese() {
        let decoded = decode_text_file_bytes(&[0xd6, 0xd0, 0xce, 0xc4]).expect("decode gbk");
        assert_eq!(decoded.text, "中文");
        let encoded = decoded.encode_like_original("中文").expect("encode gbk");
        assert_eq!(encoded, vec![0xd6, 0xd0, 0xce, 0xc4]);
    }

    #[test]
    fn decode_should_normalize_crlf_and_restore_it_on_encode() {
        // CRLF 文件配上 LF 写法的 old_string 是 Windows 上的常态：模型读到的是 LF，
        // 磁盘上存的是 CRLF。匹配必须只在一套视图上进行，否则每次编辑都要先失败一次。
        let raw = b"line one\r\nline two\r\n";
        let decoded = decode_text_file_bytes(raw).expect("decode crlf");
        assert_eq!(decoded.text, "line one\nline two\n");
        assert_eq!(decoded.line_ending, TextLineEnding::CrLf);

        let written = decoded
            .encode_like_original("line one\nline two\nline three\n")
            .expect("encode crlf");
        assert_eq!(written, b"line one\r\nline two\r\nline three\r\n");
    }

    #[test]
    fn encode_should_not_double_the_carriage_return() {
        // 新内容里如果已经带了 CRLF（模型自己写的），不能再拼出一个 \r\r\n。
        let decoded = decode_text_file_bytes(b"a\r\nb\r\n").expect("decode crlf");
        let written = decoded
            .encode_like_original("x\r\ny\n")
            .expect("encode crlf");
        assert_eq!(written, b"x\r\ny\r\n");
    }

    #[test]
    fn decode_should_keep_lf_files_untouched() {
        let decoded = decode_text_file_bytes(b"a\nb\n").expect("decode lf");
        assert_eq!(decoded.text, "a\nb\n");
        assert_eq!(decoded.line_ending, TextLineEnding::Lf);
        assert_eq!(decoded.encode_like_original("a\nb\n").expect("encode"), b"a\nb\n");
    }

    #[test]
    fn decode_should_pick_the_first_line_ending_for_mixed_files() {
        // 混合行尾文件会被整份统一成第一个出现的行尾。这是已知行为，不是疏忽：
        // 要逐行保留就得把每个替换结果映射回原始字节区间，复杂度和测试面都远超范围。
        // 钉住它是为了以后有人改到这里时，先看到这条测试再决定。
        let decoded = decode_text_file_bytes(b"a\r\nb\nc\n").expect("decode mixed");
        assert_eq!(decoded.text, "a\nb\nc\n");
        assert_eq!(decoded.line_ending, TextLineEnding::CrLf);
        assert_eq!(
            decoded.encode_like_original(&decoded.text).expect("encode"),
            b"a\r\nb\r\nc\r\n"
        );
    }

    #[test]
    fn encode_should_reject_unmappable_character() {
        let decoded = DecodedTextFile {
            text: "中文".to_string(),
            encoding: encoding_rs::GBK,
            bom: TextFileBom::None,
            line_ending: TextLineEnding::Lf,
        };
        let err = decoded
            .encode_like_original("中文🚀")
            .expect_err("emoji should not be encodable as gbk");
        assert!(err.contains("无法用原文件编码"));
    }
}
