use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
impl Span {
    pub fn text(self, source: &str) -> &str {
        &source[self.start..self.end]
    }
}
pub type ReleaseId = String;
pub type CategoryId = String;
#[derive(Debug, Clone)]
pub struct Document {
    pub source: String,
    pub preamble: Span,
    pub releases: Vec<Release>,
    pub footer: Option<Span>,
    pub opaque_blocks: Vec<OpaqueBlock>,
}
#[derive(Debug, Clone)]
pub struct Release {
    pub id: ReleaseId,
    pub heading: Span,
    pub categories: Vec<Category>,
    pub span: Span,
    pub prefix: Span,
}
#[derive(Debug, Clone)]
pub struct Category {
    pub id: CategoryId,
    pub heading: Span,
    pub entries: Vec<Entry>,
    pub span: Span,
    pub prefix: Span,
    pub suffix: Span,
    /// Source-ordered entries and intervening opaque Markdown.
    pub blocks: Vec<Block>,
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub span: Span,
    pub normalized: String,
    pub fingerprint: [u8; 32],
    pub anchor: EntryAnchor,
}
#[derive(Debug, Clone)]
pub struct EntryAnchor {
    pub position: usize,
}
#[derive(Debug, Clone)]
pub struct OpaqueBlock {
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Block {
    Entry(usize),
    Opaque(Span),
}
