use super::*;
use crate::obs::CellBand;
use crate::testkit::{ble, doc_mac, rf, wifi};

fn sensor(id: &str) -> SensorId {
    SensorId::new(id)
}

#[test]
fn given_non_ble_observations_when_keyed_then_raw_keys() {
    let mut c = Correlator::default();
    let keys = [
        c.canonical(&sensor("a"), &wifi(doc_mac(1)).build(), 30_000, 6),
        c.canonical(&sensor("a"), &rf(CellBand::B3, -50, 0), 30_000, 6),
    ];
    assert_eq!(
        keys,
        [DeviceKey::Bssid(doc_mac(1)), DeviceKey::Cell(CellBand::B3)]
    );
}

#[test]
fn given_rotated_address_with_same_vendor_data_and_close_rssi_when_keyed_then_same_device() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("a"),
        &ble(1).vendor(0x004C, &[9, 9]).rssi(-60).at_secs(1).build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("a"),
        &ble(2).vendor(0x004C, &[9, 9]).rssi(-63).at_secs(5).build(),
        30_000,
        6,
    );
    assert_eq!(first, second);
}

#[test]
fn given_rotated_address_with_far_rssi_on_same_sensor_when_keyed_then_new_device() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("a"),
        &ble(1).vendor(0x004C, &[9]).rssi(-50).at_secs(1).build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("a"),
        &ble(2).vendor(0x004C, &[9]).rssi(-80).at_secs(2).build(),
        30_000,
        6,
    );
    assert_ne!(first, second);
}

#[test]
fn given_same_vendor_data_after_window_when_keyed_then_new_device() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("a"),
        &ble(1).vendor(0x004C, &[9]).at_secs(1).build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("a"),
        &ble(2).vendor(0x004C, &[9]).at_secs(60).build(),
        30_000,
        6,
    );
    assert_ne!(first, second);
}

#[test]
fn given_macos_uuids_on_two_sensors_with_same_vendor_data_when_keyed_then_merged_and_host_local() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("mac-a"),
        &ble(0)
            .host_local(7)
            .vendor(0x01AB, &[1])
            .rssi(-50)
            .at_secs(1)
            .build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("mac-b"),
        &ble(0)
            .host_local(7)
            .vendor(0x01AB, &[1])
            .rssi(-80)
            .at_secs(1)
            .build(),
        30_000,
        6,
    );
    assert_eq!((first == second, first.is_host_local()), (true, true));
}

#[test]
fn given_same_macos_uuid_on_two_sensors_without_vendor_data_when_keyed_then_not_merged() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("mac-a"),
        &ble(0).host_local(7).at_secs(1).build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("mac-b"),
        &ble(0).host_local(7).at_secs(1).build(),
        30_000,
        6,
    );
    assert_ne!(first, second);
}

#[test]
fn given_known_address_when_seen_again_then_same_canonical_and_members_listed() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("a"),
        &ble(1).vendor(0x004C, &[1]).at_secs(1).build(),
        30_000,
        6,
    );
    let again = c.canonical(
        &sensor("a"),
        &ble(1).vendor(0x004C, &[2]).at_secs(2).build(),
        30_000,
        6,
    );
    assert_eq!((first.clone(), c.members_of(&first)), (again, vec![first]));
}

#[test]
fn given_extreme_rssi_on_same_sensor_when_keyed_then_no_overflow_and_new_device() {
    let mut c = Correlator::default();
    let first = c.canonical(
        &sensor("a"),
        &ble(1)
            .vendor(0x004C, &[9])
            .rssi(i16::MAX)
            .at_secs(1)
            .build(),
        30_000,
        6,
    );
    let second = c.canonical(
        &sensor("a"),
        &ble(2)
            .vendor(0x004C, &[9])
            .rssi(i16::MIN)
            .at_secs(2)
            .build(),
        30_000,
        6,
    );
    assert_ne!(first, second);
}

#[test]
fn given_macos_member_when_merged_then_canonical_marked_host_local() {
    let mut c = Correlator::default();
    let canonical = c.canonical(
        &sensor("win"),
        &ble(1).vendor(0x01AB, &[1]).at_secs(1).build(),
        30_000,
        6,
    );
    c.canonical(
        &sensor("mac"),
        &ble(0).host_local(5).vendor(0x01AB, &[1]).at_secs(1).build(),
        30_000,
        6,
    );
    assert_eq!(
        (
            c.has_host_local_member(&canonical),
            canonical.is_host_local()
        ),
        (true, false)
    );
}
