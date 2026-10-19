pub fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end_matches('\n').to_owned()
}
pub fn release(text: &str) -> Option<String> {
    let text = text.trim();
    let id = if let Some(rest) = text.strip_prefix('[') {
        let end = rest.find(']')?;
        let tail = rest[end + 1..].trim();
        if !tail.is_empty() && !tail.starts_with("- ") {
            return None;
        }
        &rest[..end]
    } else {
        text.split(" - ").next()?
    };
    if id.eq_ignore_ascii_case("unreleased")
        || (!id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-+_".contains(c)))
    {
        Some(id.to_lowercase())
    } else {
        None
    }
}
