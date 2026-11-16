use tacet_core::config::Whitelist;
use tacet_core::ids::{SensorId, SensorPos};
use tacet_core::obs::{CollectorKind, CollectorStatus};
use tacet_core::testkit::{hidden_wifi, quick_config, report, sensor_info, single_room};
use tacet_net::Rejection;

use super::*;

fn hub() -> Hub {
    Hub::new(
        quick_config(),
        single_room(Some(-80)),
        Whitelist::default(),
        SessionSalt([2; 32]),
    )
}

#[test]
fn given_collector_events_when_applied_then_status_and_alert_reach_hub() {
    let mut h = hub();
    let solo = SensorId::new("solo");
    h.tick(Timestamp::from_secs(11));
    apply_collector_event(
        &mut h,
        &solo,
        CollectorEvent::Status(CollectorKind::Ble, CollectorStatus::Running),
    );
    apply_collector_event(&mut h, &solo, CollectorEvent::Obs(hidden_wifi(1, -60, 20)));
    let view = h.view();
    assert_eq!(
        (
            view.sensors[0]
                .collectors
                .iter()
                .map(|c| c.state.clone())
                .collect::<Vec<_>>(),
            view.alerts.len()
        ),
        (vec!["정상 작동 중".to_owned()], 1)
    );
}

#[test]
fn given_server_events_when_applied_then_log_lines_and_hub_updates() {
    let mut h = hub();
    h.tick(Timestamp::from_secs(11));
    let info = sensor_info("solo", "3-2", SensorPos::Front);
    let lines = [
        apply_server_event(&mut h, ServerEvent::SensorJoined(info.clone())),
        apply_server_event(
            &mut h,
            ServerEvent::Uplink(
                info.id.clone(),
                Uplink::Report(report("solo", 1, hidden_wifi(1, -60, 20))),
            ),
        ),
        apply_server_event(
            &mut h,
            ServerEvent::Uplink(
                info.id.clone(),
                Uplink::Status(CollectorKind::Wifi, CollectorStatus::Running),
            ),
        ),
        apply_server_event(&mut h, ServerEvent::MalformedDropped(info.id.clone())),
        apply_server_event(&mut h, ServerEvent::Rejected(Rejection::BadPairing)),
        apply_server_event(&mut h, ServerEvent::SensorLeft(info.id.clone())),
        apply_server_event(&mut h, ServerEvent::ClockAhead(info.id.clone(), 12_000)),
        apply_server_event(
            &mut h,
            ServerEvent::Uplink(info.id.clone(), Uplink::Heartbeat),
        ),
    ];
    // 보고서는 재정렬 버퍼에 있다가 지연 시간이 지난 틱에서 판정된다.
    h.tick(Timestamp::from_secs(30));
    assert_eq!(
        (
            lines.iter().filter(|l| l.is_some()).count(),
            h.view().alerts.len(),
            h.view().sensors[0].connected
        ),
        (5, 1, false)
    );
}

#[test]
fn given_salts_when_made_twice_then_different() {
    assert_ne!(new_salt(), new_salt());
}

#[test]
fn given_identity_file_when_loaded_twice_then_same_fingerprint() {
    let dir = std::env::temp_dir().join(format!("tacet-test-{}", now_epoch_ms()));
    let path = dir.join("server-identity.json");
    let first = load_or_create_identity(&path)
        .expect("만들기")
        .fingerprint();
    let second = load_or_create_identity(&path)
        .expect("다시 읽기")
        .fingerprint();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(first, second);
}

#[test]
fn given_broken_identity_file_when_loaded_then_error() {
    let dir = std::env::temp_dir().join(format!("tacet-broken-{}", now_epoch_ms()));
    std::fs::create_dir_all(&dir).expect("폴더");
    let path = dir.join("server-identity.json");
    std::fs::write(&path, b"{\"cert\":\"abc\",\"key\":\"zz\"}").expect("쓰기");
    let result = load_or_create_identity(&path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(result.is_err());
}

#[test]
fn given_hub_with_alert_when_report_saved_then_json_has_hashed_device_only() {
    let mut h = hub();
    h.tick(Timestamp::from_secs(11));
    h.ingest(&report("solo", 1, hidden_wifi(0x2A, -60, 20)));
    let dir = std::env::temp_dir().join(format!("tacet-report-{}", now_epoch_ms()));
    let path = save_report(&h, &dir).expect("저장");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        (text.contains("\"device\""), text.contains("00·53·2A")),
        (true, false)
    );
}

