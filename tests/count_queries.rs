use common::Fixture;
use std::{fs, os::unix::fs::symlink};
use yaktool::{intent::Action, interpret::{parse_count_query, Interpreter, RuleInterpreter}, resolve::resolve};

mod common;

#[test]
fn count_parser_covers_aliases_and_forms() {
    for (request, action, source) in [
        ("how many directories in home?", Action::CountDirectories, "home"),
        ("how many folders are in Downloads?", Action::CountDirectories, "downloads"),
        ("count directories in archive.", Action::CountDirectories, "archive"),
        ("how many files in Documents?", Action::CountFiles, "documents"),
        ("how many files are in Archive?", Action::CountFiles, "archive"),
        ("COUNT FILES IN Pictures", Action::CountFiles, "pictures"),
    ] {
        let i = parse_count_query(request).unwrap().unwrap();
        assert_eq!(i.action, action);
        assert_eq!(i.source, Some(yaktool::intent::Location::DirectoryAlias(source.into())));
    }
}

#[test]
fn count_queries_are_non_recursive_and_type_safe() {
    let f = Fixture::new();
    f.file("Downloads/a", b"a");
    f.file("Downloads/b", b"b");
    fs::create_dir(f.path("Downloads/dir-a")).unwrap();
    fs::create_dir(f.path("Downloads/dir-b")).unwrap();
    f.file("Downloads/dir-a/nested", b"nested");
    f.file("Downloads/.hidden", b"hidden");
    fs::create_dir(f.path("Downloads/.hidden-dir")).unwrap();
    symlink("a", f.path("Downloads/link-file")).unwrap();
    symlink("dir-a", f.path("Downloads/link-dir")).unwrap();
    let files = resolve(&f.root, &RuleInterpreter.interpret("count files in Downloads").unwrap(), chrono::Local::now()).unwrap();
    assert_eq!(files.entries.len(), 2);
    let dirs = resolve(&f.root, &RuleInterpreter.interpret("how many directories in Downloads?").unwrap(), chrono::Local::now()).unwrap();
    assert_eq!(dirs.entries.len(), 2);
}

#[test]
fn count_parser_rejects_unsupported_modifiers() {
    for request in ["count directories recursively in home", "count files and delete them", "how many directories in home and downloads", "count directories in /etc", "delete files in home", "run count files in home"] {
        assert!(parse_count_query(request).unwrap().is_none(), "{request}");
    }
}
