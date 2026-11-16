//! devtools 바이너리. 녹화(장비 사용)는 저장소 안 경로 거부만 확인한다.

use std::path::PathBuf;
use std::process::Command;

use tacet_core::testkit::{doc_mac, report, wifi};
use tacet_devtools::obslog_line;

fn run(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_tacet-devtools"))
        .args(args)
        .output()
        .expect("바이너리 실행");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn temp(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tacet-cli-{}-{name}", std::process::id()));
    std::fs::write(&path, text).expect("쓰기");
    path
}

#[test]
fn given_obslog_when_replay_run_then_summary_printed_without_raw_ids() {
    let obslog = temp(
        "in.obslog",
        &format!(
            "{}\n",
            obslog_line(&report(
                "solo",
                1,
                wifi(doc_mac(0x2A))
                    .ssid_bytes(&[])
                    .rssi(-60)
                    .at_secs(20)
                    .build(),
            ))
        ),
    );
    let config = temp(
        "c.toml",
        "[detector]\nbaseline_ms = 10000\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{ id = \"solo\", position = \"Front\", inside_boundary_dbm = -80, wifi_inside_boundary_dbm = -80 }]",
    );
    let replay = run(&[
        "replay",
        &config.to_string_lossy(),
        &obslog.to_string_lossy(),
    ]);
    let _ = [obslog, config].iter().map(std::fs::remove_file).count();
    assert_eq!(
        (
            replay.0,
            replay.1.contains("의심 강도 중간"),
            replay.1.contains("5E:00:53"),
        ),
        (true, true, false)
    );
}

#[test]
fn given_record_into_repository_when_run_then_refused_with_reason() {
    let inside = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("녹화.obslog");
    let (ok, _, stderr) = run(&["record", "c.toml", &inside.to_string_lossy(), "1"]);
    assert_eq!((ok, stderr.contains("저장소 밖에 쓴다")), (false, true));
}

#[test]
fn given_no_arguments_when_run_then_usage_shown() {
    let (ok, _, stderr) = run(&[]);
    assert_eq!((ok, stderr.contains("사용법")), (false, true));
}
