use super::*;
use crate::engine::DeviceLabel;
use crate::ids::{RoomId, SensorId, SensorPos};
use crate::locate::Localization;
use crate::testkit::doc_mac;

fn candidate() -> AlertCandidate {
    AlertCandidate {
        device: DeviceKey::Bssid(doc_mac(1)),
        label: DeviceLabel::Wifi {
            bssid: doc_mac(1),
            hidden: true,
            direct: false,
        },
        level: Level::High,
        score: 10,
        rules: vec![RuleId::R5],
        localization: Some(Localization {
            room: Some(RoomId::new("3-1")),
            position: Some(SensorPos::Front),
            confidence: Confidence::High,
            likely_outside: false,
            needs_calibration: false,
            evidence: vec![(SensorId::new("3-1-앞"), -50)],
        }),
        notify_rooms: vec![],
        penalties: vec![],
        at: Timestamp(5),
    }
}

#[test]
fn given_same_salt_and_key_when_hashed_then_stable_sixteen_hex_digits() {
    let salt = SessionSalt([7; 32]);
    let a = hash_key(&salt, &DeviceKey::Bssid(doc_mac(1)));
    assert_eq!(
        (a.len(), a == hash_key(&salt, &DeviceKey::Bssid(doc_mac(1)))),
        (16, true)
    );
}

#[test]
fn given_different_salts_when_hashed_then_hashes_differ() {
    let key = DeviceKey::Bssid(doc_mac(1));
    assert_ne!(
        hash_key(&SessionSalt([1; 32]), &key),
        hash_key(&SessionSalt([2; 32]), &key)
    );
}

#[test]
fn given_candidate_when_reported_then_no_raw_identifier_or_evidence_kept() {
    let mut report = SessionReport::default();
    report.push(&SessionSalt([3; 32]), &candidate());
    let entry = &report.entries[0];
    let hex_only = entry.device.chars().all(|c| c.is_ascii_hexdigit());
    assert_eq!(
        (
            entry.room.clone(),
            entry.confidence,
            hex_only,
            entry.device == doc_mac(1).to_string()
        ),
        (
            Some(RoomId::new("3-1")),
            Some(Confidence::High),
            true,
            false
        )
    );
}

#[test]
fn given_salt_when_debug_printed_then_bytes_hidden() {
    assert_eq!(format!("{:?}", SessionSalt([9; 32])), "SessionSalt(..)");
}
