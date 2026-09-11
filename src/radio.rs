use crate::{
    config::Config,
    dsp::{self, Analyzer, Report},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

pub fn command(c: &Config, output: &str, count: Option<u64>) -> Result<Command> {
    c.validate()?;
    let mut cmd = match c.device.as_str() {
        "hackrf" => {
            let mut cmd = Command::new("hackrf_transfer");
            cmd.args([
                "-r",
                output,
                "-f",
                &c.frequency.to_string(),
                "-s",
                &c.sample_rate.to_string(),
                "-l",
                &c.lna_gain.to_string(),
                "-g",
                &c.vga_gain.to_string(),
            ]);
            if !c.serial.is_empty() {
                cmd.args(["-d", &c.serial]);
            }
            if let Some(n) = count {
                cmd.args(["-n", &n.to_string()]);
            }
            cmd
        }
        "rtl" => {
            let mut cmd = Command::new("rtl_sdr");
            cmd.args([
                "-f",
                &c.frequency.to_string(),
                "-s",
                &c.sample_rate.to_string(),
                "-g",
                &(c.rtl_gain as f64 / 10.0).to_string(),
            ]);
            if !c.serial.is_empty() {
                cmd.args(["-d", &c.serial]);
            }
            if let Some(n) = count {
                cmd.args(["-n", &n.to_string()]);
            }
            cmd.arg(output);
            cmd
        }
        "audio" => {
            let mut cmd = Command::new("arecord");
            cmd.args([
                "-q",
                "-D",
                &c.audio_device,
                "-t",
                "raw",
                "-f",
                "S16_LE",
                "-c",
                "1",
                "-r",
                &c.sample_rate.to_string(),
            ]);
            if let Some(n) = count {
                cmd.args(["-d", &(n / c.sample_rate as u64).max(1).to_string()]);
            }
            cmd.arg(output);
            cmd
        }
        _ => anyhow::bail!("demo is a synthetic TUI source; use encode to create fixtures"),
    };
    cmd.stdin(Stdio::null());
    Ok(cmd)
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
/// Lifecycle messages use a separate channel so a full spectrum queue cannot hide errors.
pub enum StreamEvent {
    Status(String),
    Failed(String),
}
pub struct Stream {
    pub frames: Receiver<Report>,
    pub events: Receiver<StreamEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
impl Stream {
    pub fn start(c: Config) -> Result<Self> {
        c.validate()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (tx, frames) = mpsc::sync_channel(2);
        let (events_tx, events) = mpsc::channel();
        let worker = thread::spawn(move || {
            if c.device == "demo" {
                let _ = events_tx.send(StreamEvent::Status("Receiving synthetic demo".into()));
                let mut a = Analyzer::new(c.fft_size);
                let mut tick = 0.0f32;
                while !flag.load(Ordering::Relaxed) {
                    let s: Vec<_> = (0..c.fft_size)
                        .map(|i| {
                            let p = 2.0 * std::f32::consts::PI * i as f32 / c.fft_size as f32;
                            rustfft::num_complex::Complex32::from_polar(
                                0.5,
                                p * (120.0 + tick.sin() * 20.0),
                            ) + rustfft::num_complex::Complex32::from_polar(0.15, p * -230.0)
                        })
                        .collect();
                    let _ = tx.try_send(a.analyze(
                        &s,
                        c.sample_rate,
                        c.frequency,
                        c.threshold_db,
                        "DEMO · synthetic",
                    ));
                    tick += 0.07;
                    thread::sleep(Duration::from_millis(60));
                }
                return;
            }
            let mut diagnostics = Vec::new();
            for attempt in 1..=3 {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let result = receive_attempt(&c, &flag, &tx, &events_tx);
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let retry = match result {
                    Ok((had_data, detail)) => {
                        let retry = c.device == "hackrf"
                            && !had_data
                            && detail.contains("Couldn't transfer any bytes for one second.");
                        diagnostics.push(format!("Attempt {attempt}:\n{detail}"));
                        retry
                    }
                    Err(e) => {
                        diagnostics.push(format!("Attempt {attempt}: {e:#}"));
                        false
                    }
                };
                if !retry || attempt == 3 {
                    break;
                }
                let _ = events_tx.send(StreamEvent::Status(format!(
                    "HackRF delivered no startup data; retrying ({}/3)…",
                    attempt + 1
                )));
                // Release the previous USB handle before retrying, while keeping Space/quit responsive.
                for _ in 0..5 {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            let _ = events_tx.send(StreamEvent::Failed(diagnostics.join("\n\n")));
        });
        Ok(Self {
            frames,
            events,
            stop,
            worker: Some(worker),
        })
    }
}

fn receive_attempt(
    c: &Config,
    stop: &AtomicBool,
    frames: &mpsc::SyncSender<Report>,
    events: &mpsc::Sender<StreamEvent>,
) -> Result<(bool, String)> {
    let mut child = ChildGuard(
        command(c, "-", None)?
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting receiver; run thugsrf doctor")?,
    );
    let mut out = child.0.stdout.take().context("receiver stdout")?;
    let mut err = child.0.stderr.take().context("receiver stderr")?;
    let err_reader = thread::spawn(move || {
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
    // Always drain USB; drop display blocks when DSP/UI is slower than the radio.
    let (raw_tx, raw_rx) = mpsc::sync_channel(2);
    let block_bytes = (c.fft_size * 2).max(65536);
    let seen_data = Arc::new(AtomicBool::new(false));
    let reader_seen = seen_data.clone();
    let reader = thread::spawn(move || -> std::io::Result<()> {
        loop {
            let mut bytes = vec![0; block_bytes];
            let mut offset = 0;
            while offset < bytes.len() {
                match out.read(&mut bytes[offset..]) {
                    Ok(0) => return Ok(()),
                    Ok(n) => {
                        reader_seen.store(true, Ordering::Relaxed);
                        offset += n;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            if let Err(mpsc::TrySendError::Disconnected(_)) = raw_tx.try_send(bytes) {
                return Ok(());
            }
        }
    });
    let mut analyzer = Analyzer::new(c.fft_size);
    let mut last = Instant::now() - Duration::from_secs(1);
    let mut announced = false;
    while !stop.load(Ordering::Relaxed) {
        match raw_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => {
                if !announced {
                    let _ = events.send(StreamEvent::Status(format!(
                        "Receiving {} · {:.3} MHz",
                        c.device,
                        c.frequency as f64 / 1e6
                    )));
                    announced = true;
                }
                if last.elapsed() < Duration::from_millis(60) {
                    continue;
                }
                last = Instant::now();
                let samples = if c.device == "audio" {
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| {
                            rustfft::num_complex::Complex32::new(
                                i16::from_le_bytes(*p) as f32 / 32768.0,
                                0.0,
                            )
                        })
                        .collect()
                } else {
                    dsp::iq(&bytes, c.device == "rtl")
                };
                let _ = frames.try_send(analyzer.analyze(
                    &samples,
                    c.sample_rate,
                    if c.device == "audio" { 0 } else { c.frequency },
                    c.threshold_db,
                    &c.device,
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.0.try_wait().ok().flatten().is_some() {
                    break;
                }
            }
        }
    }
    // stdout EOF can race with process exit; allow a short grace period for final diagnostics.
    let deadline = Instant::now() + Duration::from_millis(250);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let read_result = reader.join();
    let mut detail = format!(
        "Receiver exited with {status}\n{}",
        err_reader.join().unwrap_or_default()
    );
    if let Ok(Err(e)) = read_result {
        detail.push_str(&format!("\nReading receiver output failed: {e}"));
    }
    Ok((seen_data.load(Ordering::Relaxed), detail))
}
pub fn capture(c: &Config, path: &Path, seconds: u32) -> Result<String> {
    ensure!(
        (1..=3600).contains(&seconds),
        "duration must be 1..3600 seconds"
    );
    c.validate()?;
    let count = c.sample_rate as u64 * seconds as u64;
    // Reserve the destination; never silently replace an investigation recording.
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut cmd = command(c, "-", Some(count))?;
    if c.device == "audio" {
        cmd = Command::new("arecord");
        cmd.args([
            "-q",
            "-D",
            &c.audio_device,
            "-t",
            "wav",
            "-f",
            "S16_LE",
            "-c",
            "1",
            "-r",
            &c.sample_rate.to_string(),
            "-d",
            &seconds.to_string(),
            "-",
        ]);
    }
    let mut child = ChildGuard(cmd.stdout(file).stderr(Stdio::piped()).spawn()?);
    let stderr = child.0.stderr.take().context("capture stderr")?;
    let log = thread::spawn(move || {
        let mut stderr = stderr;
        let mut tail = Vec::new();
        let mut bytes = [0u8; 2048];
        while let Ok(n) = stderr.read(&mut bytes) {
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
    let deadline = Instant::now() + Duration::from_secs(seconds as u64 + 15);
    let status = loop {
        ensure!(
            !crate::CANCELLED.load(Ordering::Relaxed),
            "capture cancelled; partial recording retained"
        );
        if let Some(s) = child.0.try_wait()? {
            break s;
        }
        ensure!(
            Instant::now() < deadline,
            "capture timed out; partial recording retained"
        );
        thread::sleep(Duration::from_millis(50));
    };
    let log = log.join().unwrap_or_default();
    ensure!(
        status.success(),
        "capture failed; partial file retained: {log}"
    );
    let size = std::fs::metadata(path)?.len();
    ensure!(
        size >= count * 2,
        "short capture: {size} bytes, expected at least {}; partial file retained",
        count * 2
    );
    let format = match c.device.as_str() {
        "rtl" => "cu8",
        "audio" => "wav",
        _ => "cs8",
    };
    let metadata = if format == "wav" {
        serde_json::json!({"format":"wav", "sample_rate":c.sample_rate, "channels":1, "source":"audio", "bytes":size})
    } else {
        serde_json::json!({"global":{"core:datatype":if format=="cu8"{"cu8"}else{"ci8"},"core:sample_rate":c.sample_rate,"core:version":"1.2.5","core:description":"THUGS(red) RF recording","thugsrf:format":format},"captures":[{"core:sample_start":0,"core:frequency":c.frequency}],"annotations":[]})
    };
    let meta = path.with_extension(if format == "wav" {
        "wav.json"
    } else {
        "sigmf-meta"
    });
    File::options()
        .write(true)
        .create_new(true)
        .open(meta)?
        .write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
    Ok(format!(
        "Recorded {size} bytes to {} ({format}); metadata saved",
        path.display()
    ))
}
pub fn replay(c: &Config, path: &Path, gain: u32) -> Result<String> {
    ensure!(
        c.device == "hackrf",
        "RF replay requires HackRF; RTL-SDR is receive-only"
    );
    c.validate()?;
    ensure!(gain <= 47, "TX gain must be 0..47 dB");
    let len = std::fs::metadata(path)?.len();
    ensure!(
        len > 0 && len.is_multiple_of(2),
        "expected nonempty signed 8-bit interleaved IQ (cs8)"
    );
    let mut cmd = Command::new("hackrf_transfer");
    cmd.args(["-t"]).arg(path).args([
        "-f",
        &c.frequency.to_string(),
        "-s",
        &c.sample_rate.to_string(),
        "-x",
        &gain.to_string(),
        "-a",
        "0",
    ]);
    if !c.serial.is_empty() {
        cmd.args(["-d", &c.serial]);
    }
    let seconds = len / (c.sample_rate as u64 * 2) + 15;
    ensure!(seconds <= 3615, "replay limited to one hour");
    let output = bounded_output(&mut cmd, seconds)?;
    ensure!(output.0.success(), "HackRF replay failed: {}", output.1);
    Ok("Replay complete".into())
}
pub fn doctor() -> String {
    let mut result = String::new();
    for (program, args) in [
        ("hackrf_info", vec![]),
        ("rtl_test", vec!["-t"]),
        ("aplay", vec!["-l"]),
        ("arecord", vec!["-l"]),
    ] {
        result.push_str(&format!("\n━━ {program} ━━\n"));
        match Command::new("timeout")
            .arg("8")
            .arg(program)
            .args(args)
            .output()
        {
            Ok(o) => {
                result.push_str(&String::from_utf8_lossy(&o.stdout));
                result.push_str(&String::from_utf8_lossy(&o.stderr));
                result.push_str(&format!("exit: {}\n", o.status));
            }
            Err(e) => result.push_str(&e.to_string()),
        }
    }
    result
}

/// Drain a child's diagnostics and enforce a deadline; never expose output on the TUI terminal.
pub fn bounded_output(
    cmd: &mut Command,
    seconds: u64,
) -> Result<(std::process::ExitStatus, String)> {
    let mut child = ChildGuard(
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut err = child.0.stderr.take().context("child stderr")?;
    let reader = thread::spawn(move || {
        let mut tail = Vec::new();
        let mut bytes = [0; 2048];
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
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        ensure!(
            !crate::CANCELLED.load(Ordering::Relaxed),
            "operation cancelled"
        );
        ensure!(
            start.elapsed() < Duration::from_secs(seconds),
            "operation timed out"
        );
        thread::sleep(Duration::from_millis(50));
    };
    Ok((status, reader.join().unwrap_or_default()))
}
