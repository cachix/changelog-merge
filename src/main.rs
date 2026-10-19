use changelog_merge::{ConflictStyle, MergeOptions, merge, parse};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
#[derive(Parser)]
#[command(version, about = "Semantic three-way merging for Keep a Changelog.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Merge {
        base: PathBuf,
        ours: PathBuf,
        theirs: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u16).range(3..=256))]
        marker_size: u16,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value = "base")]
        base_label: String,
        #[arg(long, default_value = "ours")]
        ours_label: String,
        #[arg(long, default_value = "theirs")]
        theirs_label: String,
        #[arg(long)]
        explain: bool,
        #[arg(long, value_enum, default_value_t = Diagnostics::Human)]
        diagnostics: Diagnostics,
        #[arg(long)]
        strict: bool,
    },
    Check {
        file: PathBuf,
    },
}
#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
enum Diagnostics {
    Human,
    Json,
}
#[derive(Serialize)]
struct Report<'a> {
    path: Option<&'a str>,
    conflicts: &'a [changelog_merge::MergeConflict],
    changes: Vec<String>,
    events: &'a [changelog_merge::DiagnosticEvent],
}
fn read(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("input exceeds 32 MiB".into());
    }
    Ok(String::from_utf8(bytes)?)
}
fn write(path: &Path, content: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(io::Error::other("refusing to replace a symbolic link"));
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(path) {
        temp.as_file().set_permissions(metadata.permissions())?;
    }
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn run(cli: Cli) -> Result<i32, Box<dyn std::error::Error>> {
    match cli.command {
        Command::Check { file } => {
            let source = read(&file)?;
            let d = parse(&source)?;
            eprintln!(
                "{}: valid changelog, {} releases",
                file.display(),
                d.releases.len()
            );
            Ok(0)
        }
        Command::Merge {
            base,
            ours,
            theirs,
            output,
            marker_size,
            path,
            base_label,
            ours_label,
            theirs_label,
            explain,
            diagnostics,
            strict,
        } => {
            let (b, o, t) = (read(&base)?, read(&ours)?, read(&theirs)?);
            let options = MergeOptions {
                strict,
                conflict_style: ConflictStyle {
                    marker_size: marker_size.into(),
                    base_label,
                    ours_label,
                    theirs_label,
                    ..Default::default()
                },
            };
            let result = merge(&b, &o, &t, &options)?;
            let report = Report {
                path: path.as_deref(),
                conflicts: &result.conflicts,
                events: &result.diagnostics,
                changes: if explain {
                    result
                        .diagnostics
                        .iter()
                        .map(|event| {
                            format!(
                                "{:?} {} {}/{}: {}{}",
                                event.kind,
                                event.branch.as_deref().unwrap_or("result"),
                                event.release.as_deref().unwrap_or("document"),
                                event.category.as_deref().unwrap_or(""),
                                event.message,
                                if event.order.is_empty() {
                                    String::new()
                                } else {
                                    format!(" [{}]", event.order.join(", "))
                                }
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            };
            match diagnostics {
                Diagnostics::Json => eprintln!("{}", serde_json::to_string(&report)?),
                Diagnostics::Human => {
                    for line in &report.changes {
                        eprintln!("{line}");
                    }
                    for c in &result.conflicts {
                        eprintln!(
                            "{}{:?} {}/{}: {}",
                            path.as_ref().map_or(String::new(), |p| format!("{p}: ")),
                            c.kind,
                            c.release.as_deref().unwrap_or("document"),
                            c.category.as_deref().unwrap_or(""),
                            c.message
                        );
                    }
                    if explain && result.conflicts.is_empty() {
                        eprintln!(
                            "clean merge; same-anchor additions use source-order constraints and canonical content ties; identical additions are deduplicated at their insertion anchor"
                        );
                    }
                }
            }
            let destination = output.as_deref().unwrap_or(&ours);
            if destination == Path::new("-") {
                let mut stdout = io::stdout().lock();
                stdout.write_all(result.content.as_bytes())?;
                stdout.flush()?;
            } else {
                write(destination, &result.content)?;
            }
            Ok(if result.conflicts.is_empty() { 0 } else { 1 })
        }
    }
}
fn main() {
    let cli = Cli::parse();
    let json = matches!(
        &cli.command,
        Command::Merge {
            diagnostics: Diagnostics::Json,
            ..
        }
    );
    let code = match std::panic::catch_unwind(|| run(cli)) {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            if json {
                eprintln!("{}", serde_json::json!({"error":e.to_string()}));
            } else {
                eprintln!("changelog-merge: {e}");
            }
            2
        }
        Err(_) => {
            eprintln!("changelog-merge: internal fatal error");
            129
        }
    };
    std::process::exit(code);
}
