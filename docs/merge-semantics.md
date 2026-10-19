# Merge semantics

## Parsing and source ownership

Markdown source offsets and list nesting identify level-two releases, level-three
categories, and top-level list items. Fenced headings and nested list items remain
inside their owning entry. Every span refers to its owning input document.

Categories expose an ordered sequence of entry and opaque blocks. Paragraphs,
comments, and definitions between entries are explicit opaque blocks. Prefixes,
suffixes, and trailing reference definitions retain their source bytes. Earlier
reference definitions remain in their original regions. Tables remain opaque.

Release identifiers match case-insensitively without their date metadata. Custom
categories match trimmed, case-insensitive names. Duplicate section identifiers
are rejected. Structural budgets are documented in [limitations](limitations.md).

## Matching and atomic edits

Matching normalizes CRLF to LF and removes terminal newlines. Spaces on content
lines remain significant. Exact content occurrences match positionally, keeping
existing duplicates distinct. Exact matches may reorder. An O(n log n) longest
increasing subsequence of paired base positions provides stable LCS anchors.

Unmatched gaps permit one-to-one edits. Larger gaps use word-token Jaccard overlap
to identify candidates: at least 0.5 overlap, at least 0.15 separation from the
next candidate, and no competing claim on the same target. Unresolved candidates
on both sides of a gap conflict. Similarity identifies entry identity only; it
never reconciles different concurrent content edits. `--strict` rejects inferred
entry matches during concurrent merges.

Existing entry bodies are atomic. One-sided edits and deletions are accepted;
identical edits appear once. Different concurrent edits and delete/edit pairs
conflict. Terminal newline spacing merges separately from the entry body, allowing
one branch's spacing change to accompany the other's content change.

## Ordering and duplicate additions

Each entry has a base occurrence identity or an addition identity containing its
surrounding base anchors, canonical source bytes, and occurrence number. Identical
additions at the same logical anchor deduplicate, allowing LF/CRLF variants.
Repeated additions within a branch remain repeated. Different anchors remain
distinct, even when their prose is identical.

Both branch sequences contribute ordering constraints. A forward base-order edge
is omitted when the other branch explicitly reverses that pair. This permits
one-sided reorders and compatible independent reorders. Remaining cycles conflict.
New additions retain each branch's relative order. Topological ties use canonical
content bytes for additions and stable positional IDs for existing entries.
Sections use normalized names for ties. There are no time, branch-name, or
hash-iteration tie-breakers, and release versions are not semantically sorted.

## Section renames and entry moves

Section renames require a unique content correspondence: identical nonempty body
fingerprints or at least 0.5 child-identity overlap, separated by 0.2 from the next
candidate. The normalized internal identity remains the base identity while the
selected raw heading retains the new name. Renaming one branch can combine with
entry edits on the other. Different concurrent heading changes conflict.
Unidentifiable replacements conflict instead of appearing as independent sections.

Before recursive merging, a document-wide pass identifies unique entries moved
between sections. Exact global matches override local inferred matches. Edited
moves need a unique unmatched destination with at least 0.6 token overlap and a
0.2 margin. Duplicate move identities, competing destinations, moves against
removal, and moves against concurrent modification conflict.

Unambiguous moves relocate the corresponding base identity and unchanged
counterpart into a virtual destination. Destination anchors are matched in bulk,
and source-local spans retain their original ownership. Branch ordering at an
existing destination remains intact. Injected virtual counterparts contribute content
identity, but no ordering constraints at their destination. Only original branch
sequences establish moved-entry order; ties retain original base source positions.
Empty source sections removed by the moving
branch may disappear; independent new entries in those sections remain.

## Release cuts

Relocating unique `Unreleased` entries into a new version establishes released
membership. Released entries stay in that version; independently added entries
remain in `Unreleased`, including when the cutting branch removed its empty
categories or the entire `Unreleased` heading. New release dates and link changes
are merged as metadata rather than included in release identity.

Competing release destinations, different concurrent release metadata, a release
cut against an entry edit/deletion, and released membership that cannot be inferred
safely produce conflicts. A wholly one-sided cut is preserved byte for byte.

## Opaque Markdown and references

Opaque text uses a line-based three-way diff. Disjoint changes merge; overlapping
changes conflict. Entry bodies never use this fallback.

Reference definitions are atomic keyed tokens, including multiline URLs/titles.
Their label identities and source positions merge separately from their values.
Independent definition edits and different concurrent added labels combine.
Different edits to the same definition, deletion against modification, duplicate
selected labels, and ambiguous definition placement fail safely. Interleaved
reference definitions keep their positions; no footer reformatting is performed.

## Formatting and validation

The renderer copies selected source slices. It does not serialize the Markdown
AST. No-op and one-sided merges preserve exact bytes. Concurrent merges preserve
untouched mixed line endings and select one-sided whitespace changes. Inserted
entries and sections follow the base's prevailing ending, with LF for ties or a
base without newlines. Generated separators follow the surrounding output's
prevailing ending. Conflict markers use LF. Existing content slices are never
normalized across the whole document.

Every clean concurrent result is reparsed. Selected headings, entry content and
duplicate counts at each release/category path, and reference-definition
inventories must match. A validation failure prevents output replacement.
Conflict output intentionally contains unresolved Git markers and is not parsed
as a clean changelog.

## Diagnostics

The library returns ordered `DiagnosticEvent` records alongside conflicts. The CLI
sends them through `--explain` or `--diagnostics json`. Events cover matched
sections, entry additions/edits/removals, moves, renames, formatting selection,
ordering, deduplication, opaque/reference reconciliation, and conflicts. Entry
messages include a stable fingerprint and a first-line preview. Ordering events
include the selected identity sequence. Branch events carry source-local spans
when available; virtual headings and synthesized regions may have no span.
