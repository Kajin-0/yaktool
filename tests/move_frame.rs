use yaktool::interpret::{parse_move_frame, Interpreter, RuleInterpreter};
use yaktool::intent::{Action, Age};

#[test]
fn deterministic_move_frames_cover_supported_matrix() {
    let mut n = 0;
    for verb in ["move", "relocate", "put"] {
        for source in ["home", "desktop", "documents", "downloads", "pictures", "archive"] {
            for dest in ["home", "desktop", "documents", "downloads", "pictures", "archive"] {
                if source == dest { continue; }
                let cases = [
                    format!("{verb} files from {source} to {dest}"),
                    format!("{verb} PDF files from {source} into {dest}"),
                    format!("{verb} PNG files older than 7 days from {source} in {dest}"),
                    format!("{verb} text files larger than 10 MiB from {source} to {dest}"),
                ];
                for request in cases { assert!(parse_move_frame(&request).unwrap().is_some(), "{request}"); n += 1; }
            }
        }
    }
    assert!(n >= 300);
}

#[test]
fn complete_intent_contains_all_constraints_and_units() {
    let i = parse_move_frame("move JPEG files newer than 7 days larger than 2 MB from pictures to archive").unwrap().unwrap();
    assert_eq!(i.action, Action::Move);
    assert_eq!(i.filters.extensions, vec![".jpg", ".jpeg"]);
    assert!(matches!(i.filters.age, Some(Age::NewerThan(7))));
    assert_eq!(i.filters.min_size_bytes, Some(2_000_000));
}

#[test]
fn ambiguous_or_unsupported_frames_fail_closed() {
    for request in [
        "move files from downloads", "move files to archive", "move files from downloads to downloads",
        "copy files from downloads to archive", "delete files from downloads to archive",
        "do not move files from downloads to archive", "move files from downloads to archive except PDFs",
        "move files from downloads and documents to archive", "move files from downloads to archive or desktop",
        "move files from /tmp to archive", "move PDF PNG files from downloads to archive",
        "move files from downloads to archive older than 3 days and newer than 1 days",
        "move files from downloads to archive larger than 1 MB larger than 2 MB",
        "move and chmod files from downloads to archive",
    ] { assert!(parse_move_frame(request).unwrap().is_none(), "{request}"); }
}

#[test]
fn interpreter_uses_move_frame_and_preserves_safe_fallback() {
    let i = RuleInterpreter.interpret("move PDF files older than 30 days larger than 50 MB from downloads to archive").unwrap();
    assert_eq!(i.action, Action::Move);
    assert_eq!(i.filters.min_size_bytes, Some(50_000_000));
    assert!(RuleInterpreter.interpret("move my stuff somewhere").unwrap().action == Action::Clarify);
}
