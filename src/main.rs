mod addons;
mod ai;
mod codec;
mod config;
mod dsp;
mod radio;
mod storage;
mod tui;
use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use config::Config;
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "thugsrf",
    version,
    author = "Kawaiipantsu · THUGS(red)",
    about = "THUGS(red) RF — radio signal intelligence workbench",
    after_help = "Run without a command for the TUI. All frequencies and sample rates are in Hz."
)]
pub struct Cli {
    #[arg(long, global = true)]
    device: Option<String>,
    #[arg(long, global = true)]
    frequency: Option<u64>,
    #[arg(long, global = true)]
    sample_rate: Option<u32>,
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Subcommand, Debug)]
enum Action {
    /// Open the responsive terminal workbench. Live reception starts with Space.
    Tui {
        #[arg(long)]
        demo: bool,
    },
    /// Inspect connected SDR and ALSA devices.
    Doctor,
    /// Print or edit user settings.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Capture RF IQ or audio WAV, plus metadata, without overwriting a file.
    Record {
        output: PathBuf,
        #[arg(long, default_value_t = 5)]
        seconds: u32,
    },
    /// Analyze recorded IQ/WAV; saves an investigation to SQLite.
    Analyze {
        input: PathBuf,
        #[arg(long, default_value = "cs8")]
        format: String,
        #[arg(long)]
        png: Option<PathBuf>,
    },
    /// Scan a finite frequency range with passive captures.
    Scan {
        #[arg(long)]
        start: u64,
        #[arg(long)]
        end: u64,
        #[arg(long)]
        step: u64,
        #[arg(long, default_value_t = 1)]
        seconds: u32,
    },
    /// Extract envelope pulses or FSK discriminator values.
    Decode {
        input: PathBuf,
        #[arg(long, default_value = "ook")]
        mode: String,
        #[arg(long, default_value = "cs8")]
        format: String,
    },
    /// Encode binary symbols as OOK IQ or Bell 202 AFSK audio. Does not transmit.
    Encode {
        output: PathBuf,
        #[arg(long)]
        bits: String,
        #[arg(long, default_value = "ook")]
        mode: String,
        #[arg(long, default_value_t = 1200)]
        baud: u32,
        #[arg(long, default_value_t = 48000)]
        rate: u32,
    },
    /// Demodulate centered AM/FM IQ into mono WAV.
    Demod {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "fm")]
        mode: String,
        #[arg(long, default_value = "cs8")]
        format: String,
    },
    /// Transmit a finite signed 8-bit IQ file with HackRF.
    Replay {
        input: PathBuf,
        #[arg(long)]
        confirm_tx: bool,
        #[arg(long, default_value_t = 0)]
        gain: u32,
    },
    /// Play WAV through an ALSA audio output.
    Play { input: PathBuf },
    /// Inspect, enable, or run user-installed addons.
    Addon {
        #[command(subcommand)]
        action: AddonAction,
    },
    /// Ask an LLM about measured IQ/WAV features or a spectrum image.
    Ai {
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long, default_value = "cs8")]
        format: String,
        #[arg(
            long,
            default_value = "Identify plausible signals and useful next measurements."
        )]
        question: String,
    },
    /// Show investigation history, or export a stored JSON report.
    History {
        #[arg(long)]
        export: Option<i64>,
    },
}
#[derive(Subcommand, Debug)]
enum ConfigAction {
    Set { key: String, value: String },
    Path,
}
#[derive(Subcommand, Debug)]
enum AddonAction {
    List,
    Toggle {
        name: String,
    },
    Run {
        name: String,
        input: PathBuf,
        #[arg(long, default_value = "cs8")]
        format: String,
    },
}
fn configured(cli: &Cli) -> Result<Config> {
    let mut c = Config::load()?;
    if let Some(d) = &cli.device {
        c.device = d.clone();
        if cli.sample_rate.is_none() {
            c.sample_rate = match d.as_str() {
                "rtl" => 2_400_000,
                "audio" => 48000,
                _ => 8_000_000,
            };
        }
    }
    if let Some(f) = cli.frequency {
        c.frequency = f;
    }
    if let Some(s) = cli.sample_rate {
        c.sample_rate = s;
    }
    c.validate()?;
    Ok(c)
}
fn execute(cli: Cli) -> Result<String> {
    let c = configured(&cli)?;
    match cli.command {
        None | Some(Action::Tui { .. }) => anyhow::bail!("TUI cannot be nested"),
        Some(Action::Doctor) => Ok(radio::doctor()),
        Some(Action::Config { action }) => match action {
            None => Ok(toml::to_string_pretty(&c)?),
            Some(ConfigAction::Path) => Ok(config::config_dir()
                .join("config.toml")
                .display()
                .to_string()),
            Some(ConfigAction::Set { key, value }) => {
                let mut c = c;
                c.set(&key, &value)?;
                c.save()?;
                Ok("Configuration saved".into())
            }
        },
        Some(Action::Record { output, seconds }) => radio::capture(&c, &output, seconds),
        Some(Action::Analyze { input, format, png }) => {
            let r = dsp::analyze_file(
                &input,
                &format,
                c.sample_rate,
                c.frequency,
                c.fft_size,
                c.threshold_db,
            )?;
            if let Some(p) = png {
                dsp::spectrum_png(&r, &p)?;
            }
            let id = storage::save(&r)?;
            Ok(serde_json::to_string_pretty(
                &serde_json::json!({"investigation_id":id,"report":r}),
            )?)
        }
        Some(Action::Decode {
            input,
            mode,
            format,
        }) => {
            let s = codec::decode(&input, &format, c.sample_rate, &mode)?;
            storage::finding("decode", &s)?;
            Ok(s)
        }
        Some(Action::Encode {
            output,
            bits,
            mode,
            baud,
            rate,
        }) => codec::encode(&output, &bits, rate, baud, &mode),
        Some(Action::Demod {
            input,
            output,
            mode,
            format,
        }) => codec::demod(&input, &output, &format, c.sample_rate, &mode),
        Some(Action::Replay {
            input,
            confirm_tx,
            gain,
        }) => {
            ensure!(
                confirm_tx,
                "RF transmission requires --confirm-tx; use a suitable test load or your authorized RF setup"
            );
            radio::replay(&c, &input, gain)
        }
        Some(Action::Play { input }) => {
            let reader = hound::WavReader::open(&input)?;
            let seconds = reader.duration() as u64 / reader.spec().sample_rate as u64 + 15;
            ensure!(seconds <= 3615, "audio playback limited to one hour");
            let mut cmd = std::process::Command::new("aplay");
            cmd.args(["-D", &c.audio_device]).arg(input);
            let (status, log) = radio::bounded_output(&mut cmd, seconds)?;
            ensure!(status.success(), "audio playback failed: {log}");
            Ok("Audio playback complete".into())
        }
        Some(Action::Addon { action }) => match action {
            AddonAction::List => Ok(addons::list()?
                .iter()
                .map(|a| {
                    format!(
                        "{} [{}] enabled={} — {}",
                        a.manifest.name,
                        a.manifest.kind,
                        a.manifest.enabled,
                        a.manifest.description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")),
            AddonAction::Toggle { name } => addons::toggle(&name),
            AddonAction::Run {
                name,
                input,
                format,
            } => {
                let r = addons::run(
                    &name,
                    &addons::request(&input, &format, c.sample_rate, c.frequency)?,
                )?;
                let s = serde_json::to_string_pretty(&r)?;
                storage::finding("addon", &s)?;
                Ok(s)
            }
        },
        Some(Action::Ai {
            input,
            image,
            format,
            question,
        }) => {
            ensure!(
                input.is_some() || image.is_some(),
                "supply --input IQ/WAV or --image PNG/JPEG"
            );
            let report = input
                .as_ref()
                .map(|p| {
                    dsp::analyze_file(
                        p,
                        &format,
                        c.sample_rate,
                        c.frequency,
                        c.fft_size,
                        c.threshold_db,
                    )
                })
                .transpose()?;
            let result = ai::analyze(&c, report.as_ref(), image.as_deref(), &question)?;
            storage::finding("ai-hypothesis", &result)?;
            Ok(result)
        }
        Some(Action::History { export }) => {
            if let Some(id) = export {
                storage::export(id)
            } else {
                storage::history()
            }
        }
        Some(Action::Scan {
            start,
            end,
            step,
            seconds,
        }) => {
            ensure!(
                c.device == "hackrf" || c.device == "rtl",
                "scan requires an SDR"
            );
            ensure!(start <= end && step > 0, "invalid scan range");
            ensure!(
                (end - start) / step < 1000,
                "scan limited to 1000 center frequencies"
            );
            let mut reports = Vec::new();
            let mut f = start;
            loop {
                ensure!(
                    !CANCELLED.load(std::sync::atomic::Ordering::Relaxed),
                    "scan cancelled"
                );
                let mut c = c.clone();
                c.frequency = f;
                c.validate()?;
                let p = config::data_dir().join("recordings").join(format!(
                    "scan-{}-{f}.{}",
                    stamp(),
                    if c.device == "rtl" { "cu8" } else { "cs8" }
                ));
                radio::capture(&c, &p, seconds)?;
                let r = dsp::analyze_file(
                    &p,
                    if c.device == "rtl" { "cu8" } else { "cs8" },
                    c.sample_rate,
                    f,
                    c.fft_size,
                    c.threshold_db,
                )?;
                let id = storage::save(&r)?;
                reports
                    .push(serde_json::json!({"id":id,"center_hz":f,"peaks":r.peaks,"recording":p}));
                if end - f < step {
                    break;
                }
                f += step;
            }
            Ok(serde_json::to_string_pretty(&reports)?)
        }
    }
}
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    ctrlc::set_handler(|| CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed))?;
    config::init()?;
    if cli.command.is_none() || matches!(cli.command, Some(Action::Tui { .. })) {
        let mut c = configured(&cli)?;
        if matches!(cli.command, Some(Action::Tui { demo: true })) {
            c.device = "demo".into();
        }
        tui::run(c)
    } else {
        println!("{}", execute(cli)?);
        Ok(())
    }
}
