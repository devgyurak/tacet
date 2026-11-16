use super::*;

const RATE: f64 = 20e6;
const CENTER: f64 = 1_950e6;

fn tone(offset_hz: f64, amplitude: f32, len: usize) -> Vec<Complex<f32>> {
    (0..len)
        .map(|i| {
            let phase = 2.0 * std::f64::consts::PI * offset_hz * i as f64 / RATE;
            Complex::new(
                amplitude * phase.cos() as f32,
                amplitude * phase.sin() as f32,
            )
        })
        .collect()
}

#[test]
fn given_b1_uplink_range_when_planned_then_four_hops_inside_range() {
    let hops = hop_centers(1_920e6, 1_980e6, RATE);
    let inside = hops.iter().all(|c| (1_920e6..=1_980e6).contains(c));
    assert_eq!((hops.len(), inside), (4, true));
}

#[test]
fn given_bad_ranges_when_planned_then_no_hops() {
    assert_eq!(
        (hop_centers(10.0, 5.0, RATE), hop_centers(0.0, 10.0, 0.0)),
        (vec![], vec![])
    );
}

#[test]
fn given_tone_inside_band_when_measured_then_louder_than_same_tone_outside_band() {
    let samples = tone(1e6, 0.5, FFT_SIZE * 4);
    let inside = band_power_cdbfs(&samples, RATE, CENTER, CENTER + 0.5e6, CENTER + 1.5e6);
    let outside = band_power_cdbfs(&samples, RATE, CENTER, CENTER - 5e6, CENTER - 4e6);
    assert_eq!(inside.zip(outside).map(|(i, o)| i - o > 3000), Some(true));
}

#[test]
fn given_louder_tone_when_measured_then_power_rises_about_twenty_db_for_ten_times_amplitude() {
    let quiet = band_power_cdbfs(
        &tone(1e6, 0.05, FFT_SIZE * 2),
        RATE,
        CENTER,
        CENTER,
        CENTER + 2e6,
    );
    let loud = band_power_cdbfs(
        &tone(1e6, 0.5, FFT_SIZE * 2),
        RATE,
        CENTER,
        CENTER,
        CENTER + 2e6,
    );
    assert_eq!(
        quiet
            .zip(loud)
            .map(|(q, l)| (1900..=2100).contains(&(l - q))),
        Some(true)
    );
}

#[test]
fn given_too_few_samples_or_band_out_of_view_when_measured_then_none() {
    let short = band_power_cdbfs(&tone(1e6, 0.5, 10), RATE, CENTER, CENTER, CENTER + 1e6);
    let away = band_power_cdbfs(
        &tone(1e6, 0.5, FFT_SIZE),
        RATE,
        CENTER,
        CENTER + 50e6,
        CENTER + 60e6,
    );
    assert_eq!((short, away), (None, None));
}

#[test]
fn given_hop_powers_when_combined_then_linear_mean_in_cdbfs() {
    assert_eq!(
        (
            combine_cdbfs(&[-5000, -5000]),
            combine_cdbfs(&[-3000, -6000]),
            combine_cdbfs(&[])
        ),
        (Some(-5000), Some(-3301), None)
    );
}

proptest::proptest! {
    #[test]
    fn given_any_samples_when_measured_then_power_is_finite_and_at_most_zero_dbfs(
        values in proptest::collection::vec(-1.0f32..1.0, FFT_SIZE * 2..FFT_SIZE * 3),
    ) {
        let samples: Vec<Complex<f32>> = values.chunks(2).map(|c| Complex::new(c[0], c.get(1).copied().unwrap_or(0.0))).collect();
        let power = band_power_cdbfs(&samples, RATE, CENTER, CENTER - 8e6, CENTER + 8e6);
        proptest::prop_assert!(power.is_none_or(|p| p <= 300));
    }
}