#[test]
fn given_layouts_when_checked_then_empty_detected() {
    assert_eq!(
        (
            layout_is_empty(&RoomLayout::default()),
            layout_is_empty(&single_room(None))
        ),
        (true, false)
    );
}

#[test]
fn given_now_when_read_then_after_2020() {
    assert!(now_epoch_ms() > 1_577_836_800_000);
}

#[tokio::test]
async fn given_explicit_address_when_resolved_then_returned_without_discovery() {
    let addr: std::net::SocketAddr = "127.0.0.1:47100".parse().expect("주소");
    assert_eq!(resolve_server(Some(addr)).await, Some(addr));
}

#[test]
fn given_viewer_config_when_started_then_model_shows_connecting_state() {
    let config = AppConfig::parse(
        "mode = \"viewer\"\n[remote]\naddress = \"127.0.0.1:9\"\ncode = \"12345678\"",
    )
    .expect("설정");
    let handles = start_with(config, Collectors::None).expect("시작");
    std::thread::sleep(Duration::from_millis(300));
    let connection = handles
        .model
        .lock()
        .map(|m| m.connection.clone())
        .unwrap_or_default();
    assert_eq!(
        connection.map(|c| c.text.starts_with("시험본부 서버")),
        Some(true)
    );
}

#[test]
fn given_standalone_config_when_started_then_hub_and_baseline_phase() {
    let config = AppConfig::parse(
        "mode = \"standalone\"\n[sensor]\nid = \"solo\"\nroom = \"3-2\"\nposition = \"Front\"\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{ id = \"solo\", position = \"Front\" }]",
    )
    .expect("설정");
    let handles = start_with(config, Collectors::None).expect("시작");
    std::thread::sleep(Duration::from_millis(1500));
    let phase = handles
        .model
        .lock()
        .map(|m| m.view.phase.clone())
        .unwrap_or_default();
    assert_eq!(
        (handles.hub.is_some(), phase.title.starts_with("준비 중")),
        (true, true)
    );
}

fn temp_state(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "tacet-{tag}-{}-{}",
        std::process::id(),
        now_epoch_ms()
    ))
}

fn header_value(handles: &Handles, prefix: &str) -> Option<String> {
    let header = handles
        .model
        .lock()
        .map(|m| m.header.clone())
        .unwrap_or_default();
    header.iter().find_map(|h| {
        h.strip_prefix(prefix).map(|rest| {
            rest.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned()
        })
    })
}

fn wait_for(handles: &Handles, pred: impl Fn(&UiModel) -> bool) -> bool {
    (0..100).any(|_| {
        std::thread::sleep(Duration::from_millis(100));
        handles.model.lock().map(|m| pred(&m)).unwrap_or(false)
    })
}

#[test]
fn given_server_sensor_and_viewer_on_loopback_when_paired_then_sensor_joins_and_viewer_sees_server_view()
 {
    let state = temp_state("e2e");
    let server_config = AppConfig::parse(&format!(
        "mode = \"server\"\nstate_dir = {:?}\n[server]\nbind = \"127.0.0.1:0\"\ndashboard_bind = \"127.0.0.1:0\"\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{{ id = \"s\", position = \"Back\" }}]",
        state.join("server")
    ))
    .expect("서버 설정");
    let server = start_with(server_config, Collectors::None).expect("서버 시작");
    let code = header_value(&server, "센서 페어링 코드: ").unwrap_or_default();
    let viewer_code = header_value(&server, "감독관 뷰 페어링 코드: ").unwrap_or_default();
    let addr = header_value(&server, "서버 주소: ").unwrap_or_default();
    let sensor_config = AppConfig::parse(&format!(
        "mode = \"sensor\"\nstate_dir = {:?}\n[sensor]\nid = \"s\"\nroom = \"3-2\"\nposition = \"Back\"\n[remote]\naddress = \"{addr}\"\ncode = \"{code}\"",
        state.join("sensor")
    ))
    .expect("센서 설정");
    let sensor = start_with(sensor_config, Collectors::None).expect("센서 시작");
    let joined = wait_for(&server, |m| {
        m.view.sensors.iter().any(|s| s.sensor.0 == "s")
    });
    let viewer_config = AppConfig::parse(&format!(
        "mode = \"viewer\"\nstate_dir = {:?}\n[remote]\naddress = \"{addr}\"\ncode = \"{viewer_code}\"",
        state.join("viewer")
    ))
    .expect("뷰 설정");
    let viewer = start_with(viewer_config, Collectors::None).expect("뷰 시작");
    let viewer_sees = wait_for(&viewer, |m| m.view.phase.title.starts_with("준비 중"));
    let pinned = state.join("sensor").join("server-pin.txt").exists();
    drop((sensor, viewer, server));
    let _ = std::fs::remove_dir_all(&state);
    assert_eq!((joined, viewer_sees, pinned), (true, true, true));
}

