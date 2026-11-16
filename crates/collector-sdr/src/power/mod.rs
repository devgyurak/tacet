//! 대역 전력 계산(순수 함수). 모든 OS에서 합성 IQ로 테스트한다(AGENTS.md §2 규칙 6).

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

#[cfg(test)]
mod tests;

/// FFT 크기.
pub const FFT_SIZE: usize = 1024;

/// 수신 대역폭 가운데 가장자리 롤오프를 빼고 쓰는 비율.
const USABLE_FRACTION: f64 = 0.8;

/// `[lo, hi]`(Hz)를 덮는 튜닝 중심 주파수들. 표본률의 80%를 한 번에 본다.
pub fn hop_centers(lo_hz: f64, hi_hz: f64, sample_rate_hz: f64) -> Vec<f64> {
    let usable = sample_rate_hz * USABLE_FRACTION;
    // NaN도 걸러지도록 "양수가 아님"을 partial_cmp로 쓴다.
    let positive = |x: f64| x.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
    if !positive(usable) || !positive(hi_hz - lo_hz) {
        return Vec::new();
    }
    let hops = ((hi_hz - lo_hz) / usable).ceil().max(1.0);
    let step = (hi_hz - lo_hz) / hops;
    (0..hops as usize)
        .map(|i| lo_hz + step * (i as f64 + 0.5))
        .collect()
}

/// IQ 표본에서 `[lo, hi]`(Hz) 안 FFT 빈의 평균 전력(0.01dBFS 단위 정수). 해닝 창, 프레임 평균.
/// 대역이 수신 범위 밖이거나 표본이 한 프레임도 안 되면 `None`.
pub fn band_power_cdbfs(
    samples: &[Complex<f32>],
    sample_rate_hz: f64,
    center_hz: f64,
    lo_hz: f64,
    hi_hz: f64,
) -> Option<i32> {
    let n = FFT_SIZE;
    let window: Vec<f32> = (0..n)
        .map(|i| {
            let x = (2.0 * std::f64::consts::PI * i as f64 / (n as f64 - 1.0)).cos();
            (0.5 - 0.5 * x) as f32
        })
        .collect();
    let window_power: f32 = window.iter().map(|w| w * w).sum();
    let bins: Vec<usize> = (0..n)
        .filter(|&k| {
            let signed = if k < n / 2 {
                k as f64
            } else {
                k as f64 - n as f64
            };
            let freq = center_hz + signed * sample_rate_hz / n as f64;
            (lo_hz..=hi_hz).contains(&freq)
        })
        .collect();
    if bins.is_empty() {
        return None;
    }
    let fft = FftPlanner::<f32>::new().plan_fft_forward(n);
    let mut total = 0.0f64;
    let mut frames = 0usize;
    for chunk in samples.chunks_exact(n) {
        let mut buf: Vec<Complex<f32>> = chunk.iter().zip(&window).map(|(s, w)| s * w).collect();
        fft.process(&mut buf);
        let sum: f64 = bins
            .iter()
            .map(|&k| f64::from(buf[k].norm_sqr() / (window_power * n as f32)))
            .sum();
        total += sum / bins.len() as f64;
        frames += 1;
    }
    if frames == 0 {
        return None;
    }
    let mean = (total / frames as f64).max(1e-30);
    Some((10.0 * mean.log10() * 100.0).round() as i32)
}

/// 여러 홉의 전력(0.01dBFS)을 선형 평균한다.
pub fn combine_cdbfs(powers: &[i32]) -> Option<i32> {
    if powers.is_empty() {
        return None;
    }
    let mean = powers
        .iter()
        .map(|p| 10f64.powf(f64::from(*p) / 1000.0))
        .sum::<f64>()
        / powers.len() as f64;
    Some((1000.0 * mean.log10()).round() as i32)
}
