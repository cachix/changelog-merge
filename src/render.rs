pub(crate) fn append(out: &mut String, text: &str) {
    if !out.is_empty() && !out.ends_with('\n') && !text.is_empty() {
        out.push_str(ending(out));
    }
    out.push_str(text);
}
pub(crate) fn line_endings(text: &str, template: &str) -> String {
    let crlf = template
        .as_bytes()
        .windows(2)
        .filter(|s| *s == b"\r\n")
        .count();
    let lf = template.bytes().filter(|c| *c == b'\n').count();
    let text = text.replace("\r\n", "\n");
    if crlf > lf / 2 {
        text.replace('\n', "\r\n")
    } else {
        text
    }
}

/// Generated separators and inserted text follow the base's prevailing convention;
/// ties and files without newlines use LF. Existing source slices are untouched.
pub(crate) fn ending(source: &str) -> &'static str {
    let crlf = source
        .as_bytes()
        .windows(2)
        .filter(|w| *w == b"\r\n")
        .count();
    let lf = source.bytes().filter(|b| *b == b'\n').count();
    if crlf > lf / 2 { "\r\n" } else { "\n" }
}
