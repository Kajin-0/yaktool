pub mod common;
use common::Fixture;
use std::{
    ffi::OsString,
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{symlink, MetadataExt, PermissionsExt},
    },
    path::Path,
};
use yaktool::{
    execute::execute,
    filesystem::{rename_noreplace, Root},
    intent::Location,
    interpret::{Interpreter, RuleInterpreter},
    resolve::resolve,
};

#[test]
fn destination_collision() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"source");
    f.file("Archive/a.png", b"destination");
    let e = resolve(
        &f.root,
        &RuleInterpreter
            .interpret("move PNG files from Downloads to Archive")
            .unwrap(),
        chrono::Local::now(),
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "DESTINATION_EXISTS");
    assert_eq!(fs::read(f.path("Downloads/a.png")).unwrap(), b"source");
    assert_eq!(fs::read(f.path("Archive/a.png")).unwrap(), b"destination");
}
#[test]
fn destination_appears_after_preview() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"source");
    let p = f.plan("move PNG files from Downloads to Archive");
    let mut j = f.journal();
    j.store(&p).unwrap();
    f.file("Archive/a.png", b"race");
    assert_eq!(
        execute(&f.root, &p.confirm("y").unwrap(), &mut j)
            .unwrap_err()
            .code,
        "PLAN_INVALIDATED"
    );
    assert_eq!(fs::read(f.path("Archive/a.png")).unwrap(), b"race");
    assert_eq!(fs::read(f.path("Downloads/a.png")).unwrap(), b"source");
}
#[test]
fn kernel_noreplace_is_authoritative_even_without_precheck() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"source");
    let src = f.root.directory(Path::new("Downloads")).unwrap();
    let dst = f.root.directory(Path::new("Archive")).unwrap();
    assert!(f.root.absent(Path::new("Archive/a.png")).unwrap());
    f.file("Archive/a.png", b"raced");
    assert_eq!(
        rename_noreplace(&src, "a.png".as_ref(), &dst, "a.png".as_ref())
            .unwrap_err()
            .code,
        "DESTINATION_EXISTS"
    );
    assert_eq!(fs::read(f.path("Downloads/a.png")).unwrap(), b"source");
    assert_eq!(fs::read(f.path("Archive/a.png")).unwrap(), b"raced");
}
#[test]
fn replaced_source_and_modified_source_invalidate_complete_plan() {
    for replace in [false, true] {
        let f = Fixture::new();
        f.file("Downloads/a.png", b"a");
        f.file("Downloads/z.png", b"z");
        let pinned = fs::File::open(f.path("Downloads/z.png")).unwrap();
        let p = f.plan("move PNG files from Downloads to Archive");
        let mut j = f.journal();
        j.store(&p).unwrap();
        if replace {
            fs::remove_file(f.path("Downloads/z.png")).unwrap();
        }
        f.file("Downloads/z.png", b"changed");
        assert_eq!(
            execute(&f.root, &p.confirm("y").unwrap(), &mut j)
                .unwrap_err()
                .code,
            "PLAN_INVALIDATED"
        );
        assert!(f.path("Downloads/a.png").exists());
        assert_eq!(fs::read_dir(f.path("Archive")).unwrap().count(), 0);
        drop(pinned);
    }
}
#[test]
fn symlink_source_and_directory_escape() {
    let f = Fixture::new();
    f.file("Pictures/real.png", b"target");
    symlink("../Pictures/real.png", f.path("Downloads/link.png")).unwrap();
    assert_eq!(
        resolve(
            &f.root,
            &RuleInterpreter
                .interpret("move PNG files from Downloads to Archive")
                .unwrap(),
            chrono::Local::now()
        )
        .err()
        .unwrap()
        .code,
        "SYMLINK_REJECTED"
    );
    assert_eq!(fs::read(f.path("Pictures/real.png")).unwrap(), b"target");
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("a.png"), b"outside").unwrap();
    fs::remove_dir(f.path("Archive")).unwrap();
    symlink(outside.path(), f.path("Archive")).unwrap();
    assert!(f.root.directory(Path::new("Archive")).is_err());
    assert_eq!(fs::read(outside.path().join("a.png")).unwrap(), b"outside");
}
#[test]
fn substituted_parent_invalidates_plan() {
    for link in [false, true] {
        let f = Fixture::new();
        f.file("Downloads/a.png", b"source");
        let p = f.plan("move PNG files from Downloads to Archive");
        let mut j = f.journal();
        j.store(&p).unwrap();
        fs::rename(f.path("Archive"), f.path("previous-archive")).unwrap();
        if link {
            symlink("Documents", f.path("Archive")).unwrap();
        } else {
            fs::create_dir(f.path("Archive")).unwrap();
        }
        assert_eq!(
            execute(&f.root, &p.confirm("y").unwrap(), &mut j)
                .unwrap_err()
                .code,
            "PLAN_INVALIDATED"
        );
        assert!(f.path("Downloads/a.png").exists());
        assert!(!f.path("Documents/a.png").exists());
    }
}
#[test]
fn traversal_and_outside_root_rejected_by_core() {
    let f = Fixture::new();
    for path in ["../", "../../", "/etc", "Downloads/../../tmp"] {
        let mut i = RuleInterpreter
            .interpret("move PDFs from Downloads to Archive")
            .unwrap();
        i.source = Some(Location::PathBytes(path.as_bytes().to_vec()));
        assert_eq!(
            resolve(&f.root, &i, chrono::Local::now())
                .err()
                .unwrap()
                .code,
            "OUTSIDE_ALLOWED_ROOT"
        );
    }
    let lookalike = format!("{}-other/file", f.temp.path().display());
    assert!(f.root.relative(Path::new(&lookalike)).is_err());
}
#[test]
fn opaque_filenames_round_trip_and_safe_rendering() {
    let f = Fixture::new();
    let mut names: Vec<OsString> = [
        "-leading-hyphen.txt",
        "contains spaces.txt",
        "single'quote.txt",
        "double\"quote.txt",
        "Unicode-λ-文件.txt",
        "tab\tname.txt",
        "newline\nname.txt",
        "escape\u{1b}[31m.txt",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    names.push(OsString::from_vec(b"invalid-\xff.txt".to_vec()));
    for name in &names {
        fs::write(f.path("Downloads").join(name), b"opaque").unwrap();
    }
    let p = f.plan("move text files from Downloads to Archive");
    assert_eq!(p.data().operations.len(), names.len());
    let preview = yaktool::render::preview(&p, true);
    assert!(!preview.contains('\u{1b}'));
    assert!(preview.contains("\\n"));
    assert!(preview.contains("\\t"));
    let stored = yaktool::plan::Plan::stored(&p.json().unwrap(), p.hash()).unwrap();
    let mut j = f.journal();
    j.store(&stored).unwrap();
    execute(&f.root, &stored.confirm("yes").unwrap(), &mut j).unwrap();
    for name in &names {
        assert!(!f.path("Downloads").join(name).exists());
        assert_eq!(fs::read(f.path("Archive").join(name)).unwrap(), b"opaque");
    }
    let u = yaktool::execute::prepare_undo(&f.root, &j).unwrap();
    j.store(&u).unwrap();
    execute(&f.root, &u.confirm("y").unwrap(), &mut j).unwrap();
    for name in names {
        assert_eq!(fs::read(f.path("Downloads").join(name)).unwrap(), b"opaque");
    }
}
#[test]
fn permission_failure_is_typed() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let f = Fixture::new();
    fs::set_permissions(f.path("Archive"), fs::Permissions::from_mode(0o000)).unwrap();
    let error = f.root.directory(Path::new("Archive")).unwrap_err();
    fs::set_permissions(f.path("Archive"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error.code, "PERMISSION_DENIED");
}
#[test]
fn cross_filesystem_primitive_never_copies() {
    // All writes remain inside temporary fixtures, including the alternate-device fixture.
    let f = Fixture::new();
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    if fs::metadata(other.path()).unwrap().dev() == fs::metadata(f.temp.path()).unwrap().dev() {
        return;
    }
    f.file("Downloads/a.png", b"source");
    let other_root = Root::new(other.path()).unwrap();
    let src = f.root.directory(Path::new("Downloads")).unwrap();
    let dst = other_root.directory(Path::new(".")).unwrap();
    assert_eq!(
        rename_noreplace(&src, "a.png".as_ref(), &dst, "a.png".as_ref())
            .unwrap_err()
            .code,
        "CROSS_FILESYSTEM"
    );
    assert_eq!(fs::read(f.path("Downloads/a.png")).unwrap(), b"source");
    assert!(!other.path().join("a.png").exists());
}
#[test]
fn unsupported_requests_do_not_mutate() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"safe");
    for request in [
        "clean up my computer",
        "organize everything",
        "move my stuff",
        "delete the junk",
        "install firefox",
        "sudo apt update",
        "run rm -rf something",
        "fix linux",
    ] {
        assert!(resolve(
            &f.root,
            &RuleInterpreter.interpret(request).unwrap(),
            chrono::Local::now()
        )
        .is_err());
    }
    assert_eq!(fs::read(f.path("Downloads/a.png")).unwrap(), b"safe");
    assert_eq!(fs::read_dir(f.path("Archive")).unwrap().count(), 0);
}

