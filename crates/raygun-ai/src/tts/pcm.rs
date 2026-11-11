//! Synthesized speech as the players take it: 16-bit PCM, its pitch moved by playing it faster
//! or slower (the model speaks slower or faster to keep the reading speed).

/// `samples` (−1…1) as 16-bit little-endian PCM.
pub fn to_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// What a voice model is asked for so speech comes out at `speed` and `pitch` (1 = as made):
/// the model's speed, then how much faster the result is played ([`resample`]). Playing faster
/// raises the pitch, so the model speaks slower by as much.
pub fn speed_for(speed: f32, pitch: f32) -> f32 {
    (speed / pitch).clamp(0.25, 4.0)
}

/// `samples` played `factor` times faster (higher, shorter), by linear interpolation; 1 keeps
/// them as they are.
pub fn resample(samples: &[f32], factor: f32) -> Vec<f32> {
    if (factor - 1.0).abs() < 0.01 || samples.len() < 2 || factor <= 0.0 {
        return samples.to_vec();
    }
    let length = ((samples.len() as f64) / f64::from(factor)).floor() as usize;
    (0..length)
        .map(|index| {
            let at = index as f64 * f64::from(factor);
            let low = at.floor() as usize;
            let high = (low + 1).min(samples.len() - 1);
            let fraction = (at - low as f64) as f32;
            samples[low] + (samples[high] - samples[low]) * fraction
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_16_bit() {
        assert_eq!(
            to_pcm16(&[0.0, 1.0, -1.0, 2.0]),
            vec![0, 0, 0xff, 0x7f, 0x01, 0x80, 0xff, 0x7f]
        );
    }

    #[test]
    fn pitch_up_plays_faster_and_the_model_speaks_slower() {
        let samples: Vec<f32> = (0..1000).map(|index| index as f32 / 1000.0).collect();
        let higher = resample(&samples, 1.25);
        assert_eq!(higher.len(), 800);
        assert!((higher[400] - samples[500]).abs() < 1e-6);
        assert_eq!(resample(&samples, 1.0).len(), 1000);
        // Twice as fast with a 1.25 pitch: the model speaks 1.6× and the result plays 1.25× faster.
        assert!((speed_for(2.0, 1.25) - 1.6).abs() < 1e-6);
        assert_eq!(speed_for(1.0, 1.0), 1.0);
    }
}
