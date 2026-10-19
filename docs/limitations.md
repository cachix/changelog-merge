# Remaining limits

- Ambiguous entry identities, section replacements, moves, and release membership
  deliberately conflict. Matching uses documented evidence thresholds, not semantic
  interpretation. Different concurrent edits inside an entry remain atomic.
- Duplicate base entries keep positional identities within a section. Relocating
  indistinguishable duplicate occurrences cannot establish a unique global move
  identity and conflicts. Reordering indistinguishable duplicates has no observable
  textual identity beyond their occurrence positions.
- Unknown category names are supported, but arbitrary heading hierarchies, duplicate
  release/category identities, and ambiguous release wording are rejected. Tables
  and unattached prose remain opaque rather than semantic changelog entries.
- Release cuts that rewrite entries without clear correspondence, cut against
  concurrent edits/deletions, or choose competing release destinations conflict.
  Concurrent, incompatible section headings or release dates conflict as well.
- Opaque regions merge at line granularity; reference definitions are atomic. A
  competing rewrite to the same opaque lines or definition conflicts. Ambiguous
  cross-region reference relocation can conflict or fail validation. Duplicate
  selected reference labels cannot be silently reconciled.
- Existing conflict-marker-looking lines are rejected, including examples inside
  code fences. This includes Setext heading underlines made from three or more
  equals signs. Setext level-two headings using hyphens are recognized.
- Each input and reparsed clean output is limited to 32 MiB. Parsing permits up to
  100,000 structural blocks, 10,000 headings, and 2,000,000 Markdown events. Larger
  documents return an input/validation error without replacing file output.
- Inferred edit gaps permit at most 65,536 candidate pairs and 8 MiB of estimated
  comparison work. Similarity ignores candidates larger than 64 KiB combined.
  Global edited-move and section-rename searches have 65,536-pair caps. Competing
  opaque merges have a combined 65,536-token cap. Budget exhaustion conflicts
  conservatively instead of guessing. These bounds are deterministic.
- No-op and one-sided merges preserve bytes exactly. Concurrent merges preserve
  existing source slices and normalize only inserted entry/section slices. New
  opaque line edits can retain their supplying branch's endings. Mixed documents
  may therefore retain mixed endings by design.
- Fuzzing targets, seed inputs, property tests, failure tests, and reproducible
  benchmarks are included. Short local campaigns and timing samples are evidence,
  not proof of correctness or a performance guarantee on other machines. See
  [testing](testing.md) and [performance](performance.md).
- A security review, a full Nix build with recent nixpkgs, nixpkgs publication,
  and devenv integration remain separate work. The executable has no network service, daemon, or telemetry.
