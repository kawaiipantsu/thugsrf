mod addons;
mod ai;
mod codec;
mod config;
mod dsp;
mod listening;
mod radio;
mod storage;
mod survey;
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
    /// Listen to AM, narrow FM or broadcast FM through ALSA.
    Listen {
        #[arg(long, default_value = "fm")]
        mode: String,
        #[arg(long)]
        bandwidth: Option<u32>,
        #[arg(long, default_value_t = 3600)]
        seconds: u32,
    },
    /// Send microphone audio with HackRF; explicit finite transmission.
    Talk {
        #[arg(long)]
        confirm_tx: bool,
        #[arg(long, default_value = "fm")]
        mode: String,
        #[arg(long, default_value_t = 10)]
        seconds: u32,
        #[arg(long, default_value_t = 0.0)]
        ctcss: f64,
    },
    /// Decode RF/audio into real protocol packets for Wireshark.
    ExportPcap {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        protocol: String,
        #[arg(long, default_value = "auto")]
        format: String,
        #[arg(long, default_value = "{}")]
        options: String,
    },
    /// Sync or search SigID Wiki factual signal metadata in local SQLite.
    Sigid {
        #[command(subcommand)]
        action: SigidAction,
    },
    /// Import and search local frequency reference lists.
    Frequency {
        #[command(subcommand)]
        action: FrequencyAction,
    },
    /// Manage your local channel/repeater directory.
    Channels {
        #[command(subcommand)]
        action: Option<ChannelAction>,
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
    /// Run all enabled signal identifiers against a recording or packet capture.
    Identify {
        input: PathBuf,
        #[arg(long, default_value = "auto")]
        format: String,
        #[arg(long, default_value = "{}")]
        options: String,
    },
    /// Show investigation history, or export a stored JSON report.
    History {
        #[arg(long)]
        export: Option<i64>,
    },
}
#[derive(Subcommand, Debug)]
enum SigidAction {
    Sync,
    Status,
    Lookup {
        #[arg(long)]
        bandwidth: Option<f64>,
        #[arg(long)]
        modulation: Option<String>,
        #[arg(long)]
        mode: Option<String>,
        #[arg(long, default_value_t = 15)]
        limit: usize,
    },
}
#[derive(Subcommand, Debug)]
enum FrequencyAction {
    Sources,
    Import {
        input: PathBuf,
        #[arg(long, default_value = "http://www.dkscan.dk/frekvens.htm")]
        source: String,
        #[arg(long, default_value = "DK")]
        region: String,
    },
    Lookup {
        #[arg(long, default_value_t = 12500)]
        tolerance: u32,
        #[arg(long, default_value = "DK")]
        region: String,
    },
}
#[derive(Subcommand, Debug)]
enum ChannelAction {
    Set {
        index: usize,
        field: String,
        value: String,
    },
    Add {
        name: String,
        #[arg(long)]
        rx: u64,
        #[arg(long)]
        tx: Option<u64>,
        #[arg(long, default_value_t = 0.0)]
        ctcss: f64,
        #[arg(long, default_value = "fm")]
        mode: String,
        #[arg(long, default_value_t = 12500)]
        bandwidth: u32,
        #[arg(long, default_value = "User supplied")]
        source: String,
    },
    Remove {
        index: usize,
    },
}
#[derive(Subcommand, Debug)]
enum ConfigAction {
    Set { key: String, value: String },
    Path,
}
#[derive(Subcommand, Debug)]
enum AddonAction {
    /// Install bundled modules without overwriting user files.
    Install,
    List,
    Toggle {
        name: String,
    },
    Enable {
        name: String,
        #[arg(long)]
        kind: Option<String>,
    },
    Disable {
        name: String,
        #[arg(long)]
        kind: Option<String>,
    },
    Run {
        name: String,
        input: PathBuf,
        #[arg(long, default_value = "auto")]
        format: String,
        #[arg(long, default_value = "{}")]
        options: String,
    },
    RunEnabled {
        input: PathBuf,
        #[arg(long, default_value = "decoders")]
        kind: String,
        #[arg(long, default_value = "auto")]
        format: String,
        #[arg(long, default_value = "{}")]
        options: String,
    },
}
fn addon_request(
    input: &std::path::Path,
    format: &str,
    c: &Config,
    options: &str,
) -> Result<serde_json::Value> {
    let mut request = addons::request(input, format, c.sample_rate, c.frequency)?;
    let options: serde_json::Value = serde_json::from_str(options)?;
    ensure!(options.is_object(), "options must be a JSON object");
    request["options"] = options;
    Ok(request)
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
        Some(Action::Listen {
            mode,
            bandwidth,
            seconds,
        }) => {
            let mut c = c;
            c.listen_mode = mode;
            c.listen_bandwidth = bandwidth.unwrap_or(match c.listen_mode.as_str() {
                "wfm" => 200000,
                "am" => 10000,
                _ => 12500,
            });
            listening::run(&c, seconds, false, 0.0)
        }
        Some(Action::Talk {
            confirm_tx,
            mode,
            seconds,
            ctcss,
        }) => {
            ensure!(confirm_tx, "microphone transmission requires --confirm-tx");
            ensure!(
                cli.frequency.is_some(),
                "talk requires an explicit --frequency in Hz"
            );
            let mut c = c;
            c.listen_mode = mode;
            listening::run(&c, seconds, true, ctcss)
        }
        Some(Action::Channels { action }) => match action {
            None => Ok(serde_json::to_string_pretty(&listening::channels()?)?),
            Some(ChannelAction::Add {
                name,
                rx,
                tx,
                ctcss,
                mode,
                bandwidth,
                source,
            }) => listening::add(listening::Channel {
                name,
                rx_hz: rx,
                tx_hz: tx,
                ctcss_hz: ctcss,
                mode,
                bandwidth,
                source,
            }),
            Some(ChannelAction::Set {
                index,
                field,
                value,
            }) => {
                let mut rows = listening::channels()?;
                ensure!(index > 0 && index <= rows.len(), "index is 1-based");
                let row = &mut rows[index - 1];
                match field.as_str() {
                    "name" => row.name = value,
                    "source" => row.source = value,
                    "mode" => row.mode = value,
                    "rx_hz" => row.rx_hz = value.parse()?,
                    "tx_hz" => {
                        row.tx_hz = if value == "none" {
                            None
                        } else {
                            Some(value.parse()?)
                        }
                    }
                    "ctcss_hz" => row.ctcss_hz = value.parse()?,
                    "bandwidth" => row.bandwidth = value.parse()?,
                    _ => anyhow::bail!(
                        "field must be name, source, mode, rx_hz, tx_hz, ctcss_hz, or bandwidth"
                    ),
                }
                listening::save_channels(&rows)?;
                Ok("Channel updated".into())
            }
            Some(ChannelAction::Remove { index }) => {
                let mut rows = listening::channels()?;
                ensure!(index > 0 && index <= rows.len(), "index is 1-based");
                rows.remove(index - 1);
                listening::save_channels(&rows)?;
                Ok("Channel removed".into())
            }
        },
        Some(Action::Frequency { action }) => {
            let spec = match action {
                FrequencyAction::Sources => serde_json::json!({"action":"sources"}),
                FrequencyAction::Import {
                    input,
                    source,
                    region,
                } => {
                    serde_json::json!({"action":"import","path":input,"source":source,"region":region})
                }
                FrequencyAction::Lookup { tolerance, region } => {
                    serde_json::json!({"action":"lookup","hz":c.frequency,"tolerance":tolerance,"region":region})
                }
            };
            let result = std::process::Command::new("/usr/bin/python3")
                .args([
                    "-c",
                    include_str!("../addons/_shared/v0_2/frequencydb.py"),
                    &spec.to_string(),
                ])
                .output()?;
            ensure!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            Ok(String::from_utf8_lossy(&result.stdout).into_owned())
        }
        Some(Action::ExportPcap {
            input,
            output,
            protocol,
            format,
            options,
        }) => {
            let module = match protocol.as_str() {
                "ble" => "ble-advertising",
                "ax25" => "packet-radio",
                "gsm" => "gsm-bcch",
                _ => anyhow::bail!("protocol must be ble, ax25, or gsm"),
            };
            ensure!(!output.exists(), "output already exists");
            let mut request = addon_request(&input, &format, &c, &options)?;
            request["options"]["pcap"] = serde_json::json!(std::path::absolute(&output)?);
            let result = addons::run(module, &request)?;
            ensure!(
                result.get("status").and_then(|v| v.as_str()) != Some("error"),
                "{}",
                result
            );
            ensure!(
                result.get("pcap").is_some(),
                "decoder did not export PCAP: {result}"
            );
            storage::finding("pcap", &result.to_string())?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        Some(Action::Sigid { action }) => {
            let spec = match action {
                SigidAction::Sync => serde_json::json!({"action":"sync"}),
                SigidAction::Status => serde_json::json!({"action":"status"}),
                SigidAction::Lookup {
                    bandwidth,
                    modulation,
                    mode,
                    limit,
                } => {
                    serde_json::json!({"action":"lookup","hz":c.frequency,"bandwidth":bandwidth,"modulation":modulation,"mode":mode,"limit":limit})
                }
            };
            let result = std::process::Command::new("/usr/bin/python3")
                .args([
                    "-c",
                    include_str!("../addons/_shared/v0_2/sigid.py"),
                    &spec.to_string(),
                ])
                .output()?;
            ensure!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            Ok(String::from_utf8_lossy(&result.stdout).into_owned())
        }
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
            AddonAction::Install => addons::install(),
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
            AddonAction::Enable { name, kind } => addons::enable(&name, kind.as_deref(), true),
            AddonAction::Disable { name, kind } => addons::enable(&name, kind.as_deref(), false),
            AddonAction::RunEnabled {
                input,
                kind,
                format,
                options,
            } => {
                let r = addons::run_enabled(&kind, &addon_request(&input, &format, &c, &options)?)?;
                let s = serde_json::to_string_pretty(&r)?;
                storage::finding("addon-batch", &s)?;
                Ok(s)
            }
            AddonAction::Run {
                name,
                input,
                format,
                options,
            } => {
                let r = addons::run(&name, &addon_request(&input, &format, &c, &options)?)?;
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
        Some(Action::Identify {
            input,
            format,
            options,
        }) => {
            let r = addons::run_enabled(
                "identifiers",
                &addon_request(&input, &format, &c, &options)?,
            )?;
            let s = serde_json::to_string_pretty(&r)?;
            storage::finding("identification", &s)?;
            Ok(s)
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
