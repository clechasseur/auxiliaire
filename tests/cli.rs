use assert_cmd::{cargo_bin, Command};
use test_log::test;

#[test]
fn test_backup_basic() {
    let mut cmd = Command::new(cargo_bin!());

    cmd.arg("backup").arg("--help").assert().success();
}
