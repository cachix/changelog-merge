use crate::document::Span;
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConflictKind {
    ConcurrentModification,
    DeleteModify,
    AmbiguousEntryIdentity,
    ConflictingMove,
    SectionRename,
    SectionDeletion,
    ReleaseCut,
    Ordering,
    OpaqueContent,
    UnsupportedStructure,
}
#[derive(Debug, Clone, Serialize)]
pub struct MergeConflict {
    pub kind: ConflictKind,
    pub release: Option<String>,
    pub category: Option<String>,
    pub base_span: Option<Span>,
    pub ours_span: Option<Span>,
    pub theirs_span: Option<Span>,
    pub message: String,
}
#[derive(Debug, Clone)]
pub struct ConflictStyle {
    pub marker_size: usize,
    pub base_label: String,
    pub ours_label: String,
    pub theirs_label: String,
    pub diff3: bool,
}
impl Default for ConflictStyle {
    fn default() -> Self {
        Self {
            marker_size: 7,
            base_label: "base".into(),
            ours_label: "ours".into(),
            theirs_label: "theirs".into(),
            diff3: true,
        }
    }
}
pub(crate) fn markers(base: &str, ours: &str, theirs: &str, style: &ConflictStyle) -> String {
    let line = |s: &str| {
        if s.is_empty() || s.ends_with('\n') {
            s.to_owned()
        } else {
            format!("{s}\n")
        }
    };
    let n = style.marker_size;
    let mut out = format!("{} {}\n{}", "<".repeat(n), style.ours_label, line(ours));
    if style.diff3 {
        out.push_str(&format!(
            "{} {}\n{}",
            "|".repeat(n),
            style.base_label,
            line(base)
        ));
    }
    out.push_str(&format!(
        "{}\n{}{} {}\n",
        "=".repeat(n),
        line(theirs),
        ">".repeat(n),
        style.theirs_label
    ));
    out
}
