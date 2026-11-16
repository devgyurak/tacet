use std::io::Cursor;

use super::*;
use crate::config::AppConfig;
use crate::runtime::{Collectors, start_with};

fn standalone() -> Handles {
    let config = AppConfig::parse(
        "mode = \"standalone\"\n[sensor]\nid = \"solo\"\nroom = \"3-2\"\nposition = \"Front\"\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{ id = \"solo\", position = \"Front\" }]",
    )
    .expect("설정");
    start_with(config, Collectors::None).expect("시작")
}

fn viewer() -> Handles {
    let config =
        AppConfig::parse("mode = \"viewer\"\n[remote]\naddress = \"127.0.0.1:9\"\ncode = \"1\"")
            .expect("설정");
    start_with(config, Collectors::None).expect("시작")
}

#[test]
fn given_command_lines_when_parsed_then_known_ones_kept_and_garbage_dropped() {
    assert_eq!(
        [
            parse_command(r#"{"type":"save_report"}"#),
            parse_command(r#" {"type":"open_location_settings"} "#),
            parse_command(r#"{"type":"shutdown"}"#),
            parse_command(r#"{"type":"format_disk"}"#),
            parse_command("아무 글"),
        ],
        [
            Some(Command::SaveReport),
            Some(Command::OpenLocationSettings),
            Some(Command::Shutdown),
            None,
            None
        ]
    );
}

#[test]
fn given_error_message_when_encoded_then_tagged_single_line_json() {
    let line = encode(&Outgoing::Error {
        message: "설정 \"오류\"".to_owned(),
    });
    assert_eq!(line, r#"{"type":"error","message":"설정 \"오류\""}"#);
}

#[test]
fn given_model_when_pumped_twice_without_change_then_sent_once_as_model_json() {
    // 백그라운드 작업이 모델을 바꾸지 않도록 런타임 없이 모델만 만든다.
    let model: Shared = std::sync::Arc::new(std::sync::Mutex::new(UiModel::new(
        crate::config::Mode::Viewer,
    )));
    let mut last = None;
    let mut out = Vec::new();
    let first = pump(&model, &mut last, &mut out).expect("쓰기");
    let second = pump(&model, &mut last, &mut out).expect("쓰기");
    let text = String::from_utf8(out).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap_or_default();
    assert_eq!(
        (
            first,
            second,
            text.lines().count(),
            value["type"].as_str(),
            value["model"]["mode"].as_str()
        ),
        (true, false, 1, Some("model"), Some("viewer"))
    );
}

#[test]
fn given_save_report_line_in_standalone_when_served_then_report_written_and_notice_set() {
    let state = std::env::temp_dir().join(format!("tacet-ipc-{}", std::process::id()));
    let handles = standalone();
    serve_commands(
        Cursor::new("{\"type\":\"save_report\"}\n"),
        &handles,
        &state,
    );
    let notice = handles.model.lock().ok().and_then(|m| m.notice.clone());
    let saved = std::fs::read_dir(state.join("reports"))
        .map(|d| d.count())
        .unwrap_or(0);
    drop(handles);
    let _ = std::fs::remove_dir_all(&state);
    assert_eq!(
        (
            notice.map(|n| n.starts_with("보고서를 저장했습니다")),
            saved
        ),
        (Some(true), 1)
    );
}

#[test]
fn given_viewer_without_hub_when_save_report_handled_then_nothing_happens() {
    assert_eq!(
        handle(Command::SaveReport, &viewer(), &std::env::temp_dir()),
        None
    );
}

#[test]
fn given_shutdown_then_more_commands_when_served_then_stops_at_shutdown() {
    let state = std::env::temp_dir().join(format!("tacet-ipc-stop-{}", std::process::id()));
    let handles = standalone();
    serve_commands(
        Cursor::new("{\"type\":\"shutdown\"}\n{\"type\":\"save_report\"}\n"),
        &handles,
        &state,
    );
    let notice = handles.model.lock().ok().and_then(|m| m.notice.clone());
    let exists = state.join("reports").exists();
    drop(handles);
    let _ = std::fs::remove_dir_all(&state);
    assert_eq!((notice, exists), (None, false));
}

#[test]
fn given_poisoned_model_lock_when_pumped_then_model_still_sent() {
    let model: Shared = std::sync::Arc::new(std::sync::Mutex::new(UiModel::new(
        crate::config::Mode::Viewer,
    )));
    let poisoner = model.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.lock();
        panic!("락을 쥔 채 멈춘 작업(시험)");
    })
    .join();
    let mut out = Vec::new();
    let sent = pump(&model, &mut None, &mut out).expect("쓰기");
    assert_eq!(
        (model.is_poisoned(), sent, out.is_empty()),
        (true, true, false)
    );
}

#[test]
fn given_invalid_utf8_line_then_valid_command_when_served_then_command_still_handled() {
    let state = std::env::temp_dir().join(format!("tacet-ipc-utf8-{}", std::process::id()));
    let handles = standalone();
    let mut input = vec![0xFF, 0xFE, b'\n'];
    input.extend_from_slice(b"{\"type\":\"save_report\"}\n");
    serve_commands(Cursor::new(input), &handles, &state);
    let saved = std::fs::read_dir(state.join("reports"))
        .map(|d| d.count())
        .unwrap_or(0);
    drop(handles);
    let _ = std::fs::remove_dir_all(&state);
    assert_eq!(saved, 1);
}

#[test]
fn given_calibration_command_lines_when_parsed_then_selection_far_and_restart_recognized() {
    assert_eq!(
        [
            parse_command(r#"{"type":"calibrate_select","bluetooth":"b3","wifi":"w1"}"#),
            parse_command(r#"{"type":"calibrate_far"}"#),
            parse_command(r#"{"type":"calibrate_restart"}"#),
            parse_command(r#"{"type":"calibrate_select","bluetooth":"b3"}"#),
        ],
        [
            Some(Command::CalibrateSelect {
                bluetooth: "b3".to_owned(),
                wifi: "w1".to_owned()
            }),
            Some(Command::CalibrateFar),
            Some(Command::CalibrateRestart),
            None
        ]
    );
}
