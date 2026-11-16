use super::*;
use crate::obs::CellBand;
use crate::testkit::{ble, doc_mac, rf, wifi};

fn solo() -> SensorId {
    SensorId::new("solo")
}

fn at(secs: u64) -> Timestamp {
    Timestamp::from_secs(secs)
}

/// 블루투스 기기 `addr`의 광고를 `from_secs`부터 1초 간격으로 넣는다.
fn feed_ble(c: &mut Calibrator, addr: u8, name: Option<&str>, from_secs: u64, rssi: &[i16]) {
    for (i, value) in rssi.iter().enumerate() {
        let builder = ble(addr)
            .vendor(0x004C, &[1])
            .rssi(*value)
            .at_secs(from_secs + i as u64);
        let obs = match name {
            Some(n) => builder.name(n).build(),
            None => builder.build(),
        };
        c.observe(&solo(), &obs);
    }
}

/// 와이파이 `addr`의 비콘을 `from_secs`부터 8초 간격(스캔 간격)으로 넣는다.
fn feed_wifi(c: &mut Calibrator, addr: u8, ssid: &str, from_secs: u64, rssi: &[i16]) {
    for (i, value) in rssi.iter().enumerate() {
        let obs = wifi(doc_mac(addr))
            .ssid(ssid)
            .rssi(*value)
            .at_secs(from_secs + 8 * i as u64)
            .build();
        c.observe(&solo(), &obs);
    }
}

/// 노트북 가까이에서 휴대폰(블루투스 1번 "시험 폰", -40)과 핫스팟(와이파이 9번 "시험 핫스팟", -35), 그리고 멀리 다른 기기를 본 상태.
fn listed() -> Calibrator {
    let mut c = Calibrator::default();
    feed_ble(&mut c, 1, Some("시험 폰"), 0, &[-41, -40, -39]);
    feed_ble(&mut c, 2, None, 0, &[-70, -71, -69]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 0, &[-35, -36]);
    feed_wifi(&mut c, 8, "학교 와이파이", 0, &[-65, -66]);
    c
}

fn ids(list: &[Candidate]) -> Vec<(String, String, i16)> {
    list.iter()
        .map(|c| (c.id.clone(), c.label.clone(), c.rssi))
        .collect()
}

/// 휴대폰·핫스팟을 고르고 교실 끝 단계를 12초에 시작한 상태.
fn measuring() -> Calibrator {
    let mut c = listed();
    let _ = c.select("b1", "w3", at(10));
    let _ = c.start_far(at(12));
    c
}

#[test]
fn given_nearby_devices_when_listed_then_bluetooth_and_wifi_lists_strongest_first_with_names() {
    let p = listed().progress(at(10));
    assert_eq!(
        (ids(&p.bluetooth), ids(&p.wifi)),
        (
            vec![
                ("b1".to_owned(), "시험 폰".to_owned(), -40),
                ("b2".to_owned(), "이름 없는 Apple 기기".to_owned(), -70)
            ],
            vec![
                ("w3".to_owned(), "시험 핫스팟".to_owned(), -36),
                ("w4".to_owned(), "학교 와이파이".to_owned(), -66)
            ]
        )
    );
}

#[test]
fn given_name_seen_once_when_nameless_advertisements_follow_then_name_kept() {
    let mut c = Calibrator::default();
    feed_ble(&mut c, 1, Some("시험 폰"), 0, &[-40]);
    feed_ble(&mut c, 1, None, 1, &[-40, -40]);
    assert_eq!(
        c.progress(at(3)).bluetooth.first().map(|d| d.label.clone()),
        Some("시험 폰".to_owned())
    );
}

#[test]
fn given_device_not_seen_for_twenty_seconds_when_listed_then_dropped_from_list() {
    let mut c = Calibrator::default();
    feed_ble(&mut c, 1, None, 0, &[-40]);
    assert_eq!(
        (
            c.progress(at(20)).bluetooth.len(),
            c.progress(at(21)).bluetooth.len()
        ),
        (1, 0)
    );
}

#[test]
fn given_phone_and_hotspot_picked_when_selected_then_ready_for_edge_with_both_shown() {
    let mut c = listed();
    let result = c.select("b1", "w3", at(10));
    let p = c.progress(at(10));
    assert_eq!(
        (
            result,
            p.stage,
            p.selected_bluetooth.map(|d| d.label),
            p.selected_wifi.map(|d| d.label),
            p.bluetooth.len()
        ),
        (
            Ok(()),
            Stage::ReadyForFar,
            Some("시험 폰".to_owned()),
            Some("시험 핫스팟".to_owned()),
            0
        )
    );
}

#[test]
fn given_only_bluetooth_picked_when_selected_then_not_selected_failure_for_wifi() {
    let mut c = listed();
    let result = c.select("b1", "", at(10));
    let p = c.progress(at(10));
    assert_eq!(
        (result, p.problem_radio),
        (Err(CalibrationProblem::NotSelected), Some(Radio::Wifi))
    );
}

#[test]
fn given_wifi_id_passed_as_bluetooth_when_selected_then_rejected() {
    let mut c = listed();
    assert_eq!(
        c.select("w3", "w3", at(10)),
        Err(CalibrationProblem::NotSelected)
    );
}

#[test]
fn given_picked_device_gone_quiet_when_selected_then_gone_failure() {
    let mut c = listed();
    assert_eq!(c.select("b1", "w3", at(40)), Err(CalibrationProblem::Gone));
}

