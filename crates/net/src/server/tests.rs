use std::time::Duration;

use tacet_core::ids::SensorPos;
use tacet_core::obs::{CollectorKind, CollectorStatus};
use tacet_core::testkit::{hidden_wifi, report, sensor_info};
use tacet_core::view::BuildingView;
use tokio::io::AsyncWriteExt;

use super::*;
use crate::NetError;
use crate::client::{connect_sensor, connect_viewer};

/// 센서 코드 `code`, 뷰 코드 `V{code}`로 서버를 띄운다.
async fn start(code: &str, max_failures: u32) -> (ServerHandle, mpsc::Receiver<ServerEvent>) {
    start_with(code, max_failures, DEFAULT_LOCKOUT, 1_000).await
}

async fn start_with(
    code: &str,
    max_failures: u32,
    lockout: Duration,
    session_start_epoch_ms: u64,
) -> (ServerHandle, mpsc::Receiver<ServerEvent>) {
    let settings = ServerSettings {
        bind: "127.0.0.1:0".parse().expect("루프백 주소"),
        sensor_code: code.to_owned(),
        viewer_code: format!("V{code}"),
        session_start_epoch_ms,
        max_failures,
        lockout,
    };
    let identity = ServerIdentity::generate().expect("인증서 생성");
    serve(settings, &identity).await.expect("서버 시작")
}

fn epoch_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

async fn next(rx: &mut mpsc::Receiver<ServerEvent>) -> Option<ServerEvent> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .ok()
        .flatten()
}

#[tokio::test]
async fn given_paired_sensor_when_report_sent_then_server_gets_join_and_report_with_connection_sensor_id()
 {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("3-2-앞", "3-2", SensorPos::Front);
    let (mut link, session, fp) =
        connect_sensor(server.local_addr(), "11112222", None, info.clone())
            .await
            .expect("연결");
    let spoofed = report("다른-센서", 1, hidden_wifi(1, -60, 1));
    link.send(&Uplink::Report(spoofed)).await.expect("전송");
    let events = [next(&mut rx).await, next(&mut rx).await];
    assert_eq!(
        (session.start_epoch_ms, fp == server.fingerprint(), events),
        (
            1_000,
            true,
            [
                Some(ServerEvent::SensorJoined(info.clone())),
                Some(ServerEvent::Uplink(
                    info.id.clone(),
                    Uplink::Report(report("3-2-앞", 1, hidden_wifi(1, -60, 1)))
                )),
            ]
        )
    );
}

