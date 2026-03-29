use std::collections::VecDeque;

use rustfft::{FftPlanner, num_complex::Complex};

const FFT_SIZE: usize = 2048;

/// Fraction of positive-frequency bins to use (top 60% discarded).
/// Music content above ~8 kHz contributes little visually and the sparse
/// high-frequency bins produce single-bin spikes that look like noise.
const BIN_USE_FRACTION: f32 = 0.40;

/// Minimum number of FFT bins a band must span before it gets its own value.
/// Bands narrower than this inherit the previous band's value, preventing
/// isolated single-bin spikes in the treble region.
const MIN_BINS_PER_BAND: usize = 3;

/// Compute `num_bands` frequency bands from the latest samples in the ring buffer.
///
/// - Applies a Hann window to the most recent FFT_SIZE samples.
/// - Uses only the lowest `BIN_USE_FRACTION` of positive-frequency bins so
///   sparse high-frequency bins cannot spike independently.
/// - Uses logarithmic bin grouping so bass bands are wider than treble bands.
/// - Bands covering fewer than `MIN_BINS_PER_BAND` FFT bins inherit the
///   previous band's value instead of computing their own.
/// - Converts magnitudes to dB, normalises to 0.0–1.0, then applies temporal
///   smoothing.
///
/// Returns a `Vec<f32>` of length `num_bands`.  If the buffer has fewer than
/// 16 samples the function returns all-zeros (silence / startup).
pub fn compute_bands(
    samples: &VecDeque<f32>,
    planner: &mut FftPlanner<f32>,
    prev_bands: &[f32],
    num_bands: usize,
) -> Vec<f32> {
    if num_bands == 0 {
        return Vec::new();
    }

    let n = FFT_SIZE;
    let available = samples.len().min(n);

    if available < 16 {
        // Not enough samples yet — return silence.
        return vec![0.0; num_bands];
    }

    let start = samples.len() - available;

    // Build Hann-windowed complex input, zero-padded to FFT_SIZE.
    let mut buffer: Vec<Complex<f32>> = (0..n)
        .map(|i| {
            if i < available {
                let s = samples[start + i];
                let window = 0.5
                    * (1.0
                        - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
                Complex { re: s * window, im: 0.0 }
            } else {
                Complex { re: 0.0, im: 0.0 }
            }
        })
        .collect();

    let fft = planner.plan_fft_forward(n);
    fft.process(&mut buffer);

    // Magnitudes of positive-frequency bins (skip DC bin 0).
    let half = n / 2;
    let magnitudes: Vec<f32> = buffer[1..half]
        .iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt())
        .collect();

    // num_mag = half - 1 = 1023 for FFT_SIZE 2048.
    let num_mag = magnitudes.len();

    // Cap to the lowest BIN_USE_FRACTION of bins.  The top 60% of the
    // spectrum contains sparse, noise-prone bins that spike visually.
    let num_mag_used = ((num_mag as f32 * BIN_USE_FRACTION) as usize).max(1);

    // Logarithmic band grouping spanning bin 1..=num_mag_used.
    let log_min = 1.0f32.ln();
    let log_max = (num_mag_used as f32).ln();

    // ── Compute bin ranges ────────────────────────────────────────────────────
    // Build (bin_start, bin_end) for each band before averaging, so we can
    // inspect the width and decide whether to merge.
    let band_ranges: Vec<(usize, usize)> = (0..num_bands)
        .map(|b| {
            let t0 = b as f32 / num_bands as f32;
            let t1 = (b + 1) as f32 / num_bands as f32;
            let bin_start = ((log_min + t0 * (log_max - log_min)).exp() as usize)
                .min(num_mag_used - 1);
            let bin_end = ((log_min + t1 * (log_max - log_min)).exp() as usize)
                .max(bin_start + 1)
                .min(num_mag_used);
            (bin_start, bin_end)
        })
        .collect();

    // ── Average magnitudes; merge narrow bands ────────────────────────────────
    // A band covering fewer than MIN_BINS_PER_BAND FFT bins is too sparse to
    // be meaningful — it inherits the previous band's averaged magnitude instead
    // of computing its own, suppressing isolated high-frequency spikes.
    let mut raw_bands: Vec<f32> = Vec::with_capacity(num_bands);
    let mut prev_val = 0.0f32;

    for (bin_start, bin_end) in &band_ranges {
        let width = bin_end - bin_start;
        let val = if width < MIN_BINS_PER_BAND {
            prev_val
        } else {
            let sum: f32 = magnitudes[*bin_start..*bin_end].iter().sum();
            let avg = sum / width as f32;
            prev_val = avg;
            avg
        };
        raw_bands.push(val);
    }

    // ── dB normalisation + temporal smoothing ────────────────────────────────
    // Scale factor: 2/N normalises so a full-scale sine gives ~0 dB.
    let scale = 2.0 / n as f32;

    raw_bands
        .iter()
        .zip(prev_bands.iter().chain(std::iter::repeat(&0.0f32)))
        .map(|(&m, &prev)| {
            let m_scaled = m * scale;
            let db = if m_scaled > 1e-10 {
                (20.0 * m_scaled.log10()).max(-60.0)
            } else {
                -60.0
            };
            let raw = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            prev * 0.7 + raw * 0.3
        })
        .collect()
}