#[test]
fn matching_directories_and_sockets_are_not_move_sources() {
    for socket in [false, true] {
        let f = Fixture::new();
        let _listener = if socket {
            Some(std::os::unix::net::UnixListener::bind(f.path("Downloads/special.png")).unwrap())
        } else {
            fs::create_dir(f.path("Downloads/special.png")).unwrap();
            None
        };
        let e = resolve(
            &f.root,
            &RuleInterpreter
                .interpret("move PNG files from Downloads to Archive")
                .unwrap(),
            chrono::Local::now(),
        )
        .err()
        .unwrap();
        assert_eq!(e.code, "UNSUPPORTED_FILE_TYPE");
        assert!(f.path("Downloads/special.png").exists());
        assert!(!f.path("Archive/special.png").exists());
    }
}

#[test]
fn renamed_root_invalidates_plan() {
    use std::os::unix::fs::DirBuilderExt;
    let outer = tempfile::tempdir().unwrap();
    let original = outer.path().join("root");
    fs::create_dir(&original).unwrap();
    for name in ["Downloads", "Archive", "journal"] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(original.join(name))
            .unwrap();
    }
    fs::write(original.join("Downloads/a.png"), b"source").unwrap();
    let root = Root::new(&original).unwrap();
    let p = resolve(
        &root,
        &RuleInterpreter
            .interpret("move PNG files from Downloads to Archive")
            .unwrap(),
        chrono::Local::now(),
    )
    .unwrap()
    .plan
    .unwrap();
    let mut j = yaktool::journal::Journal::open(&original.join("journal/yaktool.db")).unwrap();
    j.store(&p).unwrap();
    // Keep the journal location stable while substituting the approved root pathname.
    drop(j);
    fs::rename(&original, outer.path().join("old-root")).unwrap();
    fs::create_dir(&original).unwrap();
    j = yaktool::journal::Journal::open(&outer.path().join("old-root/journal/yaktool.db")).unwrap();
    assert!(execute(&root, &p.confirm("yes").unwrap(), &mut j).is_err());
    assert!(outer.path().join("old-root/Downloads/a.png").exists());
    assert!(!outer.path().join("old-root/Archive/a.png").exists());
}
