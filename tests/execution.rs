pub mod common;
use common::Fixture;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    time::{Duration, SystemTime},
};
use yaktool::{
    execute::{execute, OperationState},
    plan::Plan,
    render,
};

#[test]
fn reference_workflow_with_journal_and_undo() {
    let f = Fixture::new();
    f.file("Downloads/old.png", b"old png");
    f.file("Downloads/new.png", b"new png");
    f.file("Downloads/report.pdf", b"pdf");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(f.path("Downloads/old.png"))
        .unwrap();
    file.set_times(
        fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(40 * 86400)),
    )
    .unwrap();
    let old = file.metadata().unwrap();
    let plan = f.plan("move PNG files older than 30 days from Downloads to Archive");
    assert_eq!(plan.data().operations.len(), 1);
    assert_eq!(plan.data().operations[0].source_snapshot.inode, old.ino());
    let preview = render::preview(&plan, true);
    assert!(preview.contains("old.png"));
    assert!(f.path("Downloads/old.png").exists());
    let json = plan.json().unwrap();
    assert_eq!(
        Plan::stored(&json, plan.hash()).unwrap().json().unwrap(),
        json
    );
    let mut journal = f.journal();
    journal.store(&plan).unwrap();
    let id = plan.data().plan_id.clone();
    assert_eq!(journal.get(&id).unwrap().status, "planned");
    assert!(plan.clone().confirm("").is_none());
    assert!(plan.clone().confirm("okay").is_none());
    let confirmed = plan.confirm("yes").unwrap();
    let result = execute(&f.root, &confirmed, &mut journal).unwrap();
    assert_eq!(result.status, "verified");
    assert_eq!(result.operations[0].state, OperationState::Verified);
    assert!(!f.path("Downloads/old.png").exists());
    assert!(f.path("Downloads/new.png").exists());
    assert!(f.path("Downloads/report.pdf").exists());
    let moved = fs::metadata(f.path("Archive/old.png")).unwrap();
    assert_eq!((moved.dev(), moved.ino()), (old.dev(), old.ino()));
    let record = journal.get(&id).unwrap();
    assert!(record.reversible);
    assert_eq!(record.plan.json().unwrap(), json);
    assert_eq!(record.status, "verified");
    assert_eq!(journal.recent().unwrap().len(), 1);
    let undo = yaktool::execute::prepare_undo(&f.root, &journal).unwrap();
    let undo_id = undo.data().plan_id.clone();
    journal.store(&undo).unwrap();
    let result = execute(&f.root, &undo.confirm("y").unwrap(), &mut journal).unwrap();
    assert_eq!(result.status, "verified");
    assert_eq!(
        fs::metadata(f.path("Downloads/old.png")).unwrap().ino(),
        old.ino()
    );
    assert!(!f.path("Archive/old.png").exists());
    assert_eq!(journal.get(&id).unwrap().status, "undone");
    assert_eq!(journal.get(&undo_id).unwrap().status, "verified");
    assert_eq!(journal.recent().unwrap().len(), 2);
}
#[test]
fn multiple_files_only_matching_move() {
    let f = Fixture::new();
    for name in ["a.png", "b.PNG", "c.png", "keep.pdf", ".hidden.png"] {
        f.file(&format!("Downloads/{name}"), name.as_bytes());
    }
    let plan = f.plan("move PNG files from Downloads to Archive");
    assert_eq!(plan.data().operations.len(), 3);
    let mut j = f.journal();
    j.store(&plan).unwrap();
    let result = execute(&f.root, &plan.confirm("y").unwrap(), &mut j).unwrap();
    assert!(result
        .operations
        .iter()
        .all(|r| r.state == OperationState::Verified));
    for name in ["a.png", "b.PNG", "c.png"] {
        assert!(!f.path(&format!("Downloads/{name}")).exists());
        assert_eq!(
            fs::read(f.path(&format!("Archive/{name}"))).unwrap(),
            name.as_bytes()
        );
    }
    assert!(f.path("Downloads/keep.pdf").exists());
    assert!(f.path("Downloads/.hidden.png").exists());
}
#[test]
fn plan_integrity_and_confirmation() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"a");
    let plan = f.plan("move PNG files from Downloads to Archive");
    for answer in ["", "n", "YES please", "true", "1"] {
        assert!(plan.clone().confirm(answer).is_none());
    }
    assert!(plan.clone().confirm(" YES\n").is_some());
    let mut altered = plan.data().clone();
    altered.operations[0].destination = b"Documents/a.png".to_vec();
    assert!(Plan::stored(&serde_json::to_string(&altered).unwrap(), plan.hash()).is_err());
    assert!(Plan::stored(
        &plan
            .json()
            .unwrap()
            .replace("yaktool.plan.v1", "yaktool.plan.v2"),
        plan.hash()
    )
    .is_err());
    assert!(Plan::stored(&plan.json().unwrap(), "bad hash").is_err());
    assert!(f.path("Downloads/a.png").exists());
}
#[test]
fn no_journal_no_mutation_and_replay_refused() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"a");
    let p = f.plan("move PNG files from Downloads to Archive");
    let c = p.confirm("y").unwrap();
    let mut j = f.journal();
    assert!(execute(&f.root, &c, &mut j).is_err());
    assert!(f.path("Downloads/a.png").exists());
    j.store(c.plan()).unwrap();
    execute(&f.root, &c, &mut j).unwrap();
    assert!(execute(&f.root, &c, &mut j).is_err());
    assert_eq!(fs::read(f.path("Archive/a.png")).unwrap(), b"a");
}
