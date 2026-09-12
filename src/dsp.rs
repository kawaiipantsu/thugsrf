use anyhow::{Result, ensure};
use rustfft::{FftPlanner, num_complex::Complex32};
use serde::{Deserialize, Serialize};
use std::{f32::consts::PI, io::Read, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peak {
    pub frequency_hz: f64,
    pub power_dbfs: f32,
    pub snr_db: f32,
    pub bandwidth_hz: f64,
    pub context: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub source: String,
    pub center_hz: u64,
    pub sample_rate: u32,
    pub samples: usize,
    pub rms_dbfs: f32,
    pub noise_dbfs: f32,
    pub crest_db: f32,
    pub peaks: Vec<Peak>,
    pub spectrum_dbfs: Vec<f32>,
    pub notes: Vec<String>,
}
pub struct Analyzer {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    buffer: Vec<Complex32>,
    scratch: Vec<Complex32>,
    norm: f32,
}
impl Analyzer {
    pub fn new(n: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(n);
        let window: Vec<f32> = (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())
            .collect();
        let norm = window.iter().sum::<f32>().powi(2);
        let scratch = vec![Complex32::default(); fft.get_inplace_scratch_len()];
        Self {
            fft,
            window,
            buffer: vec![Complex32::default(); n],
            scratch,
            norm,
        }
    }
    pub fn analyze(
        &mut self,
        samples: &[Complex32],
        rate: u32,
        center: u64,
        threshold: f32,
        source: &str,
    ) -> Report {
        let n = self.buffer.len();
        let len = samples.len().min(n);
        let mean = samples.iter().take(len).copied().sum::<Complex32>() / len.max(1) as f32;
        let mut energy = 0.0f32;
        let mut max_power = 0.0f32;
        for i in 0..n {
            let s = samples.get(i).copied().unwrap_or_default();
            energy += s.norm_sqr();
            max_power = max_power.max(s.norm_sqr());
            self.buffer[i] = (s - mean) * self.window[i];
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);
        let spectrum: Vec<f32> = (0..n)
            .map(|i| db(self.buffer[(i + n / 2) % n].norm_sqr() / self.norm))
            .collect();
        let mut sorted = spectrum.clone();
        sorted.sort_by(f32::total_cmp);
        let noise = sorted[n / 2];
        let mut peaks = Vec::new();
        let mut i = 1;
        while i < n - 1 {
            if spectrum[i] > noise + threshold {
                let start = i;
                let mut peak = i;
                while i < n - 1 && spectrum[i] > noise + threshold {
                    if spectrum[i] > spectrum[peak] {
                        peak = i;
                    }
                    i += 1;
                }
                let freq = center as f64 + (peak as f64 / n as f64 - 0.5) * rate as f64;
                peaks.push(Peak {
                    frequency_hz: freq,
                    power_dbfs: spectrum[peak],
                    snr_db: spectrum[peak] - noise,
                    bandwidth_hz: (i - start) as f64 * rate as f64 / n as f64,
                    context: band_context(freq),
                });
            } else {
                i += 1;
            }
        }
        peaks.sort_by(|a, b| b.power_dbfs.total_cmp(&a.power_dbfs));
        peaks.truncate(16);
        let rms = db(energy / len.max(1) as f32);
        Report { source:source.into(), center_hz:center, sample_rate:rate, samples:len, rms_dbfs:rms, noise_dbfs:noise, crest_db:db(max_power)-rms, peaks, spectrum_dbfs:spectrum,
            notes:vec!["Hann window; DC removed; uncalibrated dBFS. Frequency context is a hypothesis, not protocol identification.".into()] }
    }
}
fn db(p: f32) -> f32 {
    10.0 * p.max(1e-16).log10()
}
pub fn band_context(hz: f64) -> String {
    let m = hz / 1e6;
    match m {
        x if (87.5..=108.0).contains(&x) => "FM broadcast candidate",
        x if (433.05..=434.79).contains(&x) => "433 MHz SRD / ISM candidate",
        x if (863.0..=870.0).contains(&x) => "European SRD candidate",
        x if (2400.0..=2483.5).contains(&x) => "2.4 GHz ISM: Wi-Fi / BLE / other",
        x if (144.0..=146.0).contains(&x) => "2 m amateur allocation candidate",
        x if (118.0..=137.0).contains(&x) => "VHF aeronautical candidate",
        _ => "Unclassified — inspect modulation and bandwidth",
    }
    .into()
}
pub fn iq(bytes: &[u8], unsigned: bool) -> Vec<Complex32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            if unsigned {
                Complex32::new((p[0] as f32 - 127.5) / 128.0, (p[1] as f32 - 127.5) / 128.0)
            } else {
                Complex32::new(p[0] as i8 as f32 / 128.0, p[1] as i8 as f32 / 128.0)
            }
        })
        .collect()
}
pub fn read_samples(
    path: &Path,
    format: &str,
    max: usize,
) -> Result<(Vec<Complex32>, Option<u32>)> {
    if format == "wav" || path.extension().is_some_and(|s| s == "wav") {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        ensure!(spec.channels > 0, "empty WAV channel layout");
        let values: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .take(max * spec.channels as usize)
                .collect::<std::result::Result<_, _>>()?,
            hound::SampleFormat::Int => reader
                .samples::<i32>()
                .take(max * spec.channels as usize)
                .map(|v| v.map(|v| v as f32 / 2f32.powi(spec.bits_per_sample as i32 - 1)))
                .collect::<std::result::Result<_, _>>()?,
        };
        ensure!(
            values.iter().all(|v| v.is_finite()),
            "WAV contains non-finite samples"
        );
        Ok((
            values
                .chunks(spec.channels as usize)
                .map(|c| Complex32::new(c.iter().sum::<f32>() / c.len() as f32, 0.0))
                .collect(),
            Some(spec.sample_rate),
        ))
    } else {
        ensure!(
            ["cs8", "cu8"].contains(&format),
            "format must be cs8, cu8, or wav"
        );
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take((max * 2) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len().is_multiple_of(2), "truncated I/Q sample");
        Ok((iq(&bytes, format == "cu8"), None))
    }
}
pub fn analyze_file(
    path: &Path,
    format: &str,
    rate: u32,
    center: u64,
    n: usize,
    threshold: f32,
) -> Result<Report> {
    let (samples, wav_rate) = read_samples(path, format, 4_194_304)?;
    ensure!(samples.len() >= 256, "need at least 256 samples");
    let requested_n = n;
    let n = n.min(1usize << samples.len().ilog2());
    let mut a = Analyzer::new(n);
    let mut report = a.analyze(
        &samples[..n],
        wav_rate.unwrap_or(rate),
        if wav_rate.is_some() { 0 } else { center },
        threshold,
        &path.display().to_string(),
    );
    let mut count = 1usize;
    // Average power across the bounded analysis window, rather than only inspecting the first FFT.
    let mut power: Vec<f32> = report
        .spectrum_dbfs
        .iter()
        .map(|v| 10f32.powf(v / 10.0))
        .collect();
    for chunk in samples[n..].chunks_exact(n) {
        let r = a.analyze(
            chunk,
            report.sample_rate,
            report.center_hz,
            threshold,
            &report.source,
        );
        for (p, d) in power.iter_mut().zip(&r.spectrum_dbfs) {
            *p += 10f32.powf(d / 10.0);
        }
        count += 1;
        if r.peaks.first().map_or(-160.0, |p| p.power_dbfs)
            > report.peaks.first().map_or(-160.0, |p| p.power_dbfs)
        {
            report.peaks = r.peaks;
        }
    }
    report.spectrum_dbfs = power.iter().map(|p| db(p / count as f32)).collect();
    report.samples = samples.len();
    if n < requested_n {
        report.notes.push(format!(
            "Short recording: using {n} FFT bins instead of requested {requested_n}."
        ));
    }
    report.notes.push(format!("Spectrum averages {count} frames; detections retain the strongest frame. File analysis capped at 4194304 samples; RMS/noise/crest describe first frame."));
    Ok(report)
}
pub fn spectrum_png(report: &Report, path: &Path) -> Result<()> {
    let (w, h) = (1024usize, 320usize);
    let mut pixels = vec![0u8; w * h * 3];
    for x in 0..w {
        let d = report.spectrum_dbfs[x * report.spectrum_dbfs.len() / w];
        let bar = (((d + 120.0) / 120.0).clamp(0.0, 1.0) * (h - 1) as f32) as usize;
        for y in 0..h {
            let k = (y * w + x) * 3;
            let c = if y >= h - 1 - bar {
                [240, 40, 80]
            } else {
                [11, 15, 24]
            };
            pixels[k..k + 3].copy_from_slice(&c);
        }
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tone_frequency_and_level() {
        let n = 2048;
        let samples: Vec<_> = (0..n)
            .map(|i| Complex32::from_polar(0.5, 2.0 * PI * 128.0 * i as f32 / n as f32))
            .collect();
        let r = Analyzer::new(n).analyze(&samples, 2048000, 100000000, 12.0, "test");
        assert!((r.peaks[0].frequency_hz - 100128000.0).abs() < 1.0);
        assert!((r.peaks[0].power_dbfs + 6.0206).abs() < 0.1);
    }
    #[test]
    fn silence_is_finite_and_has_no_peaks() {
        let r =
            Analyzer::new(256).analyze(&vec![Complex32::default(); 256], 8000, 0, 12.0, "silence");
        assert!(r.peaks.is_empty());
        assert!(r.spectrum_dbfs.iter().all(|x| x.is_finite()));
    }
    #[test]
    fn signed_unsigned_samples() {
        assert_eq!(
            iq(&[128, 127], false)[0],
            Complex32::new(-1.0, 127.0 / 128.0)
        );
        assert!(iq(&[128, 128], true)[0].norm() < 0.01);
    }
}
