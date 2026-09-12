use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use tempfile::TempDir;
use yaktool::{
    filesystem::Root,
    interpret::{Interpreter, RuleInterpreter},
    journal::Journal,
    plan::Plan,
    resolve::resolve,
};

pub struct Fixture {
    pub temp: TempDir,
    pub root: Root,
}
impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}
impl Fixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        for d in [
            "Downloads",
            "Archive",
            "Documents",
            "Pictures",
            "Desktop",
            "journal",
        ] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(temp.path().join(d))
                .unwrap();
        }
        let root = Root::new(temp.path()).unwrap();
        Self { temp, root }
    }
    pub fn path(&self, p: &str) -> PathBuf {
        self.temp.path().join(p)
    }
    pub fn file(&self, p: &str, data: &[u8]) {
        fs::write(self.path(p), data).unwrap();
    }
    pub fn plan(&self, request: &str) -> Plan {
        resolve(
            &self.root,
            &RuleInterpreter.interpret(request).unwrap(),
            chrono::Local::now(),
        )
        .unwrap()
        .plan
        .unwrap()
    }
    pub fn journal(&self) -> Journal {
        Journal::open(&self.path("journal/yaktool.db")).unwrap()
    }
}
