//! Source-preserving three-way merging for Keep a Changelog.
//!
//! The library performs no filesystem access and never executes Git.
//!
//! ```
//! use changelog_merge::{merge, MergeOptions};
//! let base = "## Unreleased\n### Added\n- Base.\n";
//! let ours = base.replace("- Base.", "- SSH.\n- Base.");
//! let theirs = base.replace("- Base.", "- S3.\n- Base.");
//! let result = merge(base, &ours, &theirs, &MergeOptions::default())?;
//! assert!(result.conflicts.is_empty());
//! assert!(result.content.contains("- S3.\n- SSH.\n- Base."));
//! # Ok::<(), changelog_merge::MergeError>(())
//! ```
pub mod conflict;
pub mod diagnostics;
pub mod diff;
pub mod document;
pub mod error;
pub mod identity;
mod matching;
mod merge;
mod ordering;
mod parser;
mod relocation;
mod render;
mod text_merge;
pub use conflict::{ConflictKind, ConflictStyle, MergeConflict};
pub use diagnostics::{DiagnosticEvent, EventKind};
pub use document::Document;
pub use error::{MergeError, ParseError};
pub use merge::{MergeOptions, MergeOutcome, merge};
pub use parser::parse;
