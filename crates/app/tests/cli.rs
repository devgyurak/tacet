//! 앱 바이너리의 설정 오류 경로. 창이나 장비를 열지 않는다.

use std::process::Command;

fn run(args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_tacet"))
        .args(args)
        .output()
        .expect("바이너리 실행");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn given_missing_config_file_when_app_started_then_fails_with_korean_message() {
    let missing = std::env::temp_dir().join("없는-설정-파일.toml");
    let (ok, stderr) = run(&["--config", &missing.to_string_lossy()]);
    assert_eq!(
        (ok, stderr.contains("설정 파일을 읽지 못했다")),
        (false, true)
    );
}

#[test]
fn given_invalid_config_file_when_app_started_then_fails_before_opening_window() {
    let path = std::env::temp_dir().join(format!("tacet-bad-{}.toml", std::process::id()));
    std::fs::write(&path, "mode = \"server\"\n").expect("쓰기");
    let (ok, stderr) = run(&["--config", &path.to_string_lossy()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        (ok, stderr.contains("설정 파일이 올바르지 않다")),
        (false, true)
    );
}
