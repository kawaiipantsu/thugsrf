//! Managed live audio worker. DSP uses bounded NumPy/SciPy blocks in an embedded helper.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
/// Fan-out point for raw IQ bytes into a live (stdin-fed) audio worker. Cheap to
/// hold and poll even when nobody is listening: `push` is a lock + branch, and the
/// sender is swapped in place so starting/stopping/reconfiguring audio, or a Stream
/// restart on retune, never requires recreating this handle.
#[derive(Clone, Default)]
pub struct AudioFeed(Arc<Mutex<Option<mpsc::SyncSender<Vec<u8>>>>>);
impl AudioFeed {
    pub fn push(&self, bytes: &[u8]) {
        if let Ok(slot) = self.0.lock()
            && let Some(tx) = slot.as_ref()
        {
            let _ = tx.try_send(bytes.to_vec());
        }
    }
    fn set(&self, tx: Option<mpsc::SyncSender<Vec<u8>>>) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = tx;
        }
    }
}
pub struct Audio {
    child: Child,
    rds: Arc<Mutex<serde_json::Map<String, serde_json::Value>>>,
    live_feed: Option<AudioFeed>,
}
impl Drop for Audio {
    fn drop(&mut self) {
        if let Some(feed) = &self.live_feed {
            feed.set(None);
        }
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
    }
}
impl Audio {
    pub fn start(
        c: &Config,
        seconds: u32,
        tx: bool,
        tone: f64,
        live: Option<AudioFeed>,
    ) -> Result<Self> {
        c.validate()?;
        ensure!(
            live.is_none() || !tx,
            "live listening does not support microphone TX"
        );
        ensure!(
            (1..=3600).contains(&seconds),
            "audio duration must be 1..3600 seconds"
        );
        ensure!(
            ["hackrf", "rtl"].contains(&c.device.as_str()),
            "listening requires HackRF or RTL-SDR"
        );
        ensure!(
            !tx || (c.device == "hackrf" && seconds <= 60 && c.listen_mode != "wfm"),
            "microphone TX requires HackRF AM/NFM, at most 60 seconds"
        );
        ensure!(
            tone == 0.0 || (60.0..=260.0).contains(&tone),
            "CTCSS must be 0 or 60..260 Hz"
        );
        let spec = serde_json::json!({"config":c,"seconds":seconds,"tx":tx,"tone":tone,"stdin":live.is_some()});
        let helper = format!(
            "{}\n{}",
            include_str!("../addons/_shared/v0_2/rds.py"),
            include_str!("../scripts/radio-audio.py")
        );
        let mut child = Command::new("/usr/bin/python3")
            .args(["-c", &helper, &spec.to_string()])
            .env("OPENBLAS_NUM_THREADS", "1")
            .env("OMP_NUM_THREADS", "1")
            .stdin(if live.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("starting audio worker")?;
        if let Some(feed) = &live {
            let mut stdin = child.stdin.take().context("audio worker stdin")?;
            let (feed_tx, feed_rx) = mpsc::sync_channel::<Vec<u8>>(8);
            thread::spawn(move || {
                for bytes in feed_rx {
                    if stdin.write_all(&bytes).is_err() {
                        break;
                    }
                }
            });
            feed.set(Some(feed_tx));
        }
        let rds = Arc::new(Mutex::new(serde_json::Map::new()));
        let state = Arc::clone(&rds);
        let output = child.stdout.take().context("audio worker stdout")?;
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Ok(serde_json::Value::Object(event)) = serde_json::from_str(&line)
                    && let Ok(mut state) = state.lock()
                {
                    if event
                        .get("pi")
                        .is_some_and(|pi| state.get("pi") != Some(pi))
                    {
                        state.clear();
                    }
                    for key in [
                        "pi",
                        "ps",
                        "radiotext",
                        "prog_type",
                        "tp",
                        "ta",
                        "rds_status",
                    ] {
                        if let Some(value) = event.get(key) {
                            state.insert(key.into(), value.clone());
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            rds,
            live_feed: live,
        })
    }
    pub fn rds_summary(&self) -> String {
        let Ok(state) = self.rds.lock() else {
            return "RDS unavailable".into();
        };
        if let Some(status) = state.get("rds_status").and_then(|v| v.as_str()) {
            return status.chars().filter(|c| !c.is_control()).collect();
        }
        if state.is_empty() {
            return "RDS: waiting for station data…".into();
        }
        let fields: Vec<_> = ["ps", "pi", "prog_type", "radiotext"]
            .iter()
            .filter_map(|key| state.get(*key).and_then(|v| v.as_str()))
            .map(|s| s.chars().filter(|c| !c.is_control()).collect::<String>())
            .collect();
        format!(
            "RDS: {}{}",
            fields.join(" · "),
            if state.get("ta").and_then(|v| v.as_bool()) == Some(true) {
                " · TRAFFIC"
            } else {
                ""
            }
        )
    }
    pub fn finished(&mut self) -> Result<Option<String>> {
        if let Some(status) = self.child.try_wait()? {
            let path = crate::config::data_dir().join("audio.log");
            let diagnostics = std::fs::read_to_string(&path).unwrap_or_default();
            if !status.success() {
                let saved = crate::config::save_log("audio", &diagnostics);
                let note = saved.map_or_else(String::new, |p| {
                    format!(
                        "\n\nAlso saved to {} (kept across future runs)",
                        p.display()
                    )
                });
                anyhow::bail!("Audio failed ({status})\n{diagnostics}{note}");
            }
            Ok(Some(format!(
                "Audio ended; diagnostics: {}",
                path.display()
            )))
        } else {
            Ok(None)
        }
    }
}
pub fn run(c: &Config, seconds: u32, tx: bool, tone: f64) -> Result<String> {
    let mut audio = Audio::start(c, seconds, tx, tone, None)?;
    loop {
        if let Some(s) = audio.finished()? {
            return Ok(if c.listen_mode == "wfm" && !tx {
                format!("{s}\n{}", audio.rds_summary())
            } else {
                s
            });
        }
        if crate::CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok("Audio stopped".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub name: String,
    pub rx_hz: u64,
    pub tx_hz: Option<u64>,
    #[serde(default)]
    pub ctcss_hz: f64,
    pub mode: String,
    pub bandwidth: u32,
    #[serde(default)]
    pub source: String,
}
#[derive(Serialize, Deserialize)]
struct Directory {
    channels: Vec<Channel>,
}
pub fn presets() -> Vec<Channel> {
    [
        ("FM broadcast", 100_000_000, "wfm", 200_000),
        ("MW upper band", 1_008_000, "am", 10_000),
        ("SW 49 m", 6_070_000, "am", 10_000),
        ("SW 31 m", 9_650_000, "am", 10_000),
        ("SW 25 m", 11_800_000, "am", 10_000),
        ("SW 19 m", 15_200_000, "am", 10_000),
    ]
    .into_iter()
    .map(|(n, f, m, b)| Channel {
        name: n.into(),
        rx_hz: f,
        tx_hz: None,
        ctcss_hz: 0.0,
        mode: m.into(),
        bandwidth: b,
        source: "Tuning preset; station availability varies".into(),
    })
    .collect()
}
pub fn channels() -> Result<Vec<Channel>> {
    let p = crate::config::config_dir().join("repeaters.toml");
    if !p.exists() {
        save_channels(&[
            Channel {
                name: "VHF simplex (not a repeater)".into(),
                rx_hz: 145_500_000,
                tx_hz: Some(145_500_000),
                ctcss_hz: 0.0,
                mode: "fm".into(),
                bandwidth: 12_500,
                source: "User-editable simplex preset".into(),
            },
            Channel {
                name: "UHF simplex (not a repeater)".into(),
                rx_hz: 433_500_000,
                tx_hz: Some(433_500_000),
                ctcss_hz: 0.0,
                mode: "fm".into(),
                bandwidth: 12_500,
                source: "User-editable simplex preset".into(),
            },
        ])?;
    }
    let d: Directory = toml::from_str(&std::fs::read_to_string(p)?)?;
    validate(&d.channels)?;
    Ok(d.channels)
}
fn validate(rows: &[Channel]) -> Result<()> {
    ensure!(rows.len() <= 2000, "maximum 2000 channels");
    for r in rows {
        ensure!(
            !r.name.trim().is_empty() && (1_000_000..=6_000_000_000).contains(&r.rx_hz),
            "invalid channel name/RX"
        );
        ensure!(
            r.tx_hz
                .is_none_or(|f| (1_000_000..=6_000_000_000).contains(&f)),
            "invalid TX frequency"
        );
        ensure!(
            ["am", "fm", "nfm", "wfm"].contains(&r.mode.as_str())
                && (3000..=200000).contains(&r.bandwidth),
            "invalid mode/bandwidth"
        );
        ensure!(
            r.ctcss_hz == 0.0 || (60.0..=260.0).contains(&r.ctcss_hz),
            "invalid CTCSS"
        );
    }
    Ok(())
}
pub fn save_channels(rows: &[Channel]) -> Result<()> {
    validate(rows)?;
    std::fs::create_dir_all(crate::config::config_dir())?;
    let path = crate::config::config_dir().join("repeaters.toml");
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(
        &tmp,
        toml::to_string_pretty(&Directory {
            channels: rows.to_vec(),
        })?,
    )?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
pub fn add(channel: Channel) -> Result<String> {
    let mut rows = channels()?;
    rows.push(channel);
    save_channels(&rows)?;
    Ok("Channel saved".into())
}