#[test]
fn given_server_config_on_loopback_when_started_then_header_shows_codes_and_sensor_can_pair() {
    let state = temp_state("server");
    let config = AppConfig::parse(&format!(
        "mode = \"server\"\nstate_dir = {:?}\n[server]\nbind = \"127.0.0.1:0\"\ndashboard_bind = \"127.0.0.1:0\"",
        state
    ))
    .expect("설정");
    let handles = start_with(config, Collectors::None).expect("시작");
    let header = handles
        .model
        .lock()
        .map(|m| m.header.clone())
        .unwrap_or_default();
    assert_eq!(
        (
            header.first().map(|h| h.starts_with("센서 페어링 코드: ")),
            header.len(),
            handles.hub.is_some()
        ),
        (Some(true), 6, true)
    );
    drop(handles);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn given_sensor_config_without_server_when_started_then_connection_attempt_reported() {
    let config = AppConfig::parse(
        "mode = \"sensor\"\n[sensor]\nid = \"s\"\nroom = \"3-2\"\nposition = \"Back\"\n[remote]\naddress = \"127.0.0.1:9\"\ncode = \"1\"",
    )
    .expect("설정");
    let handles = start_with(config, Collectors::None).expect("시작");
    std::thread::sleep(Duration::from_millis(500));
    let connection = handles
        .model
        .lock()
        .map(|m| m.connection.clone())
        .unwrap_or_default();
    assert_eq!(
        connection.map(|c| c.text.starts_with("시험본부 서버")),
        Some(true)
    );
}

#[test]
fn given_sensor_replaying_hidden_wifi_when_server_has_no_baseline_then_viewer_sees_alert_end_to_end()
 {
    let state = temp_state("alert");
    let server_config = AppConfig::parse(&format!(
        "mode = \"server\"\nstate_dir = {:?}\n[server]\nbind = \"127.0.0.1:0\"\ndashboard_bind = \"127.0.0.1:0\"\n[detector]\nbaseline_ms = 0\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{{ id = \"s\", position = \"Back\", inside_boundary_dbm = -80, wifi_inside_boundary_dbm = -80 }}]",
        state.join("server")
    ))
    .expect("서버 설정");
    let server = start_with(server_config, Collectors::None).expect("서버 시작");
    let code = header_value(&server, "센서 페어링 코드: ").unwrap_or_default();
    let viewer_code = header_value(&server, "감독관 뷰 페어링 코드: ").unwrap_or_default();
    let addr = header_value(&server, "서버 주소: ").unwrap_or_default();
    let sensor_config = AppConfig::parse(&format!(
        "mode = \"sensor\"\nstate_dir = {:?}\n[sensor]\nid = \"s\"\nroom = \"3-2\"\nposition = \"Back\"\n[remote]\naddress = \"{addr}\"\ncode = \"{code}\"",
        state.join("sensor")
    ))
    .expect("센서 설정");
    let events = vec![
        CollectorEvent::Status(CollectorKind::Ble, CollectorStatus::Running),
        CollectorEvent::Obs(hidden_wifi(0x2A, -60, 1)),
    ];
    let sensor = start_with(sensor_config, Collectors::Replay(events)).expect("센서 시작");
    let viewer_config = AppConfig::parse(&format!(
        "mode = \"viewer\"\nstate_dir = {:?}\n[remote]\naddress = \"{addr}\"\ncode = \"{viewer_code}\"",
        state.join("viewer")
    ))
    .expect("뷰 설정");
    let viewer = start_with(viewer_config, Collectors::None).expect("뷰 시작");
    let alerted = wait_for(&viewer, |m| {
        m.view.alerts.iter().any(|a| {
            a.text.starts_with("[확인 필요] 숨긴 Wi-Fi 감지") && a.explain.place == "3-2 교실"
        })
    });
    let sensor_status = wait_for(&sensor, |m| {
        m.view.sensors.iter().any(|s| {
            s.collectors
                .iter()
                .map(|c| c.state.as_str())
                .eq(["정상 작동 중"])
        })
    });
    drop((sensor, viewer, server));
    let _ = std::fs::remove_dir_all(&state);
    assert_eq!((alerted, sensor_status), (true, true));
}

#[test]
fn given_standalone_replaying_hidden_hotspot_when_baseline_is_zero_then_local_alert_shown() {
    let config = AppConfig::parse(
        "mode = \"standalone\"\n[sensor]\nid = \"solo\"\nroom = \"3-2\"\nposition = \"Front\"\n[detector]\nbaseline_ms = 0\n[[layout.rooms]]\nid = \"3-2\"\nsensors = [{ id = \"solo\", position = \"Front\", inside_boundary_dbm = -80 }]",
    )
    .expect("설정");
    let events = vec![CollectorEvent::Obs(
        tacet_core::testkit::wifi(tacet_core::testkit::doc_random_mac(4))
            .ssid_bytes(&[])
            .rssi(-60)
            .at_secs(1)
            .build(),
    )];
    let handles = start_with(config, Collectors::Replay(events)).expect("시작");
    assert!(wait_for(&handles, |m| m
        .view
        .alerts
        .iter()
        .any(|a| a.text.starts_with("[확인 필요] 숨긴 Wi-Fi 감지"))));
}

/// 사건들을 넣어 둔 수신기.
fn queued(events: &[CollectorEvent]) -> mpsc::Receiver<CollectorEvent> {
    let (tx, rx) = mpsc::channel(events.len().max(1));
    for event in events {
        let _ = tx.try_send(event.clone());
    }
    rx
}

#[test]
fn given_events_queued_while_offline_when_drained_then_observations_counted_and_statuses_kept() {
    let mut rx = queued(&[
        CollectorEvent::Obs(hidden_wifi(1, -60, 1)),
        CollectorEvent::Status(CollectorKind::Ble, CollectorStatus::Running),
        CollectorEvent::Obs(hidden_wifi(2, -60, 2)),
    ]);
    let mut statuses = std::collections::BTreeMap::new();
    let dropped = drain_offline(&mut rx, &mut statuses);
    assert_eq!(
        (
            dropped,
            statuses.into_iter().collect::<Vec<_>>(),
            offline_text("서버를 찾는 중", dropped)
        ),
        (
            2,
            vec![(CollectorKind::Ble, CollectorStatus::Running)],
            "서버를 찾는 중 — 연결되지 않은 동안 관측 2건을 버렸다(그동안은 탐지 공백)".to_owned()
        )
    );
}

#[cfg(unix)]
#[test]
fn given_private_file_when_written_then_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_state("private");
    std::fs::create_dir_all(&dir).expect("폴더");
    let path = dir.join("key.json");
    write_private(&path, b"{}").expect("쓰기");
    let mode = std::fs::metadata(&path)
        .map(|m| m.permissions().mode() & 0o777)
        .ok();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(mode, Some(0o600));
}

#[test]
fn given_state_dir_that_is_a_file_when_pin_saved_then_warning_returned() {
    let dir = temp_state("pinfail");
    std::fs::create_dir_all(&dir).expect("폴더");
    let blocker = dir.join("not-a-dir");
    std::fs::write(&blocker, b"x").expect("파일");
    let note = save_pin(&blocker, &Fingerprint::of_der(b"synthetic"));
    let ok = save_pin(&dir, &Fingerprint::of_der(b"synthetic"));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        (note.contains("지문을 저장하지 못했다"), ok),
        (true, String::new())
    );
}
