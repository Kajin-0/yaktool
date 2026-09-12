pub mod common;
use common::Fixture;
use yaktool::{
    intent::Action,
    interpret::{Interpreter, RuleInterpreter},
    policy::{decision, limits, PolicyDecision, MAX_BYTES},
    resolve::resolve,
};
#[test]
fn file_limits_zero_one_hundred_hundred_one() {
    for n in [0, 1, 100, 101] {
        let f = Fixture::new();
        for i in 0..n {
            f.file(&format!("Downloads/{i:03}.png"), b"x");
        }
        let r = resolve(
            &f.root,
            &RuleInterpreter
                .interpret("move PNG files from Downloads to Archive")
                .unwrap(),
            chrono::Local::now(),
        );
        if n <= 100 {
            assert_eq!(r.unwrap().plan.unwrap().data().operations.len(), n);
        } else {
            assert_eq!(r.err().unwrap().code, "FILE_LIMIT_EXCEEDED");
        }
        assert_eq!(std::fs::read_dir(f.path("Downloads")).unwrap().count(), n);
        assert_eq!(std::fs::read_dir(f.path("Archive")).unwrap().count(), 0);
    }
}
#[test]
fn sparse_byte_limits() {
    for n in [MAX_BYTES - 1, MAX_BYTES, MAX_BYTES + 1] {
        let f = Fixture::new();
        std::fs::File::create(f.path("Downloads/big.png"))
            .unwrap()
            .set_len(n)
            .unwrap();
        let r = resolve(
            &f.root,
            &RuleInterpreter
                .interpret("move PNG files from Downloads to Archive")
                .unwrap(),
            chrono::Local::now(),
        );
        if n <= MAX_BYTES {
            assert_eq!(r.unwrap().plan.unwrap().data().total_bytes, n);
        } else {
            assert_eq!(r.err().unwrap().code, "BYTE_LIMIT_EXCEEDED");
        }
        assert!(f.path("Downloads/big.png").exists());
        assert!(!f.path("Archive/big.png").exists());
    }
}
#[test]
fn static_policy() {
    assert_eq!(decision(&Action::Move), PolicyDecision::Confirm);
    assert_eq!(decision(&Action::List), PolicyDecision::Allow);
    assert_eq!(decision(&Action::Unsupported), PolicyDecision::Deny);
    assert!(limits(100, MAX_BYTES).is_ok());
    assert!(limits(101, 0).is_err());
}
