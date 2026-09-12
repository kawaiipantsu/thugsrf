use crate::dsp;
use anyhow::{Result, ensure};
use std::{io::Write, path::Path};
pub fn decode(path: &Path, format: &str, rate: u32, mode: &str) -> Result<String> {
    if mode == "rds" {
        return decode_rds(path, format, rate);
    }
    let (s, r) = dsp::read_samples(path, format, 2_000_000)?;
    let rate = r.unwrap_or(rate);
    ensure!(!s.is_empty(), "empty recording");
    let result = match mode {
        "ook" => {
            let max = s.iter().map(|x| x.norm()).fold(0f32, f32::max);
            let threshold = max * 0.4;
            let mut pulses = Vec::new();
            let mut state = s[0].norm() > threshold;
            let mut count = 0u64;
            for x in &s {
                let on = x.norm() > threshold;
                if on != state {
                    if pulses.len() < 10000 {
                        pulses.push(serde_json::json!({"high":state,"microseconds":count as f64/rate as f64*1e6}));
                    }
                    count = 0;
                    state = on;
                }
                count += 1;
            }
            if pulses.len() < 10000 {
                pulses.push(
                    serde_json::json!({"high":state,"microseconds":count as f64/rate as f64*1e6}),
                );
            }
            serde_json::json!({"decoder":"ook-envelope","threshold":threshold,"pulses":pulses,"note":"Raw pulse timings; symbol clock and protocol are not inferred. Up to 10000 pulses."})
        }
        "fsk" => {
            let frequencies: Vec<f32> = s
                .windows(2)
                .take(10000)
                .map(|p| (p[1] * p[0].conj()).arg() * rate as f32 / (2.0 * std::f32::consts::PI))
                .collect();
            serde_json::json!({"decoder":"fsk-discriminator","instantaneous_hz":frequencies,"note":"Unfiltered phase discriminator; not synchronized protocol bits."})
        }
        _ => anyhow::bail!(
            "builtin decoder must be ook, fsk or rds; use addon run for protocol addons"
        ),
    };
    Ok(serde_json::to_string_pretty(&result)?)
}
pub fn encode(path: &Path, bits: &str, rate: u32, baud: u32, mode: &str) -> Result<String> {
    ensure!(
        !bits.is_empty() && bits.len() <= 100000 && bits.bytes().all(|b| b == b'0' || b == b'1'),
        "bits must contain 1..100000 binary digits"
    );
    ensure!(
        baud > 0 && rate >= baud && rate <= 20_000_000,
        "invalid rate/baud"
    );
    let n = rate / baud;
    ensure!(
        n as u64 * bits.len() as u64 <= 100_000_000,
        "output exceeds 100 million samples"
    );
    let file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)?;
    match mode {
        "ook" => {
            let mut w = std::io::BufWriter::new(file);
            for bit in bits.bytes() {
                for _ in 0..n {
                    w.write_all(&[if bit == b'1' { 90 } else { 0 }, 0])?;
                }
            }
            w.flush()?;
        }
        "afsk" => {
            ensure!(rate >= 8000, "AFSK requires at least 8000 samples/sec");
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::new(file, spec)?;
            let mut phase = 0f64;
            for bit in bits.bytes() {
                let hz = if bit == b'1' { 1200.0 } else { 2200.0 };
                for _ in 0..n {
                    w.write_sample((phase.sin() * 16000.0) as i16)?;
                    phase = (phase + 2.0 * std::f64::consts::PI * hz / rate as f64)
                        % (2.0 * std::f64::consts::PI);
                }
            }
            w.finalize()?;
        }
        _ => anyhow::bail!("mode must be ook (cs8 IQ) or afsk (WAV)"),
    };
    Ok(format!(
        "Encoded {} bits to {}; actual baud {:.3}",
        bits.len(),
        path.display(),
        rate as f64 / n as f64
    ))
}
pub fn demod(path: &Path, output: &Path, format: &str, rate: u32, mode: &str) -> Result<String> {
    ensure!(["am", "fm"].contains(&mode), "mode must be am or fm");
    let (s, r) = dsp::read_samples(path, format, 16_000_000)?;
    let rate = r.unwrap_or(rate);
    ensure!(rate >= 48000, "demodulation requires sample rate >= 48 kHz");
    let decim = (rate / 48000).max(1);
    let out_rate = rate / decim;
    let file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut w = hound::WavWriter::new(
        file,
        hound::WavSpec {
            channels: 1,
            sample_rate: out_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    let mut prev = rustfft::num_complex::Complex32::new(1.0, 0.0);
    let mut dc = 0f32;
    let mut lp = 0f32;
    let alpha = 1.0 - (-2.0 * std::f32::consts::PI * 12000.0 / rate as f32).exp();
    for (i, x) in s.iter().enumerate() {
        let v = if mode == "fm" {
            (*x * prev.conj()).arg() * rate as f32 / (2.0 * std::f32::consts::PI * 75000.0)
        } else {
            x.norm()
        };
        prev = *x;
        dc += 0.0001 * (v - dc);
        lp += alpha * ((v - dc) - lp);
        if i % decim as usize == 0 {
            w.write_sample((lp.clamp(-1.0, 1.0) * 30000.0) as i16)?;
        }
    }
    w.finalize()?;
    Ok(format!(
        "Wrote {} Hz mono WAV; tune the carrier to center before demodulation. Basic single-pole audio filter; input limited to 16 million samples.",
        out_rate
    ))
}

fn decode_rds(path: &Path, format: &str, rate: u32) -> Result<String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::atomic::Ordering,
        thread,
        time::{Duration, Instant},
    };
    let spec = serde_json::json!({"path":path,"format":format,"rate":rate});
    let helper = format!(
        "{}\n{}",
        include_str!("../addons/_shared/v0_2/rds.py"),
        r#"
import signal, sys
signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
signal.signal(signal.SIGALRM, lambda *_: sys.exit('RDS decode timed out'))
signal.alarm(120)
try:
    print(json.dumps(decode_rds_file(json.loads(sys.argv[1]))))
except Exception as exc:
    sys.exit(str(exc))
"#
    );
    let mut child = Command::new("/usr/bin/python3")
        .args(["-c", &helper, &spec.to_string()])
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("OMP_NUM_THREADS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let output = thread::spawn(move || {
        let mut b = Vec::new();
        stdout.take(1_048_577).read_to_end(&mut b).map(|_| b)
    });
    let errors = thread::spawn(move || {
        let mut b = Vec::new();
        stderr.take(8192).read_to_end(&mut b).map(|_| b)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if crate::CANCELLED.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(135)
        {
            let _ = Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status();
            // The Python handler closes its owned decoder before exiting.
            let deadline = Instant::now() + Duration::from_secs(12);
            while child.try_wait()?.is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("RDS decoding cancelled or timed out");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let data = output
        .join()
        .map_err(|_| anyhow::anyhow!("RDS output reader failed"))??;
    let error = errors
        .join()
        .map_err(|_| anyhow::anyhow!("RDS diagnostic reader failed"))??;
    ensure!(status.success(), "{}", String::from_utf8_lossy(&error));
    ensure!(data.len() <= 1_048_576, "RDS output exceeds 1 MiB");
    let result: serde_json::Value = serde_json::from_slice(&data)?;
    Ok(serde_json::to_string_pretty(&result)?)
}
