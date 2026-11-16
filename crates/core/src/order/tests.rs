use super::*;
use crate::obs::CellBand;
use crate::testkit::{report, rf};

fn seqs(reports: &[SensorReport]) -> Vec<(String, u64)> {
    reports
        .iter()
        .map(|r| (r.sensor.0.clone(), r.seq))
        .collect()
}

#[test]
fn given_reports_arriving_out_of_order_when_drained_then_time_sensor_seq_order() {
    let mut buffer = ReorderBuffer::new(2_000);
    buffer.push(report("b", 1, rf(CellBand::B3, -50, 5)));
    buffer.push(report("a", 2, rf(CellBand::B3, -50, 5)));
    buffer.push(report("a", 1, rf(CellBand::B3, -50, 5)));
    buffer.push(report("c", 1, rf(CellBand::B3, -50, 4)));
    assert_eq!(
        seqs(&buffer.ready(Timestamp::from_secs(7))),
        vec![
            ("c".into(), 1),
            ("a".into(), 1),
            ("a".into(), 2),
            ("b".into(), 1)
        ]
    );
}

#[test]
fn given_recent_reports_when_drained_then_held_until_delay_passes() {
    let mut buffer = ReorderBuffer::new(2_000);
    buffer.push(report("a", 1, rf(CellBand::B3, -50, 5)));
    let early = buffer.ready(Timestamp::from_secs(6));
    let held = buffer.len();
    let later = buffer.ready(Timestamp::from_secs(7));
    assert_eq!(
        (early.len(), held, later.len(), buffer.is_empty()),
        (0, 1, 1, true)
    );
}

#[test]
fn given_duplicate_report_when_pushed_twice_then_drained_once() {
    let mut buffer = ReorderBuffer::new(0);
    buffer.push(report("a", 1, rf(CellBand::B3, -50, 1)));
    buffer.push(report("a", 1, rf(CellBand::B3, -50, 1)));
    assert_eq!(buffer.ready(Timestamp::from_secs(9)).len(), 1);
}

/// 같은 보고서 셋을 `order` 순서로 넣고 꺼낸 순서.
fn drain_in(order: &[usize]) -> Vec<(String, u64)> {
    let all = [
        report("x", 1, rf(CellBand::B3, -50, 3)),
        report("y", 1, rf(CellBand::B3, -50, 3)),
        report("x", 2, rf(CellBand::B3, -50, 2)),
    ];
    let mut buffer = ReorderBuffer::new(0);
    for i in order {
        buffer.push(all[*i].clone());
    }
    seqs(&buffer.ready(Timestamp::from_secs(10)))
}

#[test]
fn given_two_arrival_orders_when_drained_then_same_sequence() {
    assert_eq!(drain_in(&[0, 1, 2]), drain_in(&[2, 1, 0]));
}
