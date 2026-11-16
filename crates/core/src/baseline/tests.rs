use super::*;
use crate::obs::CellBand;
use crate::testkit::doc_mac;

#[test]
fn given_device_added_when_checked_then_contained_and_counted() {
    let mut b = Baseline::default();
    b.add_device(DeviceKey::Bssid(doc_mac(1)));
    b.add_device(DeviceKey::Cell(CellBand::B3));
    assert_eq!(
        (
            b.contains(&DeviceKey::Bssid(doc_mac(1))),
            b.contains(&DeviceKey::Bssid(doc_mac(2))),
            b.device_count()
        ),
        (true, false, 2)
    );
}

#[test]
fn given_even_rf_samples_when_reference_taken_then_lower_middle_used() {
    let mut b = Baseline::default();
    let s = SensorId::new("a");
    b.add_rf(&s, CellBand::B3, -6000);
    b.add_rf(&s, CellBand::B3, -5800);
    b.add_rf(&s, CellBand::B3, -6200);
    b.add_rf(&s, CellBand::B3, -5900);
    assert_eq!(b.rf_reference(&s, CellBand::B3), Some(-6000));
}

#[test]
fn given_rf_on_other_sensor_when_reference_taken_then_none() {
    let mut b = Baseline::default();
    b.add_rf(&SensorId::new("a"), CellBand::B3, -6000);
    assert_eq!(b.rf_reference(&SensorId::new("b"), CellBand::B3), None);
}
