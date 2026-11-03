//! Whisper's mel filter bank, computed rather than shipped: librosa's `filters.mel` with
//! `htk=False` and `norm="slaney"`, which is what Whisper was trained with.

/// `n_mels` rows of `n_fft / 2 + 1` weights, row-major.
pub fn mel_filters(n_mels: usize, n_fft: usize, sample_rate: f64) -> Vec<f32> {
    let bins = n_fft / 2 + 1;
    let nyquist = sample_rate / 2.0;
    let fft_freqs: Vec<f64> = (0..bins)
        .map(|bin| bin as f64 * nyquist / (bins - 1) as f64)
        .collect();
    let (low, high) = (hz_to_mel(0.0), hz_to_mel(nyquist));
    let mel_freqs: Vec<f64> = (0..n_mels + 2)
        .map(|index| mel_to_hz(low + (high - low) * index as f64 / (n_mels + 1) as f64))
        .collect();
    let mut weights = vec![0f32; n_mels * bins];
    for mel in 0..n_mels {
        let (left, center, right) = (mel_freqs[mel], mel_freqs[mel + 1], mel_freqs[mel + 2]);
        let norm = 2.0 / (right - left);
        for (bin, &freq) in fft_freqs.iter().enumerate() {
            let lower = (freq - left) / (center - left);
            let upper = (right - freq) / (right - center);
            weights[mel * bins + bin] = (lower.min(upper).max(0.0) * norm) as f32;
        }
    }
    weights
}

const F_SP: f64 = 200.0 / 3.0;
const MIN_LOG_HZ: f64 = 1000.0;
const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;

fn log_step() -> f64 {
    6.4f64.ln() / 27.0
}

fn hz_to_mel(hz: f64) -> f64 {
    if hz >= MIN_LOG_HZ {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / log_step()
    } else {
        hz / F_SP
    }
}

fn mel_to_hz(mel: f64) -> f64 {
    if mel >= MIN_LOG_MEL {
        MIN_LOG_HZ * (log_step() * (mel - MIN_LOG_MEL)).exp()
    } else {
        mel * F_SP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_turns_back_into_hertz() {
        for hz in [0.0, 440.0, 999.0, 1000.0, 4000.0, 8000.0] {
            assert!((mel_to_hz(hz_to_mel(hz)) - hz).abs() < 1e-6, "{hz}");
        }
    }

    #[test]
    fn filters_are_triangles_in_order() {
        let filters = mel_filters(80, 400, 16_000.0);
        assert_eq!(filters.len(), 80 * 201);
        let mut last_peak = 0;
        for mel in 0..80 {
            let row = &filters[mel * 201..(mel + 1) * 201];
            assert!(row.iter().all(|weight| *weight >= 0.0));
            let peak = (0..201).max_by(|a, b| row[*a].total_cmp(&row[*b])).unwrap();
            assert!(peak >= last_peak, "row {mel} peaks before row {}", mel - 1);
            last_peak = peak;
        }
        // Slaney's normalization: the first filter's peak is 2 / (its width in Hz).
        let first_peak = filters[..201].iter().copied().fold(0.0, f32::max);
        assert!((first_peak - 0.0248).abs() < 0.002, "{first_peak}");
    }
}