#[tokio::test]
async fn given_status_uplink_when_sent_then_forwarded() {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("s", "3-1", SensorPos::Back);
    let (mut link, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("연결");
    link.send(&Uplink::Status(
        CollectorKind::Ble,
        CollectorStatus::Running,
    ))
    .await
    .expect("전송");
    next(&mut rx).await;
    assert_eq!(
        next(&mut rx).await,
        Some(ServerEvent::Uplink(
            info.id,
            Uplink::Status(CollectorKind::Ble, CollectorStatus::Running)
        ))
    );
}

#[tokio::test]
async fn given_wrong_code_when_connecting_then_bad_pairing_rejected() {
    let (server, mut rx) = start("11112222", 3).await;
    let result = connect_viewer(server.local_addr(), "V00000000", None).await;
    assert_eq!(
        (
            matches!(result, Err(NetError::Rejected(Rejection::BadPairing))),
            next(&mut rx).await
        ),
        (true, Some(ServerEvent::Rejected(Rejection::BadPairing)))
    );
}

#[tokio::test]
async fn given_failures_reach_limit_when_correct_code_used_then_locked_out() {
    let (server, _rx) = start("11112222", 1).await;
    let _ = connect_viewer(server.local_addr(), "V00000000", None).await;
    let result = connect_viewer(server.local_addr(), "V11112222", None).await;
    assert!(matches!(
        result,
        Err(NetError::Rejected(Rejection::LockedOut))
    ));
}

#[tokio::test]
async fn given_same_sensor_reconnects_when_old_link_still_open_then_new_link_replaces_old() {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("dup", "3-1", SensorPos::Front);
    let (mut old, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("첫 연결");
    next(&mut rx).await;
    let (mut new, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("다시 연결");
    next(&mut rx).await;
    let _ = old
        .send(&Uplink::Report(report("dup", 1, hidden_wifi(1, -60, 1))))
        .await;
    new.send(&Uplink::Report(report("dup", 2, hidden_wifi(2, -60, 1))))
        .await
        .expect("전송");
    assert_eq!(
        next(&mut rx).await,
        Some(ServerEvent::Uplink(
            info.id,
            Uplink::Report(report("dup", 2, hidden_wifi(2, -60, 1)))
        ))
    );
}

#[tokio::test]
async fn given_sensor_code_when_viewer_connects_then_bad_pairing() {
    let (server, _rx) = start("11112222", 3).await;
    let result = connect_viewer(server.local_addr(), "11112222", None).await;
    assert!(matches!(
        result,
        Err(NetError::Rejected(Rejection::BadPairing))
    ));
}

#[tokio::test]
async fn given_lockout_expired_when_correct_code_used_then_accepted() {
    let (server, _rx) = start_with("11112222", 1, Duration::ZERO, 1_000).await;
    let _ = connect_viewer(server.local_addr(), "V00000000", None).await;
    let result = connect_viewer(server.local_addr(), "V11112222", None).await;
    assert!(result.is_ok());
}

/// 시각 어긋남 사건이면 (센서, 앞선 밀리초).
fn clock_ahead(event: ServerEvent) -> Option<(SensorId, u64)> {
    match event {
        ServerEvent::ClockAhead(id, ms) => Some((id, ms)),
        _ => None,
    }
}

#[tokio::test]
async fn given_report_far_ahead_of_server_clock_when_sent_then_dropped_with_clock_event() {
    let (server, mut rx) = start_with("11112222", 3, DEFAULT_LOCKOUT, epoch_ms_now()).await;
    let info = sensor_info("fast", "3-1", SensorPos::Front);
    let (mut link, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("연결");
    next(&mut rx).await;
    link.send(&Uplink::Report(report(
        "fast",
        1,
        hidden_wifi(1, -60, 3_600),
    )))
    .await
    .expect("전송");
    let ahead = next(&mut rx)
        .await
        .and_then(clock_ahead)
        .map(|(id, ms)| (id, ms > 3_000_000));
    assert_eq!(ahead, Some((info.id, true)));
}

#[tokio::test]
async fn given_heartbeat_when_sent_then_not_forwarded() {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("hb", "3-1", SensorPos::Front);
    let (mut link, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("연결");
    next(&mut rx).await;
    link.send(&Uplink::Heartbeat).await.expect("전송");
    link.send(&Uplink::Status(
        CollectorKind::Wifi,
        CollectorStatus::Running,
    ))
    .await
    .expect("전송");
    assert_eq!(
        next(&mut rx).await,
        Some(ServerEvent::Uplink(
            info.id,
            Uplink::Status(CollectorKind::Wifi, CollectorStatus::Running)
        ))
    );
}

#[tokio::test]
async fn given_wrong_pin_when_connecting_then_pin_mismatch() {
    let (server, _rx) = start("11112222", 3).await;
    let wrong = Fingerprint::of_der(b"other server");
    let result = connect_viewer(server.local_addr(), "V11112222", Some(wrong)).await;
    assert!(matches!(result, Err(NetError::PinMismatch)));
}

#[tokio::test]
async fn given_correct_pin_when_connecting_then_accepted() {
    let (server, _rx) = start("11112222", 3).await;
    let result = connect_viewer(server.local_addr(), "V11112222", Some(server.fingerprint())).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn given_viewer_when_view_published_then_viewer_receives_initial_and_new_view() {
    let (server, _rx) = start("11112222", 3).await;
    let (mut viewer, _) = connect_viewer(server.local_addr(), "V11112222", None)
        .await
        .expect("연결");
    let first = viewer.next_view().await.expect("첫 화면");
    server.publish(BuildingView {
        phase: tacet_core::view::phase_view(tacet_core::engine::Phase::Watching, 0),
        ..BuildingView::default()
    });
    let second = viewer
        .next_view()
        .await
        .expect("새 화면")
        .map(|v| v.phase.title);
    assert_eq!(
        (first, second),
        (Some(BuildingView::default()), Some("감시 중".to_owned()))
    );
}

#[tokio::test]
async fn given_sensor_disconnects_when_link_dropped_then_server_reports_left() {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("bye", "3-1", SensorPos::Front);
    let (link, _, _) = connect_sensor(server.local_addr(), "11112222", None, info.clone())
        .await
        .expect("연결");
    next(&mut rx).await;
    drop(link);
    assert_eq!(next(&mut rx).await, Some(ServerEvent::SensorLeft(info.id)));
}

#[tokio::test]
async fn given_raw_tcp_garbage_when_connecting_then_server_keeps_serving() {
    let (server, _rx) = start("11112222", 3).await;
    let mut raw = tokio::net::TcpStream::connect(server.local_addr())
        .await
        .expect("TCP 연결");
    raw.write_all(b"not tls at all\n").await.expect("쓰기");
    drop(raw);
    assert!(
        connect_viewer(server.local_addr(), "V11112222", None)
            .await
            .is_ok()
    );
}

/// 프로토콜을 직접 다루는 시험용 연결(TLS + 첫 메시지를 마음대로).
async fn raw_hello(
    server: &ServerHandle,
    hello: &crate::protocol::ClientHello,
) -> (
    tokio::io::BufReader<
        tokio::io::ReadHalf<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
    >,
    tokio::io::WriteHalf<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
    Option<crate::protocol::ServerHello>,
) {
    let (config, _) = crate::tls::client_config(None).expect("설정");
    let name = rustls::pki_types::ServerName::try_from(crate::tls::SERVER_NAME).expect("이름");
    let tcp = tokio::net::TcpStream::connect(server.local_addr())
        .await
        .expect("TCP");
    let tls = tokio_rustls::TlsConnector::from(config)
        .connect(name, tcp)
        .await
        .expect("TLS");
    let (read_half, mut writer) = tokio::io::split(tls);
    let mut reader = tokio::io::BufReader::new(read_half);
    crate::frame::write_frame(&mut writer, hello)
        .await
        .expect("쓰기");
    let answer = crate::frame::read_frame(&mut reader).await.expect("응답");
    (reader, writer, answer)
}

#[tokio::test]
async fn given_other_protocol_version_when_connecting_then_bad_version_rejected() {
    let (server, _rx) = start("11112222", 3).await;
    let role = crate::protocol::Role::Viewer;
    let proof = crate::pairing::proof("V11112222", &server.fingerprint(), &role.tag());
    let hello = crate::protocol::ClientHello {
        version: crate::protocol::VERSION + 1,
        role,
        proof,
    };
    let (_, _, answer) = raw_hello(&server, &hello).await;
    assert_eq!(
        answer,
        Some(crate::protocol::ServerHello::Rejected(
            Rejection::BadVersion
        ))
    );
}

#[tokio::test]
async fn given_paired_sensor_sending_malformed_line_when_read_then_dropped_and_connection_kept() {
    let (server, mut rx) = start("11112222", 3).await;
    let info = sensor_info("s", "3-2", SensorPos::Front);
    let role = crate::protocol::Role::Sensor(info.clone());
    let proof = crate::pairing::proof("11112222", &server.fingerprint(), &role.tag());
    let hello = crate::protocol::ClientHello {
        version: crate::protocol::VERSION,
        role,
        proof,
    };
    let (_reader, mut writer, _) = raw_hello(&server, &hello).await;
    writer.write_all(b"{\"Report\": 7}\n").await.expect("쓰기");
    crate::frame::write_frame(
        &mut writer,
        &Uplink::Status(CollectorKind::Sdr, CollectorStatus::Running),
    )
    .await
    .expect("쓰기");
    let events = [
        next(&mut rx).await,
        next(&mut rx).await,
        next(&mut rx).await,
    ];
    assert_eq!(
        events,
        [
            Some(ServerEvent::SensorJoined(info.clone())),
            Some(ServerEvent::MalformedDropped(info.id.clone())),
            Some(ServerEvent::Uplink(
                info.id,
                Uplink::Status(CollectorKind::Sdr, CollectorStatus::Running)
            )),
        ]
    );
}
