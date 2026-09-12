pub mod common;
use common::Fixture;
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, MetadataExt},
    },
};
use yaktool::{
    intent::{Age, Location},
    interpret::{Interpreter, RuleInterpreter},
    resolve::{date_bounds, resolve},
};

#[test]
fn aliases_filters_hidden_types_and_metadata() {
    let f = Fixture::new();
    for (a, p) in [
        ("home", "."),
        ("desktop", "Desktop"),
        ("downloads", "Downloads"),
        ("documents", "Documents"),
        ("pictures", "Pictures"),
        ("archive", "Archive"),
    ] {
        assert_eq!(
            f.root
                .location(&Location::DirectoryAlias(a.into()))
                .unwrap(),
            std::path::Path::new(p)
        );
    }
    f.file("Downloads/a.PDF", b"report");
    f.file("Downloads/.hidden.pdf", b"hidden");
    f.file("Downloads/a.png", b"png");
    fs::create_dir(f.path("Downloads/folder.pdf")).unwrap();
    symlink("a.PDF", f.path("Downloads/link.pdf")).unwrap();
    let r = resolve(
        &f.root,
        &RuleInterpreter.interpret("find PDFs in Downloads").unwrap(),
        chrono::Local::now(),
    )
    .unwrap();
    assert_eq!(r.entries.len(), 2);
    assert!(r.entries[1].1.symlink());
    assert_eq!(
        r.entries[0].1.device_id,
        fs::metadata(f.path("Downloads/a.PDF")).unwrap().dev()
    );
    let list = resolve(
        &f.root,
        &RuleInterpreter.interpret("show Downloads").unwrap(),
        chrono::Local::now(),
    )
    .unwrap();
    assert_eq!(list.entries.len(), 4);
}
#[test]
fn size_filter_is_strict() {
    let f = Fixture::new();
    for (name, n) in [
        ("equal", 500_000_000),
        ("larger", 500_000_001),
        ("smaller", 1),
    ] {
        fs::File::create(f.path(&format!("Downloads/{name}")))
            .unwrap()
            .set_len(n)
            .unwrap();
    }
    let r = resolve(
        &f.root,
        &RuleInterpreter
            .interpret("find files larger than 500 MB in Downloads")
            .unwrap(),
        chrono::Local::now(),
    )
    .unwrap();
    assert_eq!(r.entries.len(), 1);
    assert_eq!(r.entries[0].0.file_name().unwrap().as_bytes(), b"larger");
}
#[test]
fn dates_resolve_to_exact_boundaries() {
    use chrono::{Datelike, Duration, Timelike};
    let now = chrono::Local::now();
    let (before, after) = date_bounds(&Some(Age::OlderThan(30)), now).unwrap();
    assert_eq!(
        before.unwrap(),
        i128::from((now - Duration::days(30)).timestamp_nanos_opt().unwrap())
    );
    assert!(after.is_none());
    let (before, after) = date_bounds(&Some(Age::NewerThan(2)), now).unwrap();
    assert!(before.is_none());
    assert!(after.is_some());
    for age in [Age::Today, Age::ThisWeek] {
        let (before, after) = date_bounds(&Some(age.clone()), now).unwrap();
        assert_eq!(
            before.unwrap(),
            i128::from(now.timestamp_nanos_opt().unwrap()) + 1
        );
        let start = chrono::DateTime::from_timestamp_nanos((after.unwrap() + 1) as i64)
            .with_timezone(&chrono::Local);
        assert_eq!(start.hour(), 0);
        assert_eq!(start.minute(), 0);
        if matches!(age, Age::ThisWeek) {
            assert_eq!(start.weekday(), chrono::Weekday::Mon);
        }
    }
}
#[test]
fn missing_directories_are_not_created() {
    let f = Fixture::new();
    fs::remove_dir(f.path("Archive")).unwrap();
    let e = resolve(
        &f.root,
        &RuleInterpreter
            .interpret("move PDFs from Downloads to Archive")
            .unwrap(),
        chrono::Local::now(),
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "DESTINATION_MISSING");
    assert!(!f.path("Archive").exists());
    fs::remove_dir(f.path("Downloads")).unwrap();
    assert_eq!(
        resolve(
            &f.root,
            &RuleInterpreter.interpret("show Downloads").unwrap(),
            chrono::Local::now()
        )
        .err()
        .unwrap()
        .code,
        "SOURCE_MISSING"
    );
}
