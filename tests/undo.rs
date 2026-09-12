pub mod common;
use common::Fixture;
use yaktool::execute::{execute, prepare_undo};
fn moved(f: &Fixture) -> yaktool::journal::Journal {
    f.file("Downloads/a.png", b"original");
    let p = f.plan("move PNG files from Downloads to Archive");
    let mut j = f.journal();
    j.store(&p).unwrap();
    execute(&f.root, &p.confirm("yes").unwrap(), &mut j).unwrap();
    j
}
#[test]
fn undo_collision_refuses_without_overwrite() {
    let f = Fixture::new();
    let j = moved(&f);
    f.file("Downloads/a.png", b"new source");
    assert_eq!(
        prepare_undo(&f.root, &j).unwrap_err().code,
        "UNDO_STATE_CHANGED"
    );
    assert_eq!(
        std::fs::read(f.path("Downloads/a.png")).unwrap(),
        b"new source"
    );
    assert_eq!(std::fs::read(f.path("Archive/a.png")).unwrap(), b"original");
}
#[test]
fn undo_destination_replacement_refuses() {
    let f = Fixture::new();
    let j = moved(&f);
    let pinned = std::fs::File::open(f.path("Archive/a.png")).unwrap();
    std::fs::remove_file(f.path("Archive/a.png")).unwrap();
    f.file("Archive/a.png", b"replacement");
    assert_eq!(
        prepare_undo(&f.root, &j).unwrap_err().code,
        "UNDO_STATE_CHANGED"
    );
    assert_eq!(
        std::fs::read(f.path("Archive/a.png")).unwrap(),
        b"replacement"
    );
    assert!(!f.path("Downloads/a.png").exists());
    drop(pinned);
}
#[test]
fn undo_modified_destination_refuses() {
    let f = Fixture::new();
    let j = moved(&f);
    f.file("Archive/a.png", b"changed content");
    assert_eq!(
        prepare_undo(&f.root, &j).unwrap_err().code,
        "UNDO_STATE_CHANGED"
    );
    assert!(!f.path("Downloads/a.png").exists());
}
#[test]
fn undo_whole_plan_preflight() {
    let f = Fixture::new();
    f.file("Downloads/b.png", b"b");
    let j = moved(&f);
    f.file("Downloads/b.png", b"collision");
    assert_eq!(
        prepare_undo(&f.root, &j).unwrap_err().code,
        "UNDO_STATE_CHANGED"
    );
    assert!(f.path("Archive/a.png").exists());
    assert!(f.path("Archive/b.png").exists());
    assert!(!f.path("Downloads/a.png").exists());
}
#[test]
fn empty_history() {
    let f = Fixture::new();
    assert_eq!(
        prepare_undo(&f.root, &f.journal()).unwrap_err().code,
        "NOTHING_TO_UNDO"
    );
}
