//! Sequential HackRF sweep panorama. This is not simultaneous full-band IQ reception.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, serde::Serialize)]
pub struct Panorama {
    pub power: Vec<f32>,
    pub measured_pass: Vec<Option<u64>>,
    pub peak: Vec<f32>,
    pub coverage: f32,
    pub passes: u64,
    pub start_mhz: u32,
    pub end_mhz: u32,
    pub bin_hz: u32,
}
pub struct Survey {
    pub frames: mpsc::Receiver<Panorama>,
    pub events: mpsc::Receiver<String>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Survey {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
pub fn parse_line(line: &str) -> Result<(String, Vec<(f64, f32)>)> {
    let fields: Vec<_> = line.trim().split(',').map(str::trim).collect();
    ensure!(fields.len() >= 7, "short sweep row");
    let low: f64 = fields[2].parse()?;
    let high: f64 = fields[3].parse()?;
    let width: f64 = fields[4].parse()?;
    ensure!(
        low.is_finite() && high.is_finite() && width.is_finite() && high > low && width > 0.0,
        "invalid sweep bounds"
    );
    let mut points = Vec::new();
    for (i, value) in fields[6..].iter().enumerate() {
        let power: f32 = value.parse()?;
        let hz = low + (i as f64 + 0.5) * width;
        if power.is_finite() && hz < high {
            points.push((hz, power));
        }
    }
    Ok((format!("{} {}", fields[0], fields[1]), points))
}
impl Survey {
    pub fn start(c: &Config) -> Result<Self> {
        ensure!(c.device == "hackrf", "wideband sweep requires HackRF");
        c.validate()?;
        let mut cmd = Command::new("hackrf_sweep");
        cmd.args([
            "-f",
            &format!("{}:{}", c.sweep_start_mhz, c.sweep_end_mhz),
            "-w",
            &c.sweep_bin_hz.to_string(),
            "-l",
            &c.lna_gain.to_string(),
            "-g",
            &c.vga_gain.to_string(),
            "-a",
            "0",
            "-p",
            "0",
            "-n",
            "-P",
            "estimate",
        ]);
        if !c.serial.is_empty() {
            cmd.args(["-d", &c.serial]);
        }
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting hackrf_sweep")?;
        let out = child.stdout.take().context("sweep stdout")?;
        let err = child.stderr.take().context("sweep stderr")?;
        let (lines_tx, lines) = mpsc::sync_channel(2048);
        let reader = thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                match line {
                    Ok(line) => {
                        let _ = lines_tx.try_send(line);
                    }
                    Err(_) => break,
                }
            }
        });
        let log_reader = thread::spawn(move || {
            let mut err = err;
            let mut tail = Vec::new();
            let mut bytes = [0u8; 1024];
            while let Ok(n) = err.read(&mut bytes) {
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&bytes[..n]);
                if tail.len() > 8192 {
                    tail.drain(..tail.len() - 8192);
                }
            }
            String::from_utf8_lossy(&tail).into_owned()
        });
        let (frame_tx, frames) = mpsc::sync_channel(2);
        let (event_tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let c = c.clone();
        let worker = thread::spawn(move || {
            let count = (((c.sweep_end_mhz - c.sweep_start_mhz) as u64 * 1_000_000)
                .div_ceil(c.sweep_bin_hz as u64)) as usize;
            let mut frame = Panorama {
                power: vec![-160.0; count],
                measured_pass: vec![None; count],
                peak: vec![-160.0; count],
                coverage: 0.0,
                passes: 0,
                start_mhz: c.sweep_start_mhz,
                end_mhz: c.sweep_end_mhz,
                bin_hz: c.sweep_bin_hz,
            };
            let mut seen = vec![false; count];
            let mut stamp = String::new();
            let mut last = Instant::now();
            while !flag.load(Ordering::Relaxed) {
                match lines.recv_timeout(Duration::from_millis(100)) {
                    Ok(line) => {
                        if let Ok((date, points)) = parse_line(&line) {
                            if !stamp.is_empty() && date != stamp {
                                frame.passes += 1;
                                seen.fill(false);
                            }
                            stamp = date;
                            for (hz, power) in points {
                                if hz < c.sweep_start_mhz as f64 * 1e6 {
                                    continue;
                                }
                                let i = ((hz - c.sweep_start_mhz as f64 * 1e6)
                                    / c.sweep_bin_hz as f64)
                                    as usize;
                                if i < count {
                                    frame.power[i] = power;
                                    frame.measured_pass[i] = Some(frame.passes + 1);
                                    frame.peak[i] = frame.peak[i].max(power);
                                    seen[i] = true;
                                }
                            }
                            if last.elapsed() > Duration::from_millis(150) {
                                frame.coverage =
                                    seen.iter().filter(|s| **s).count() as f32 / count as f32;
                                let _ = frame_tx.try_send(frame.clone());
                                last = Instant::now();
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if child.try_wait().ok().flatten().is_some() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let status = match child.try_wait() {
                Ok(Some(s)) => Some(s),
                _ => {
                    let _ = child.kill();
                    child.wait().ok()
                }
            };
            let _ = reader.join();
            let log = log_reader.join().unwrap_or_default();
            if !flag.load(Ordering::Relaxed) {
                let _ = event_tx.send(format!("Sweep stopped ({status:?})\n{log}"));
            }
        });
        Ok(Self {
            frames,
            events,
            stop,
            worker: Some(worker),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_bin_centers() {
        let (_, p) =
            parse_line("2026-09-11, 12:00:00, 1000000, 3000000, 1000000, 20, -55, -42").unwrap();
        assert_eq!(p, vec![(1500000.0, -55.0), (2500000.0, -42.0)]);
        assert!(parse_line("x,y,NaN,1,0,1,-2").is_err());
    }
}
