pub mod common;
use common::Fixture;
use yaktool::{
    execute::{execute, prepare_undo, ExecutionResult, OperationResult, OperationState},
    journal::Journal,
};

#[test]
fn durable_reopen_and_readers_during_confirmation() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"a");
    let plan = f.plan("move PNG files from Downloads to Archive");
    let id = plan.data().plan_id.clone();
    let j = f.journal();
    j.store(&plan).unwrap();
    let reader = Journal::read_only(&f.path("journal/yaktool.db")).unwrap();
    assert_eq!(reader.get(&id).unwrap().plan.hash(), plan.hash());
    drop(reader);
    assert!(Journal::open(&f.path("journal/yaktool.db")).is_err());
    drop(j);
    let j = f.journal();
    assert_eq!(j.get(&id).unwrap().status, "planned");
    assert_eq!(
        j.get(&id).unwrap().plan.json().unwrap(),
        plan.json().unwrap()
    );
}
#[test]
fn database_failure_prevents_filesystem_mutation() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"source");
    let p = f.plan("move PNG files from Downloads to Archive");
    let mut j = f.journal();
    j.store(&p).unwrap();
    let sql = rusqlite::Connection::open(f.path("journal/yaktool.db")).unwrap();
    sql.execute_batch("CREATE TRIGGER fail_result BEFORE UPDATE OF result_json ON transactions BEGIN SELECT RAISE(ABORT, 'simulated storage failure'); END;").unwrap();
    assert_eq!(
        execute(&f.root, &p.confirm("y").unwrap(), &mut j)
            .unwrap_err()
            .code,
        "DATABASE_ERROR"
    );
    assert_eq!(std::fs::read(f.path("Downloads/a.png")).unwrap(), b"source");
    assert!(!f.path("Archive/a.png").exists());
    assert_eq!(j.recent().unwrap()[0].status, "confirmed");
}
#[test]
fn interrupted_attempt_is_durable_and_never_guessed_for_undo() {
    let f = Fixture::new();
    f.file("Downloads/a.png", b"a");
    let p = f.plan("move PNG files from Downloads to Archive");
    let j = f.journal();
    j.store(&p).unwrap();
    j.confirm(&p).unwrap();
    j.save_result(
        &p,
        &ExecutionResult {
            status: "executing".into(),
            operations: vec![OperationResult {
                state: OperationState::Attempted,
                post_snapshot: None,
                error: None,
            }],
        },
    )
    .unwrap();
    drop(j);
    let j = f.journal();
    assert_eq!(j.recent().unwrap()[0].status, "executing");
    assert_eq!(
        prepare_undo(&f.root, &j).unwrap_err().code,
        "NOTHING_TO_UNDO"
    );
    assert!(f.path("Downloads/a.png").exists());
}
#[test]
fn doctor_probe_does_not_create_database() {
    let f = Fixture::new();
    let p = f.path("not-created/yaktool.db");
    assert!(Journal::check_readonly(&p)
        .unwrap()
        .contains("Not initialized"));
    assert!(!f.path("not-created").exists());
}
#[test]
fn journal_symlinks_are_rejected() {
    let f = Fixture::new();
    f.file("Documents/untouched", b"untouched");
    std::os::unix::fs::symlink(f.path("Documents/untouched"), f.path("journal/yaktool.db"))
        .unwrap();
    assert!(Journal::open(&f.path("journal/yaktool.db")).is_err());
    assert!(Journal::check_readonly(&f.path("journal/yaktool.db")).is_err());
    assert_eq!(
        std::fs::read(f.path("Documents/untouched")).unwrap(),
        b"untouched"
    );
}
