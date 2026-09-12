//! Best-effort live addon decoding from contiguous snapshots of the existing RX stream.
use crate::{addons, config::Config};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Capture {
    bytes: Vec<u8>,
    time: SystemTime,
}

#[derive(Clone)]
pub struct Tap {
    latest: Arc<Mutex<Option<Capture>>>,
    capture: Arc<AtomicBool>,
    samples: usize,
}

impl Tap {
    /// Called in the USB reader, before lossy display throttling. Never blocks RX.
    pub fn push(&self, bytes: &[u8], pending: &mut Vec<u8>) {
        if !self.capture.load(Ordering::Relaxed) {
            pending.clear();
            return;
        }
        let target = self.samples * 2;
        for chunk in bytes.chunks(target) {
            let take = (target - pending.len()).min(chunk.len());
            pending.extend_from_slice(&chunk[..take]);
            if pending.len() == target {
                if let Ok(mut latest) = self.latest.try_lock() {
                    *latest = Some(Capture {
                        bytes: std::mem::take(pending),
                        time: SystemTime::now(),
                    });
                } else {
                    pending.clear();
                }
            }
            pending.extend_from_slice(&chunk[take..]);
        }
    }
}

pub struct Decoder {
    pub tap: Tap,
    pub enabled: Arc<AtomicBool>,
    pub events: mpsc::Receiver<String>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Decoder {
    pub fn start(c: Config, stop: Arc<AtomicBool>, active: bool) -> Self {
        let tap = Tap {
            latest: Arc::new(Mutex::new(None)),
            capture: Arc::new(AtomicBool::new(false)),
            samples: (c.sample_rate as usize * 2).min(8_000_000),
        };
        let enabled = Arc::new(AtomicBool::new(active));
        let (send, events) = mpsc::sync_channel(512);
        let worker_tap = tap.clone();
        let worker_enabled = enabled.clone();
        let worker = thread::spawn(move || work(c, stop, worker_enabled, worker_tap, send));
        Self {
            tap,
            enabled,
            events,
            worker: Some(worker),
        }
    }

    pub fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn work(
    c: Config,
    stop: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    tap: Tap,
    send: mpsc::SyncSender<String>,
) {
    let format = match c.device.as_str() {
        "rtl" => "cu8",
        "audio" => "wav",
        "demo" => {
            let _ = send.try_send(
                "DEMO: synthetic spectrum; live RF decoders require a real sample source".into(),
            );
            return;
        }
        _ => "cs8",
    };
    let directory = std::env::temp_dir().join(format!(
        "thugsrf-live-{}-{}",
        std::process::id(),
        crate::stamp()
    ));
    use std::os::unix::fs::DirBuilderExt;
    if let Err(e) = std::fs::DirBuilder::new().mode(0o700).create(&directory) {
        let _ = send.try_send(format!("Live decoder temporary directory: {e}"));
        return;
    }
    let _scratch = Scratch(directory.clone());
    let path = directory.join(format!("capture.{format}"));
    let mut last_notice = String::new();
    while !stop.load(Ordering::Relaxed) {
        if !enabled.load(Ordering::Relaxed) {
            tap.capture.store(false, Ordering::Relaxed);
            if let Ok(mut latest) = tap.latest.lock() {
                *latest = None;
            }
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        let modules = match addons::list() {
            Ok(modules) => modules
                .into_iter()
                .filter(|a| {
                    a.manifest.enabled
                        && a.manifest.kind == "decoders"
                        && (a.manifest.formats.is_empty()
                            || a.manifest.formats.iter().any(|f| f == format))
                })
                .collect::<Vec<_>>(),
            Err(e) => {
                let notice = format!("Cannot load decoders: {e}");
                if notice != last_notice {
                    let _ = send.try_send(notice.clone());
                    last_notice = notice;
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
        };
        tap.capture.store(!modules.is_empty(), Ordering::Relaxed);
        let notice = if modules.is_empty() {
            format!(
                "No enabled decoders accept {format}; enable a compatible decoder in Addons (4)"
            )
        } else {
            format!(
                "Trying {} on {:.6} MHz · {:.2}s windows · best effort, busy windows skipped",
                modules
                    .iter()
                    .map(|a| a.manifest.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                c.frequency as f64 / 1e6,
                tap.samples as f64 / c.sample_rate as f64
            )
        };
        if notice != last_notice {
            let _ = send.try_send(notice.clone());
            last_notice = notice;
        }
        let capture = tap.latest.lock().ok().and_then(|mut latest| latest.take());
        let Some(capture) = capture.filter(|_| !modules.is_empty()) else {
            thread::sleep(Duration::from_millis(100));
            continue;
        };
        let result = (|| -> anyhow::Result<()> {
            if format == "wav" {
                let mut wav = hound::WavWriter::create(
                    &path,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: c.sample_rate,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )?;
                for pair in capture.bytes.as_chunks::<2>().0 {
                    wav.write_sample(i16::from_le_bytes([pair[0], pair[1]]))?;
                }
                wav.finalize()?;
            } else {
                std::fs::write(&path, &capture.bytes)?;
            }
            let request = addons::request(&path, format, c.sample_rate, c.frequency)?;
            for group in modules.chunks(3) {
                if stop.load(Ordering::Relaxed) || !enabled.load(Ordering::Relaxed) {
                    break;
                }
                thread::scope(|scope| {
                    for module in group {
                        let stop = &stop;
                        let enabled = &enabled;
                        let request = &request;
                        let send = &send;
                        let time = capture.time;
                        scope.spawn(move || {
                            let name = &module.manifest.name;
                            let result = addons::run_live(name, request, stop, enabled);
                            if stop.load(Ordering::Relaxed) || !enabled.load(Ordering::Relaxed) {
                                return;
                            }
                            let prefix = entry_prefix(time, c.frequency, name);
                            match result {
                                Ok(value) => {
                                    for message in result_lines(&value) {
                                        send_event(send, format!("{prefix} {message}"), stop);
                                    }
                                }
                                Err(e) => {
                                    send_event(send, format!("{prefix} ERROR: {e}"), stop);
                                }
                            }
                        });
                    }
                });
            }
            Ok(())
        })();
        if let Err(e) = result {
            let _ = send.try_send(format!("Live decoder input: {e}"));
        }
    }
}

fn send_event(send: &mpsc::SyncSender<String>, mut line: String, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match send.try_send(line) {
            Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => return,
            Err(mpsc::TrySendError::Full(pending)) => {
                line = pending;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

pub fn entry_prefix(time: SystemTime, hz: u64, name: &str) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86400;
    format!(
        "[{:02}:{:02}:{:02} UTC · {:.6} MHz · {name}]",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        hz as f64 / 1e6
    )
}

fn clean(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

fn result_lines(value: &serde_json::Value) -> Vec<String> {
    if let Some(error) = value.get("error") {
        return vec![format!("ERROR: {}", clean(&error.to_string()))];
    }
    if let Some(events) = value.get("events").and_then(|v| v.as_array()) {
        if !events.is_empty() {
            let mut lines: Vec<_> = events
                .iter()
                .map(|event| clean(&event.to_string()))
                .collect();
            if let Some(diagnostic) = value
                .get("diagnostics")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
            {
                lines.push(format!("diagnostics: {}", clean(diagnostic)));
            }
            return lines;
        }
        let diagnostic = value
            .get("diagnostics")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        return vec![if diagnostic.is_empty() {
            "No messages recovered in this window".into()
        } else {
            format!("No messages · {}", clean(diagnostic))
        }];
    }
    vec![clean(&value.to_string())]
}

pub fn append(log: &mut VecDeque<String>, line: String) {
    let mut chars = line.chars();
    let mut preview: String = chars.by_ref().take(3000).collect();
    if chars.next().is_some() {
        preview.push_str(" … [display shortened; file logging retains the full entry]");
    }
    log.push_front(preview);
    log.truncate(500);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_tap_preserves_contiguous_samples_across_uneven_blocks() {
        let tap = Tap {
            latest: Arc::new(Mutex::new(None)),
            capture: Arc::new(AtomicBool::new(true)),
            samples: 4,
        };
        let mut pending = Vec::new();
        tap.push(&[0, 1, 2, 3, 4], &mut pending);
        tap.push(&[5, 6, 7, 8, 9], &mut pending);
        assert_eq!(
            tap.latest.lock().unwrap().take().unwrap().bytes,
            vec![0, 1, 2, 3, 4, 5, 6, 7]
        );
        tap.push(&[10, 11, 12, 13, 14, 15], &mut pending);
        assert_eq!(
            tap.latest.lock().unwrap().take().unwrap().bytes,
            vec![8, 9, 10, 11, 12, 13, 14, 15]
        );
    }
    #[test]
    fn console_bounds_do_not_truncate_the_log_input() {
        let line = "x".repeat(5000);
        assert_eq!(clean(&line).len(), 5000);
        let mut log = VecDeque::new();
        append(&mut log, line);
        assert!(log[0].len() < 3200 && log[0].contains("display shortened"));
    }
    #[test]
    fn live_output_distinguishes_events_errors_and_empty_windows() {
        assert!(result_lines(&serde_json::json!({"events":[]}))[0].contains("No messages"));
        assert!(result_lines(&serde_json::json!({"error":"missing atest"}))[0].contains("ERROR"));
        assert!(
            result_lines(&serde_json::json!({"events":[{"mmsi":"123456789"}]}))[0]
                .contains("123456789")
        );
    }
}
