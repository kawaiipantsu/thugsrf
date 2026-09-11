use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub device: String,
    pub frequency: u64,
    pub sample_rate: u32,
    pub lna_gain: u32,
    pub vga_gain: u32,
    pub rtl_gain: u32,
    pub serial: String,
    pub audio_device: String,
    pub fft_size: usize,
    pub threshold_db: f32,
    pub ai_provider: String,
    pub ai_model: String,
    pub local_url: String,
    pub sweep_start_mhz: u32,
    pub sweep_end_mhz: u32,
    pub sweep_bin_hz: u32,
    pub listen_mode: String,
    pub listen_bandwidth: u32,
    pub squelch_dbfs: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            device: "hackrf".into(),
            frequency: 433_920_000,
            sample_rate: 8_000_000,
            lna_gain: 16,
            vga_gain: 20,
            rtl_gain: 200,
            serial: String::new(),
            audio_device: "default".into(),
            fft_size: 2048,
            threshold_db: 12.0,
            ai_provider: "local".into(),
            ai_model: String::new(),
            local_url: "http://127.0.0.1:11434/v1".into(),
            sweep_start_mhz: 1,
            sweep_end_mhz: 6000,
            sweep_bin_hz: 1_000_000,
            listen_mode: "fm".into(),
            listen_bandwidth: 12500,
            squelch_dbfs: -65.0,
        }
    }
}
pub fn config_dir() -> PathBuf {
    base("XDG_CONFIG_HOME", ".config").join("thugsrf")
}
pub fn data_dir() -> PathBuf {
    base("XDG_DATA_HOME", ".local/share").join("thugsrf")
}
fn base(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into())).join(fallback)
        })
}
pub fn init() -> Result<()> {
    for p in [
        config_dir(),
        config_dir().join("decoders"),
        config_dir().join("identifiers"),
        data_dir().join("recordings"),
    ] {
        fs::create_dir_all(p)?;
    }
    if !config_dir().join("config.toml").exists() {
        Config::default().save()?;
    }
    Ok(())
}
impl Config {
    pub fn load() -> Result<Self> {
        let path = config_dir().join("config.toml");
        let c: Self = if path.exists() {
            toml::from_str(&fs::read_to_string(path)?)?
        } else {
            Self::default()
        };
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.sweep_start_mhz >= 1
                && self.sweep_start_mhz < self.sweep_end_mhz
                && self.sweep_end_mhz <= 6000,
            "sweep range must be within 1..6000 MHz"
        );
        ensure!(
            (100_000..=5_000_000).contains(&self.sweep_bin_hz),
            "sweep bins must be 100 kHz..5 MHz"
        );
        ensure!(
            ["am", "fm", "wfm"].contains(&self.listen_mode.as_str()),
            "listen_mode must be am, fm, or wfm"
        );
        ensure!(
            (3000..=200000).contains(&self.listen_bandwidth),
            "listen_bandwidth must be 3000..200000 Hz"
        );
        ensure!(
            self.squelch_dbfs.is_finite() && (-160.0..=0.0).contains(&self.squelch_dbfs),
            "squelch_dbfs must be -160..0"
        );
        ensure!(
            ["hackrf", "rtl", "audio", "demo"].contains(&self.device.as_str()),
            "device must be hackrf, rtl, audio, or demo"
        );
        ensure!(
            self.fft_size.is_power_of_two() && (256..=65536).contains(&self.fft_size),
            "fft_size must be a power of two from 256 to 65536"
        );
        ensure!(
            self.threshold_db.is_finite() && (0.0..=100.0).contains(&self.threshold_db),
            "threshold_db must be 0..100"
        );
        ensure!(
            self.lna_gain <= 40 && self.lna_gain.is_multiple_of(8),
            "HackRF LNA gain must be 0..40 in steps of 8"
        );
        ensure!(
            self.vga_gain <= 62 && self.vga_gain.is_multiple_of(2),
            "HackRF VGA gain must be 0..62 in steps of 2"
        );
        match self.device.as_str() {
            "hackrf" => {
                ensure!(
                    self.frequency <= 6_000_000_000,
                    "HackRF frequency must be <= 6 GHz"
                );
                ensure!(
                    (8_000_000..=20_000_000).contains(&self.sample_rate),
                    "HackRF sample rate must be 8..20 MS/s"
                );
            }
            "rtl" => {
                ensure!(
                    (24_000_000..=1_766_000_000).contains(&self.frequency),
                    "NESDR tuning range is 24..1766 MHz"
                );
                ensure!(
                    (225_001..=300_000).contains(&self.sample_rate)
                        || (900_001..=2_560_000).contains(&self.sample_rate),
                    "RTL rate must be 225001..300000 or 900001..2560000"
                );
            }
            "audio" => ensure!(
                (8000..=192000).contains(&self.sample_rate),
                "audio rate must be 8000..192000"
            ),
            _ => ensure!(self.sample_rate > 0, "sample rate must be positive"),
        }
        ensure!(
            ["local", "openai", "anthropic"].contains(&self.ai_provider.as_str()),
            "unknown AI provider"
        );
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        fs::create_dir_all(config_dir())?;
        let tmp = config_dir().join("config.toml.tmp");
        fs::write(&tmp, toml::to_string_pretty(self)?)?;
        fs::rename(tmp, config_dir().join("config.toml")).context("saving configuration")
    }
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        if key == "device" {
            let mut next = self.clone();
            next.device = value.into();
            next.sample_rate = match value {
                "rtl" => 2_400_000,
                "audio" => 48_000,
                _ => 8_000_000,
            };
            next.validate()?;
            *self = next;
            return Ok(());
        }
        let mut t = toml::Value::try_from(self.clone())?;
        let old = t.get(key).context("unknown setting")?;
        t[key] = match old {
            toml::Value::Integer(_) => toml::Value::Integer(value.parse()?),
            toml::Value::Float(_) => toml::Value::Float(value.parse()?),
            _ => toml::Value::String(value.into()),
        };
        let next: Self = t.try_into()?;
        next.validate()?;
        *self = next;
        Ok(())
    }
}
