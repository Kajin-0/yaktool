use yaktool::{
    intent::Action,
    interpret::{parse_size, Interpreter, RuleInterpreter},
};
#[test]
fn supported_phrases() {
    for (phrase, expected) in [
        ("show Downloads", Action::List),
        ("list Downloads", Action::List),
        ("show me Downloads", Action::List),
        ("show me the files in Downloads", Action::List),
        ("find PDFs in Downloads", Action::Search),
        ("find PDF files in downloads", Action::Search),
        ("find PNG files in Pictures", Action::Search),
        (
            "find files larger than 500 MB in Downloads",
            Action::FindLarge,
        ),
        ("find files older than 30 days in Downloads", Action::Search),
        ("find PDFs modified this week in Documents", Action::Search),
        ("find files newer than 3 days in home", Action::Search),
        ("find text modified today in Documents", Action::Search),
        (
            "move PNG files older than 30 days from Downloads to Archive",
            Action::Move,
        ),
        ("move PDFs from Downloads to Documents", Action::Move),
        (
            "Move PNG files older than 30 days from Downloads to Archive.",
            Action::Move,
        ),
    ] {
        assert_eq!(
            RuleInterpreter.interpret(phrase).unwrap().action,
            expected,
            "{phrase}"
        );
    }
}
#[test]
fn unsupported_and_ambiguous_fail_closed() {
    for phrase in [
        "clean up my computer",
        "organize everything",
        "move my stuff",
        "move my stuff somewhere",
        "delete the junk",
        "install firefox",
        "sudo apt update",
        "run rm -rf something",
        "fix linux",
        "wipe downloads",
        "move PDFs from Downloads",
        "move PDFs from Downloads to Archive; touch pwned",
        "find PDFs recursively in home",
    ] {
        assert!(
            matches!(
                RuleInterpreter.interpret(phrase).unwrap().action,
                Action::Unsupported | Action::Clarify
            ),
            "{phrase}"
        );
    }
}
#[test]
fn explicit_sizes_and_overflow() {
    for (unit, expected) in [
        ("KB", 1000),
        ("MB", 1_000_000),
        ("GB", 1_000_000_000),
        ("KiB", 1024),
        ("MiB", 1_048_576),
        ("GiB", 1_073_741_824),
    ] {
        assert_eq!(parse_size("1", unit).unwrap(), expected);
    }
    assert!(parse_size("18446744073709551615", "GB").is_err());
    assert!(parse_size("1.5", "MB").is_err());
}