#[test]
fn given_edge_too_early_when_far_started_before_selection_then_too_early() {
    let mut c = listed();
    assert_eq!(c.start_far(at(10)), Err(CalibrationProblem::TooEarly));
}

#[test]
fn given_both_measured_at_edge_for_twenty_seconds_when_ticked_then_two_boundaries_from_edge_lower_medians()
 {
    let mut c = measuring();
    feed_ble(&mut c, 2, None, 12, &[-50, -50, -50]);
    feed_ble(&mut c, 1, None, 12, &[-79, -77, -78, -76]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 12, &[-62, -60, -61]);
    c.tick(at(32));
    let p = c.progress(at(32));
    assert_eq!(
        (p.stage, p.bluetooth_dbm, p.wifi_dbm, p.problem),
        (Stage::Done, Some(-78), Some(-61), None)
    );
}

#[test]
fn given_wifi_samples_still_short_after_twenty_seconds_when_ticked_then_still_measuring() {
    let mut c = measuring();
    feed_ble(&mut c, 1, None, 12, &[-78, -78, -78]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 12, &[-60, -60]);
    c.tick(at(32));
    let p = c.progress(at(32));
    assert_eq!(
        (
            p.stage,
            p.bluetooth_samples,
            p.wifi_samples,
            p.needed_samples
        ),
        (Stage::Far, 3, 2, 3)
    );
}

#[test]
fn given_hotspot_not_seen_at_edge_for_a_minute_when_ticked_then_not_seen_failure_for_wifi() {
    let mut c = measuring();
    feed_ble(&mut c, 1, None, 12, &[-78, -78, -78]);
    c.tick(at(72));
    let p = c.progress(at(72));
    assert_eq!(
        (p.problem, p.problem_radio),
        (Some(CalibrationProblem::NotSeenAtEdge), Some(Radio::Wifi))
    );
}

#[test]
fn given_phone_moved_only_a_little_when_finished_then_not_farther_for_bluetooth() {
    let mut c = measuring();
    feed_ble(&mut c, 1, None, 12, &[-46, -46, -46]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 12, &[-60, -60, -60]);
    c.tick(at(32));
    let p = c.progress(at(32));
    assert_eq!(
        (p.problem, p.problem_radio),
        (Some(CalibrationProblem::NotFarther), Some(Radio::Bluetooth))
    );
}

#[test]
fn given_edge_exactly_ten_db_weaker_on_both_when_finished_then_boundaries_saved() {
    let mut c = measuring();
    feed_ble(&mut c, 1, None, 12, &[-50, -50, -50]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 12, &[-46, -46, -46]);
    c.tick(at(32));
    assert_eq!(c.progress(at(32)).wifi_dbm, Some(-46));
}

#[test]
fn given_samples_from_before_edge_stage_when_measured_then_ignored() {
    let mut c = measuring();
    feed_ble(&mut c, 1, None, 5, &[-40, -40, -40]);
    feed_ble(&mut c, 1, None, 20, &[-75, -75, -75]);
    feed_wifi(&mut c, 9, "시험 핫스팟", 12, &[-60, -60, -60]);
    c.tick(at(32));
    assert_eq!(c.progress(at(32)).bluetooth_dbm, Some(-75));
}

#[test]
fn given_failed_calibration_when_restarted_then_list_kept_and_selection_cleared() {
    let mut c = listed();
    let _ = c.select("b1", "", at(10));
    c.restart();
    let p = c.progress(at(10));
    assert_eq!(
        (p.stage, p.problem, p.selected_bluetooth, p.bluetooth.len()),
        (Stage::Listing, None, None, 2)
    );
}

#[test]
fn given_rf_observation_when_calibrating_then_not_listed() {
    let mut c = Calibrator::default();
    c.observe(&solo(), &rf(CellBand::B3, -30, 1));
    let p = c.progress(at(2));
    assert_eq!((p.bluetooth.len(), p.wifi.len()), (0, 0));
}

/// 단계마다 쉬운 말 안내가 비어 있는 것.
fn empty_messages(progress: &[CalibrationProgress]) -> usize {
    progress.iter().filter(|p| p.message.is_empty()).count()
}

#[test]
fn given_each_stage_when_progress_read_then_plain_korean_message_present() {
    let mut ready = listed();
    let _ = ready.select("b1", "w3", at(10));
    assert_eq!(
        empty_messages(&[
            listed().progress(at(10)),
            ready.progress(at(10)),
            measuring().progress(at(13))
        ]),
        0
    );
}

#[test]
fn given_device_quiet_past_list_window_when_another_device_seen_then_forgotten_but_selected_device_kept()
 {
    let mut c = listed();
    let _ = c.select("b1", "w3", at(10));
    feed_ble(&mut c, 7, None, 40, &[-60]);
    let p = c.progress(at(40));
    c.restart();
    let after = c.progress(at(40));
    assert_eq!(
        (
            p.selected_bluetooth.map(|d| d.id),
            ids(&after.bluetooth).len()
        ),
        (Some("b1".to_owned()), 1)
    );
}

#[test]
fn given_nameless_advertisement_first_when_named_one_follows_then_name_shown() {
    let mut c = Calibrator::default();
    feed_ble(&mut c, 1, None, 0, &[-40]);
    feed_ble(&mut c, 1, Some("시험 폰"), 1, &[-40]);
    assert_eq!(
        c.progress(at(2)).bluetooth.first().map(|d| d.label.clone()),
        Some("시험 폰".to_owned())
    );
}
