use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub device: String,
    pub frequency: u64,
    pub fine_tune_hz: u64,
    pub coarse_tune_hz: u64,
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
            fine_tune_hz: 500_000,
            coarse_tune_hz: 10_000_000,
            sample_rate: 8_000_000,
            lna_gain: 16,
            vga_gain: 20,
            rtl_gain: 200,
            serial: String::new(),
            audio_device: "default".into(),
            fft_size: 8192,
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
/// Durable failure diagnostics, kept across runs; unlike `data_dir()`'s working
/// files (e.g. audio.log), which a later run truncates and overwrites.
pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}
/// Persist `content` as a timestamped `<kind>-<unix ms>.log` under `log_dir()`,
/// pruning older files of the same kind beyond the newest 20 so this never grows
/// unbounded. Best-effort: a write failure here must never mask the real error.
pub fn save_log(kind: &str, content: &str) -> Option<PathBuf> {
    let dir = log_dir();
    fs::create_dir_all(&dir).ok()?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("{kind}-{millis}.log"));
    fs::write(&path, content).ok()?;
    let prefix = format!("{kind}-");
    if let Ok(entries) = fs::read_dir(&dir) {
        let mut own: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with(&prefix))
            })
            .collect();
        own.sort_by_key(std::fs::DirEntry::file_name);
        for stale in own.iter().rev().skip(20) {
            let _ = fs::remove_file(stale.path());
        }
    }
    Some(path)
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
            (1..=6_000_000_000).contains(&self.fine_tune_hz)
                && (1..=6_000_000_000).contains(&self.coarse_tune_hz),
            "tuning steps must be 1 Hz..6 GHz"
        );
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
            ["am", "fm", "nfm", "wfm"].contains(&self.listen_mode.as_str()),
            "listen_mode must be am, fm, nfm, or wfm"
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
            toml::Value::Integer(_)
                if ["frequency", "fine_tune_hz", "coarse_tune_hz"].contains(&key) =>
            {
                toml::Value::Integer(parse_frequency(value)?.try_into()?)
            }
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

/// Parse decimal frequencies exactly, without floating-point rounding.
pub fn parse_frequency(input: &str) -> Result<u64> {
    let text = input.trim().to_ascii_lowercase();
    let (number, scale) = [
        ("ghz", 1_000_000_000_u64),
        ("mhz", 1_000_000),
        ("khz", 1000),
        ("hz", 1),
    ]
    .into_iter()
    .find_map(|(suffix, scale)| text.strip_suffix(suffix).map(|n| (n.trim(), scale)))
    .unwrap_or((text.as_str(), 1));
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    ensure!(
        !whole.is_empty()
            && whole.bytes().all(|b| b.is_ascii_digit())
            && fraction.bytes().all(|b| b.is_ascii_digit()),
        "enter a frequency such as 443mhz, 145.252MHz, 500 kHz or integer Hz"
    );
    let fraction = fraction.trim_end_matches('0');
    let divisor = 10_u64
        .checked_pow(fraction.len().try_into()?)
        .context("frequency precision is too large")?;
    let fractional = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>()?
    };
    let scaled = fractional
        .checked_mul(scale)
        .context("frequency overflow")?;
    ensure!(scaled % divisor == 0, "frequency must resolve to whole Hz");
    whole
        .parse::<u64>()?
        .checked_mul(scale)
        .and_then(|v| v.checked_add(scaled / divisor))
        .context("frequency overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frequency_units_and_precision() {
        for (s, hz) in [
            ("443mhz", 443000000),
            ("145.252Mhz", 145252000),
            (" 500 kHz ", 500000),
            ("6GHz", 6000000000),
            ("433920000", 433920000),
            ("0.000001MHz", 1),
            ("1.00000000000000000000Hz", 1),
        ] {
            assert_eq!(parse_frequency(s).unwrap(), hz);
        }
        for s in [
            "",
            "-1MHz",
            "NaN",
            "1.1Hz",
            "1.2.3MHz",
            "18446744073709551615GHz",
            "1e6",
        ] {
            assert!(parse_frequency(s).is_err(), "{s}");
        }
    }
    #[test]
    fn settings_validate_atomically_and_default_old_configs() {
        let mut c: Config = toml::from_str("frequency = 433920000").unwrap();
        assert_eq!(c.fine_tune_hz, 500000);
        assert_eq!(c.coarse_tune_hz, 10000000);
        c.set("frequency", "145.252Mhz").unwrap();
        c.set("fine_tune_hz", "12.5kHz").unwrap();
        assert_eq!(c.frequency, 145252000);
        assert_eq!(c.fine_tune_hz, 12500);
        assert!(c.set("frequency", "7GHz").is_err());
        assert_eq!(c.frequency, 145252000);
        assert!(c.set("fine_tune_hz", "0Hz").is_err());
        assert_eq!(c.fine_tune_hz, 12500);
    }
}
