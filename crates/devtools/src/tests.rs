use std::path::PathBuf;

use tacet_core::report::SessionSalt;
use tacet_core::testkit::{doc_random_mac, hidden_wifi, quick_config, report, single_room, wifi};

use super::*;

fn config() -> ReplayConfig {
    ReplayConfig {
        detector: quick_config(),
        layout: single_room(Some(-80)),
        whitelist: Whitelist::default(),
    }
}

#[test]
fn given_reports_when_written_and_parsed_then_round_trip_and_bad_lines_counted() {
    let r = report("solo", 1, hidden_wifi(1, -60, 20));
    let text = format!("{}\n{{broken\n\n{}\n", obslog_line(&r), obslog_line(&r));
    assert_eq!(parse_obslog(&text), (vec![r.clone(), r], 1));
}

#[test]
fn given_obslog_with_threat_when_replayed_then_hashed_device_and_breakdown_without_raw_ids() {
    let reports = vec![
        report("solo", 1, hidden_wifi(0x2A, -60, 20)),
        report(
            "solo",
            2,
            wifi(doc_random_mac(5))
                .ssid_bytes(&[])
                .rssi(-60)
                .at_secs(21)
                .build(),
        ),
    ];
    let lines = replay(&config(), &reports, &SessionSalt([4; 32]));
    let leaked = lines
        .iter()
        .any(|l| l.summary.contains("5E:00:53") || l.device.contains("53"));
    assert_eq!(
        (
            lines.len(),
            lines[0].at_secs,
            lines[0].summary.starts_with("의심 강도 중간"),
            leaked
        ),
        (2, 20, true, false)
    );
}

#[test]
fn given_paths_when_checked_then_repo_paths_and_wrong_extensions_refused() {
    let in_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("x.obslog");
    let wrong_ext = std::env::temp_dir().join("x.json");
    let ok = std::env::temp_dir().join("x.obslog");
    assert_eq!(
        (
            check_record_path(&in_repo).is_err(),
            check_record_path(&wrong_ext).is_err(),
            check_record_path(&ok).is_ok()
        ),
        (true, true, !inside_git_repo(&ok))
    );
}

#[test]
fn given_relative_path_inside_repo_when_checked_then_detected() {
    assert!(inside_git_repo(std::path::Path::new("target/x.obslog")));
}

#[test]
fn given_replay_toml_when_parsed_then_layout_loaded() {
    let cfg: ReplayConfig =
        toml::from_str("[[layout.rooms]]\nid = \"3-2\"\nsensors = []").expect("설정");
    assert_eq!(cfg.layout.rooms.len(), 1);
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn given_command_lines_when_parsed_then_commands_or_usage() {
    let out = std::env::temp_dir().join("녹화.obslog");
    let out_text = out.to_string_lossy().into_owned();
    assert_eq!(
        (
            parse_command(&args(&["replay", "c.toml", "i.obslog"])),
            parse_command(&args(&["record", "c.toml", &out_text, "30"]))
                .map(|_| ())
                .is_ok()
                || inside_git_repo(&out),
            parse_command(&args(&["record", "c.toml", &out_text, "삼십"])),
            parse_command(&args(&[])),
        ),
        (
            Ok(Command::Replay {
                config: "c.toml".into(),
                input: "i.obslog".into()
            }),
            true,
            Err("초는 정수다".to_owned()),
            Err(USAGE.to_owned()),
        )
    );
}

#[test]
fn given_record_into_repo_when_parsed_then_refused_before_opening_devices() {
    let in_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("x.obslog")
        .to_string_lossy()
        .into_owned();
    assert!(parse_command(&args(&["record", "c.toml", &in_repo, "5"])).is_err());
}

#[test]
fn given_config_and_obslog_texts_when_replay_run_then_lines_and_summary() {
    let obslog = format!(
        "{}\n",
        obslog_line(&report("solo", 1, hidden_wifi(1, -60, 20)))
    );
    let config = "[detector]\nbaseline_ms = 10000\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{ id = \"solo\", position = \"Front\", inside_boundary_dbm = -80 }]";
    let out = run_replay(config, &obslog, &SessionSalt([1; 32])).expect("재생");
    assert_eq!(
        (
            out.lines().count(),
            out.ends_with("보고서 1건, 형식이 틀린 줄 0건")
        ),
        (2, true)
    );
}

#[test]
fn given_bad_config_when_replay_run_then_error_message() {
    assert!(run_replay("[detector", "", &SessionSalt([1; 32])).is_err());
}
