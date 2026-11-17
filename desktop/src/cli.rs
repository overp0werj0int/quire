//! Command-line arguments: `quire [folder or note]` and `quire mcp --folder <path>`.

use std::path::PathBuf;

/// What the command line asks to open instead of the last folder.
#[derive(Debug, PartialEq)]
pub enum Start {
    /// A folder, with the path inside it of a note to open.
    Folder(PathBuf, Option<String>),
    /// A note in no known folder, opened in Note mode (`vault::Scope::Note`).
    Note(PathBuf),
}

/// The folder or note named on the command line; `known` are the folders in the list.
pub fn start_target(known: &[PathBuf]) -> Option<Start> {
    if rune::is_headless() {
        // `QUIRE_NOTE` names a note in that folder to open, for snapshots of one note.
        return std::env::var_os("QUIRE_FOLDER").map(|p| Start::Folder(PathBuf::from(p), std::env::var("QUIRE_NOTE").ok()));
    }
    let arg = rune::extra_args().iter().map(PathBuf::from).find(|p| p.exists())?.canonicalize().ok()?;
    target(arg, known)
}

/// A folder opens as itself. A note opens in the known folder that holds it, at any depth, or
/// else on its own. Any other file opens nothing.
fn target(arg: PathBuf, known: &[PathBuf]) -> Option<Start> {
    if arg.is_dir() {
        return Some(Start::Folder(arg, None));
    }
    if !arg.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown")) {
        return None;
    }
    let home = known.iter().filter_map(|k| Some((k, arg.strip_prefix(k.canonicalize().ok()?).ok()?))).max_by_key(|(k, _)| k.components().count());
    Some(match home {
        Some((root, rel)) => {
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            Start::Folder(root.clone(), Some(rel))
        }
        None => Start::Note(arg),
    })
}

/// Runs `quire mcp`, if that is what was asked. Returns false to start the app instead.
pub fn run_subcommand() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            let folder = args.iter().position(|a| a == "--folder").and_then(|i| args.get(i + 1)).map(PathBuf::from);
            let Some(folder) = folder else {
                eprintln!("Usage: quire mcp --folder <path>");
                std::process::exit(2);
            };
            if !folder.is_dir() {
                eprintln!("quire mcp: {} is not a folder", folder.display());
                std::process::exit(2);
            }
            if let Err(e) = crate::mcp::serve_stdio(folder) {
                eprintln!("quire mcp: {e}");
                std::process::exit(1);
            }
            true
        }
        Some("--help" | "-h") => {
            println!("Quire, a plain editor for folders of Markdown files.\n\nUsage:\n  quire [folder | note.md]     Open the app, optionally on a folder or a note\n  quire mcp --folder <path>    Serve a folder to AI apps over MCP (stdio)");
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Start, target};

    #[test]
    fn a_note_opens_in_its_known_folder_or_alone() {
        let dir = std::env::temp_dir().join(format!("quire-cli-{}", std::process::id()));
        let (known, loose) = (dir.join("notes"), dir.join("loose"));
        std::fs::create_dir_all(known.join("Drafts")).unwrap();
        std::fs::create_dir_all(&loose).unwrap();
        for f in [known.join("Drafts/a.md"), loose.join("b.md"), loose.join("c.txt")] {
            std::fs::write(f, "x").unwrap();
        }
        let canon = |p: std::path::PathBuf| p.canonicalize().unwrap();
        let list = [known.clone()];
        assert_eq!(target(canon(known.join("Drafts/a.md")), &list), Some(Start::Folder(known.clone(), Some("Drafts/a.md".into()))), "a note in a known folder opens that folder, not its subfolder");
        assert_eq!(target(canon(loose.join("b.md")), &list), Some(Start::Note(canon(loose.join("b.md")))), "a note in no known folder opens alone");
        assert_eq!(target(canon(loose.join("c.txt")), &list), None, "another file opens nothing");
        assert_eq!(target(canon(loose.clone()), &list), Some(Start::Folder(canon(loose), None)), "a folder opens as itself");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
