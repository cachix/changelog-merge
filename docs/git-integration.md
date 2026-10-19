# Git integration

Git supplies `%O` (base), `%A` (ours, also the output), and `%B` (theirs).
`--marker-size %L` honors attribute marker lengths, and `--path %P` provides only
diagnostic metadata. The executable reads only explicitly supplied input paths.
Conflict labels default to ours/base/theirs; `--ours-label`, `--base-label`, and
`--theirs-label` override them. Labels must be single-line strings.

For a shell session, this configuration requires no persistent Git mutations:

```sh
export GIT_CONFIG_COUNT=3
export GIT_CONFIG_KEY_0=merge.changelog-merge.name
export GIT_CONFIG_VALUE_0='Keep a Changelog semantic merger'
export GIT_CONFIG_KEY_1=merge.changelog-merge.driver
export GIT_CONFIG_VALUE_1='changelog-merge merge %O %A %B --marker-size %L --path %P'
export GIT_CONFIG_KEY_2=merge.changelog-merge.recursive
export GIT_CONFIG_VALUE_2=binary
```

This example assumes no preexisting indexed `GIT_CONFIG_*` entries. If another
tool manages them, append these three entries to its existing count instead of
overwriting its configuration. Use a shell-quoted absolute executable path if the
binary is outside PATH or its path contains spaces.

```sh
git check-attr merge -- CHANGELOG.md
git config --get merge.changelog-merge.driver
```

The attribute should report `changelog-merge`, and the configuration should show
the driver command. `recursive = binary` makes internal merging of multiple merge
bases conservative. Merge, rebase, and cherry-pick all use the driver when their
three-way merges require it; ordinary fast-forward or trivial merges may not.

Git commands outside the configured environment do not inherit these settings.
GitHub and GitLab hosted web merges do not execute arbitrary local merge drivers.
Merge locally or in a configured CI job, then push the result. Tracking attributes
alone does not guarantee semantic merging on those services.

The driver writes a complete temporary file beside the destination, syncs it, and
atomically replaces the destination. Existing file permissions are retained.
Symlink output paths are rejected. Fatal errors leave the destination unchanged;
ordinary conflicts write complete diff3 output and return status 1.
