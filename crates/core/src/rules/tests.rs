use super::*;
use crate::ids::Mac;
use crate::obs::{Band, CellBand};
use crate::testkit::{ble, doc_mac, doc_random_mac, rf, rules_for, vendors_missing_rule, wifi};

#[test]
fn given_each_phone_vendor_id_when_scored_then_no_vendor_misses_r1() {
    assert_eq!(
        vendors_missing_rule(&PHONE_VENDOR_IDS, RuleId::R1),
        Vec::<u16>::new()
    );
}

#[test]
fn given_each_glasses_vendor_id_when_scored_then_no_vendor_misses_r1a() {
    assert_eq!(
        vendors_missing_rule(&GLASSES_VENDOR_IDS, RuleId::R1a),
        Vec::<u16>::new()
    );
}

#[test]
fn given_oculus_service_uuid_when_scored_then_r1a_hits() {
    assert_eq!(
        rules_for(&ble(1).service(OCULUS_SERVICE).build(), None),
        vec![RuleId::R1a]
    );
}

#[test]
fn given_fast_pair_service_data_when_scored_then_only_r1b_hits() {
    assert_eq!(
        rules_for(
            &ble(1).service_data(FAST_PAIR_SERVICE, &[0, 1, 2]).build(),
            None
        ),
        vec![RuleId::R1b]
    );
}

#[test]
fn given_unknown_vendor_without_calibration_when_scored_then_no_rule_hits() {
    assert_eq!(
        rules_for(&ble(1).vendor(0x1234, &[1]).build(), None),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_rssi_equal_to_boundary_when_scored_then_r2_hits() {
    assert_eq!(
        rules_for(&ble(1).rssi(-75).build(), Some(-75)),
        vec![RuleId::R2]
    );
}

#[test]
fn given_rssi_one_below_boundary_when_scored_then_r2_misses() {
    assert_eq!(
        rules_for(&ble(1).rssi(-76).build(), Some(-75)),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_hidden_ssid_random_bssid_inside_boundary_when_scored_then_r3_r4_r5_hit() {
    let obs = wifi(doc_random_mac(7)).ssid_bytes(&[]).rssi(-50).build();
    assert_eq!(
        rules_for(&obs, Some(-75)),
        vec![RuleId::R3, RuleId::R4, RuleId::R5]
    );
}

#[test]
fn given_nul_filled_ssid_when_scored_then_r5_hits() {
    assert_eq!(
        rules_for(&wifi(doc_mac(7)).ssid_bytes(&[0; 8]).build(), None),
        vec![RuleId::R5]
    );
}

#[test]
fn given_direct_prefix_ssid_when_scored_then_r6_hits() {
    assert_eq!(
        rules_for(&wifi(doc_mac(7)).ssid("DIRECT-xy-Phone").build(), None),
        vec![RuleId::R6]
    );
}

#[test]
fn given_lowercase_direct_prefix_when_scored_then_r6_misses() {
    assert_eq!(
        rules_for(&wifi(doc_mac(7)).ssid("direct-xy").build(), None),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_multicast_locally_administered_bssid_when_scored_then_r4_misses() {
    let obs = wifi(Mac([0x03, 0x00, 0x5E, 0x90, 0x10, 0x01])).build();
    assert_eq!(rules_for(&obs, None), Vec::<RuleId>::new());
}

#[test]
fn given_6ghz_hidden_network_when_scored_then_r5_hits() {
    let obs = wifi(doc_mac(9))
        .ssid_bytes(&[])
        .channel(Band::B6, 37)
        .build();
    assert_eq!(rules_for(&obs, None), vec![RuleId::R5]);
}

#[test]
fn given_baseline_device_when_scored_then_no_rule_hits() {
    let ctx = RuleContext {
        in_baseline: true,
        inside_boundary_dbm: Some(-75),
        rf_reference_cdbm: None,
        rf_burst_cdb: 1000,
    };
    assert_eq!(
        evaluate(&ble(1).vendor(0x004C, &[1]).rssi(-40).build(), &ctx),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_rf_power_at_reference_plus_burst_when_scored_then_r7_hits() {
    let ctx = RuleContext {
        in_baseline: true,
        inside_boundary_dbm: None,
        rf_reference_cdbm: Some(-6000),
        rf_burst_cdb: 1000,
    };
    assert_eq!(evaluate(&rf(CellBand::B3, -50, 0), &ctx), vec![RuleId::R7]);
}

#[test]
fn given_rf_power_below_burst_when_scored_then_r7_misses() {
    let ctx = RuleContext {
        in_baseline: false,
        inside_boundary_dbm: None,
        rf_reference_cdbm: Some(-6000),
        rf_burst_cdb: 1000,
    };
    assert_eq!(
        evaluate(&rf(CellBand::B3, -51, 0), &ctx),
        Vec::<RuleId>::new()
    );
}

#[test]
fn given_rf_without_reference_when_scored_then_r7_misses() {
    assert_eq!(
        rules_for(&rf(CellBand::B1, 0, 0), None),
        Vec::<RuleId>::new()
    );
}
