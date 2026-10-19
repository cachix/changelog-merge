use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Repo {
    _temp: tempfile::TempDir,
    root: PathBuf,
}
fn driver() -> String {
    format!(
        "'{}' merge %O %A %B --marker-size %L --path %P",
        env!("CARGO_BIN_EXE_changelog-merge")
            .replace('\\', "/")
            .replace('\'', "'\\''")
    )
}
fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .env("GIT_CONFIG_COUNT", "5")
        .env("GIT_CONFIG_KEY_0", "merge.changelog-merge.name")
        .env("GIT_CONFIG_VALUE_0", "Keep a Changelog semantic merger")
        .env("GIT_CONFIG_KEY_1", "merge.changelog-merge.driver")
        .env("GIT_CONFIG_VALUE_1", driver())
        .env("GIT_CONFIG_KEY_2", "merge.changelog-merge.recursive")
        .env("GIT_CONFIG_VALUE_2", "binary")
        .env("GIT_CONFIG_KEY_3", "core.autocrlf")
        .env("GIT_CONFIG_VALUE_3", "false")
        .env("GIT_CONFIG_KEY_4", "commit.gpgsign")
        .env("GIT_CONFIG_VALUE_4", "false")
        .output()
        .unwrap()
}
fn ok(root: &Path, args: &[&str]) {
    let o = git(root, args);
    assert!(
        o.status.success(),
        "git {args:?}\n{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
const BASE: &str = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Base.\n";
impl Repo {
    fn new(source_conflict: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo with spaces");
        fs::create_dir(&root).unwrap();
        ok(&root, &["init", "-b", "main"]);
        fs::write(root.join("CHANGELOG.md"), BASE).unwrap();
        fs::write(
            root.join(".gitattributes"),
            "CHANGELOG.md merge=changelog-merge\n",
        )
        .unwrap();
        fs::write(root.join("source.txt"), "base\n").unwrap();
        ok(&root, &["add", "."]);
        ok(&root, &["commit", "-m", "Base"]);
        ok(&root, &["switch", "-c", "feature"]);
        fs::write(
            root.join("CHANGELOG.md"),
            BASE.replace("- Base.", "- Feature.\n- Base."),
        )
        .unwrap();
        if source_conflict {
            fs::write(root.join("source.txt"), "feature\n").unwrap();
        }
        ok(&root, &["commit", "-am", "Feature"]);
        ok(&root, &["switch", "main"]);
        fs::write(
            root.join("CHANGELOG.md"),
            BASE.replace("- Base.", "- Main.\n- Base."),
        )
        .unwrap();
        if source_conflict {
            fs::write(root.join("source.txt"), "main\n").unwrap();
        }
        ok(&root, &["commit", "-am", "Main"]);
        Self { _temp: temp, root }
    }
    fn verify(&self, root: &Path) {
        assert_eq!(
            fs::read_to_string(root.join("CHANGELOG.md")).unwrap(),
            BASE.replace("- Base.", "- Feature.\n- Main.\n- Base.")
        );
        assert!(git(root, &["ls-files", "-u"]).stdout.is_empty());
        assert!(git(root, &["status", "--porcelain"]).stdout.is_empty());
        let config = fs::read_to_string(self.root.join(".git/config")).unwrap();
        assert!(!config.contains("changelog-merge"));
    }
}
#[test]
fn merge_rebase_cherry_pick() {
    for operation in ["merge", "rebase", "cherry-pick"] {
        let r = Repo::new(false);
        match operation {
            "merge" => ok(&r.root, &["merge", "--no-edit", "feature"]),
            "rebase" => {
                ok(&r.root, &["switch", "feature"]);
                ok(&r.root, &["rebase", "main"]);
            }
            _ => ok(&r.root, &["cherry-pick", "feature"]),
        }
        r.verify(&r.root);
    }
}
#[test]
fn linked_worktrees_and_non_root_cwd() {
    let r = Repo::new(false);
    let worktree = r._temp.path().join("linked worktree with spaces");
    ok(
        &r.root,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            worktree.to_str().unwrap(),
            "main",
        ],
    );
    fs::create_dir(worktree.join("subdir")).unwrap();
    ok(&worktree.join("subdir"), &["merge", "--no-edit", "feature"]);
    r.verify(&worktree);
}
#[test]
fn ordinary_source_conflict_leaves_changelog_merged() {
    let r = Repo::new(true);
    let output = git(&r.root, &["merge", "--no-edit", "feature"]);
    assert!(!output.status.success());
    let unmerged = String::from_utf8(git(&r.root, &["ls-files", "-u"]).stdout).unwrap();
    assert!(unmerged.contains("source.txt"));
    assert!(!unmerged.contains("CHANGELOG.md"));
    assert!(
        !fs::read_to_string(r.root.join("CHANGELOG.md"))
            .unwrap()
            .contains("<<<<<<<")
    );
}
#[test]
fn semantic_conflict_is_unmerged_in_git_index() {
    let r = Repo::new(false);
    for (branch, label) in [("main", "Main edit"), ("feature", "Feature edit")] {
        ok(&r.root, &["switch", branch]);
        let path = r.root.join("CHANGELOG.md");
        let source = fs::read_to_string(&path)
            .unwrap()
            .replace("- Base.", &format!("- {label}."));
        fs::write(path, source).unwrap();
        ok(&r.root, &["commit", "-am", label]);
    }
    ok(&r.root, &["switch", "main"]);
    let result = git(&r.root, &["merge", "--no-edit", "feature"]);
    assert_eq!(result.status.code(), Some(1));
    let unmerged = String::from_utf8(git(&r.root, &["ls-files", "-u"]).stdout).unwrap();
    assert_eq!(unmerged.lines().count(), 3);
    assert!(unmerged.lines().all(|line| line.ends_with("CHANGELOG.md")));
    let content = fs::read_to_string(r.root.join("CHANGELOG.md")).unwrap();
    assert!(content.contains("<<<<<<< ours\n"));
    assert!(content.contains("- Main edit."));
    assert!(content.contains("||||||| base\n"));
    assert!(content.contains("- Base."));
    assert!(content.contains("- Feature edit."));
    assert!(content.contains(">>>>>>> theirs"));
    assert_eq!(
        fs::read_to_string(r.root.join("source.txt")).unwrap(),
        "base\n"
    );
}
#[test]
fn real_git_release_cut_preserves_post_cut_additions() {
    let r = Repo::new(false);
    ok(&r.root, &["switch", "feature"]);
    let cut = "# Changelog\n\n## [Unreleased]\n\n## [1.0.0] - 2026-10-19\n\n### Added\n\n- Feature.\n- Base.\n";
    fs::write(r.root.join("CHANGELOG.md"), cut).unwrap();
    ok(&r.root, &["commit", "-am", "Cut release"]);
    ok(&r.root, &["switch", "main"]);
    ok(&r.root, &["merge", "--no-edit", "feature"]);
    let content = fs::read_to_string(r.root.join("CHANGELOG.md")).unwrap();
    let d = changelog_merge::parse(&content).unwrap();
    assert_eq!(d.releases[0].id, "unreleased");
    assert_eq!(d.releases[0].categories[0].entries.len(), 1);
    assert_eq!(d.releases[0].categories[0].entries[0].normalized, "- Main.");
    assert_eq!(d.releases[1].id, "1.0.0");
    assert_eq!(d.releases[1].categories[0].entries.len(), 2);
    assert!(git(&r.root, &["ls-files", "-u"]).stdout.is_empty());
    assert!(git(&r.root, &["status", "--porcelain"]).stdout.is_empty());
}
