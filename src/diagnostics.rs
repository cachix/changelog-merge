use crate::document::Span;
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EventKind {
    MatchedRelease,
    MatchedCategory,
    Added,
    Modified,
    Deleted,
    Moved,
    Renamed,
    Ordered,
    Deduplicated,
    OpaqueMerged,
    ReferenceMerged,
    FormattingSelected,
    Conflict,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticEvent {
    pub kind: EventKind,
    pub branch: Option<String>,
    pub release: Option<String>,
    pub category: Option<String>,
    pub base_span: Option<Span>,
    pub branch_span: Option<Span>,
    pub message: String,
    /// Ordered stable entry identities or section identifiers, when applicable.
    pub order: Vec<String>,
}
