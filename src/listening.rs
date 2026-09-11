//! Managed live audio worker. DSP uses bounded NumPy/SciPy blocks in an embedded helper.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};
pub struct Audio {
    child: Child,
}
impl Drop for Audio {
    fn drop(&mut self) {
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
    pub fn start(c: &Config, seconds: u32, tx: bool, tone: f64) -> Result<Self> {
        c.validate()?;
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
        let spec = serde_json::json!({"config":c,"seconds":seconds,"tx":tx,"tone":tone});
        let child = Command::new("/usr/bin/python3")
            .args([
                "-c",
                include_str!("../scripts/radio-audio.py"),
                &spec.to_string(),
            ])
            .env("OPENBLAS_NUM_THREADS", "1")
            .env("OMP_NUM_THREADS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("starting audio worker")?;
        Ok(Self { child })
    }
    pub fn finished(&mut self) -> Result<Option<String>> {
        if let Some(status) = self.child.try_wait()? {
            let path = crate::config::data_dir().join("audio.log");
            ensure!(
                status.success(),
                "Audio failed ({status})\n{}",
                std::fs::read_to_string(&path).unwrap_or_default()
            );
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
    let mut audio = Audio::start(c, seconds, tx, tone)?;
    loop {
        if let Some(s) = audio.finished()? {
            return Ok(s);
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
            ["am", "fm", "wfm"].contains(&r.mode.as_str())
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
