use assert_cmd::{crate_name, Command};

#[test]
fn test_backup_basic() {
    let mut cmd = Command::cargo_bin(crate_name!()).unwrap();

    cmd.arg("backup").arg("--help").assert().success();
}
