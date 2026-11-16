use super::*;

#[test]
fn given_seconds_when_converted_then_milliseconds() {
    assert_eq!(Timestamp::from_secs(3), Timestamp(3000));
}

#[test]
fn given_two_times_when_elapsed_taken_then_saturates_at_zero_when_reversed() {
    assert_eq!(
        (
            Timestamp(5000).since(Timestamp(2000)),
            Timestamp(1000).since(Timestamp(2000))
        ),
        (3000, 0)
    );
}

#[test]
fn given_time_when_milliseconds_added_then_saturates_at_max() {
    assert_eq!(
        (Timestamp(1).plus_ms(2), Timestamp(u64::MAX).plus_ms(1)),
        (Timestamp(3), Timestamp(u64::MAX))
    );
}
