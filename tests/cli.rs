use std::{fs, process::Command};
fn run(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_changelog-merge"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn output_atomicity_utf8_json_and_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let b = dir.path().join("base");
    let o = dir.path().join("ours");
    let t = dir.path().join("theirs");
    let out = dir.path().join("out");
    let base = "## Unreleased\n### Added\n- Base.\n";
    fs::write(&b, base).unwrap();
    fs::write(&o, base.replace("Base", "Ours")).unwrap();
    fs::write(&t, base.replace("Base", "Theirs")).unwrap();
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
        "--output".as_ref(),
        "-".as_ref(),
        "--diagnostics".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stdout).contains("<<<<<<< ours"));
    let json: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(json["conflicts"].as_array().unwrap().len(), 1);
    assert!(
        json["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "Conflict")
    );
    assert!(fs::read_to_string(&o).unwrap().contains("Ours"));
    assert!(!fs::read_to_string(&o).unwrap().contains("<<<<<<<"));
    fs::write(&t, [0xff]).unwrap();
    fs::write(&out, "untouched").unwrap();
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
        "--output".as_ref(),
        out.as_os_str(),
        "--diagnostics".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read_to_string(&out).unwrap(), "untouched");
    serde_json::from_slice::<serde_json::Value>(&result.stderr).unwrap();
    fs::write(&t, base).unwrap();
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
    ]);
    assert!(result.status.success());
    assert_eq!(
        fs::read_to_string(&o).unwrap(),
        base.replace("Base", "Ours")
    );
}
#[test]
fn checks_errors_and_limits_leave_files_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let b = temp.path().join("base");
    let o = temp.path().join("ours");
    let t = temp.path().join("theirs");
    let source = "## Unreleased\n### Added\n- Base.\n";
    for path in [&b, &o, &t] {
        fs::write(path, source).unwrap();
    }
    assert!(run(&["check".as_ref(), o.as_os_str()]).status.success());
    assert_eq!(fs::read_to_string(&o).unwrap(), source);
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
        "--ours-label".as_ref(),
        "bad\nlabel".as_ref(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read_to_string(&o).unwrap(), source);
    let missing = temp.path().join("missing/output");
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
        "--output".as_ref(),
        missing.as_os_str(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(fs::read_to_string(&o).unwrap(), source);
    assert!(!missing.exists());
    fs::write(&t, vec![b'a'; 32 * 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        run(&[
            "merge".as_ref(),
            b.as_os_str(),
            o.as_os_str(),
            t.as_os_str()
        ])
        .status
        .code(),
        Some(2)
    );
    assert_eq!(fs::read_to_string(&o).unwrap(), source);
}
#[cfg(unix)]
#[test]
fn atomic_replacement_preserves_permissions_and_rejects_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let b = temp.path().join("base");
    let o = temp.path().join("ours");
    let t = temp.path().join("theirs");
    let link = temp.path().join("link");
    let source = "## Unreleased\n### Added\n- Base.\n";
    fs::write(&b, source).unwrap();
    fs::write(&o, source.replace("Base", "Edited")).unwrap();
    fs::write(&t, source).unwrap();
    fs::set_permissions(&o, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(
        run(&[
            "merge".as_ref(),
            b.as_os_str(),
            o.as_os_str(),
            t.as_os_str()
        ])
        .status
        .success()
    );
    assert_eq!(
        fs::metadata(&o).unwrap().permissions().mode() & 0o777,
        0o640
    );
    symlink(&o, &link).unwrap();
    let original = fs::read(&o).unwrap();
    let result = run(&[
        "merge".as_ref(),
        b.as_os_str(),
        o.as_os_str(),
        t.as_os_str(),
        "--output".as_ref(),
        link.as_os_str(),
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&o).unwrap(), original);
}
