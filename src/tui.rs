use crate::{
    config::{self, Config},
    dsp::Report,
    radio::Stream,
};
use anyhow::{Result, ensure};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        *,
    },
};
use std::{
    collections::VecDeque,
    io::{self, IsTerminal},
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};
const RED: Color = Color::Rgb(244, 39, 67);
const CYAN: Color = Color::Rgb(120, 210, 219);
const BG: Color = Color::Rgb(9, 12, 19);
const MUTED: Color = Color::Rgb(127, 142, 160);
const WATERFALL_PALETTES: [&str; 5] = ["Classic", "Fire", "Ocean", "Green", "Grayscale"];
const TABS: [&str; 10] = [
    "Spectrum",
    "Detections",
    "Recordings",
    "Addons",
    "Settings",
    "Workbench",
    "Survey",
    "Listen",
    "VHF/UHF",
    "Decoder Console",
];
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}
struct App {
    c: Config,
    tab: usize,
    stream: Option<Stream>,
    survey: Option<crate::survey::Survey>,
    panorama: Option<crate::survey::Panorama>,
    audio: Option<crate::listening::Audio>,
    report: Option<Report>,
    water: VecDeque<Vec<f32>>,
    waterfall_palette: usize,
    waterfall_floor: f32,
    zoom: usize,
    view_center: Option<f64>,
    selected_frequency: Option<f64>,
    editing_bandwidth: bool,
    status: String,
    output: String,
    input: Option<String>,
    editing: Option<String>,
    tuning: bool,
    selected: usize,
    scroll: u16,
    busy: bool,
    reload_config: bool,
    job: mpsc::Receiver<String>,
    send: mpsc::Sender<String>,
    paused: bool,
    history: String,
    decoder_log: VecDeque<String>,
    decoder_log_file: Option<std::io::BufWriter<std::fs::File>>,
    decoder_log_path: Option<std::path::PathBuf>,
    decoders_enabled: bool,
    last_rds: String,
}
pub fn run(c: Config) -> Result<()> {
    ensure!(
        io::stdout().is_terminal() && io::stdin().is_terminal(),
        "TUI requires an interactive terminal; use doctor or analyze for headless operation"
    );
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (send, job) = mpsc::channel();
    let mut a = App {
        c,
        tab: 0,
        stream: None,
        survey: None,
        panorama: None,
        audio: None,
        report: None,
        water: VecDeque::new(),
        waterfall_palette: 0,
        waterfall_floor: -110.0,
        zoom: 1,
        view_center: None,
        selected_frequency: None,
        editing_bandwidth: false,
        status: "Ready · Space starts reception · : opens command bar".into(),
        output: help().into(),
        input: None,
        editing: None,
        tuning: false,
        selected: 0,
        scroll: 0,
        busy: false,
        reload_config: false,
        job,
        send,
        paused: false,
        history: crate::storage::history().unwrap_or_default(),
        decoder_log: VecDeque::new(),
        decoder_log_file: None,
        decoder_log_path: None,
        decoders_enabled: true,
        last_rds: String::new(),
    };
    while !crate::CANCELLED.load(Ordering::Relaxed) || a.busy {
        let mut rds_line = None;
        if let Some(audio) = &mut a.audio {
            if a.c.listen_mode == "wfm" {
                let summary = audio.rds_summary();
                if summary != a.last_rds {
                    let line = format!(
                        "{} {summary}",
                        crate::live::entry_prefix(
                            std::time::SystemTime::now(),
                            a.c.frequency,
                            "rds"
                        )
                    );
                    rds_line = Some(line);
                    a.last_rds = summary;
                }
            }
            match audio.finished() {
                Ok(Some(message)) => {
                    a.status = message;
                    a.audio = None;
                }
                Err(e) => {
                    a.status = "Audio failed · details in Workbench".into();
                    a.output = e.to_string();
                    a.tab = 5;
                    a.audio = None;
                }
                _ => {}
            }
        }
        if let Some(line) = rds_line {
            decoder_line(&mut a, line);
        }
        let mut sweep_end = None;
        if let Some(survey) = &a.survey {
            while let Ok(frame) = survey.frames.try_recv() {
                a.panorama = Some(frame);
            }
            while let Ok(message) = survey.events.try_recv() {
                sweep_end = Some(message);
            }
        }
        if let Some(message) = sweep_end {
            a.survey = None;
            a.status = message.clone();
            a.output = message;
        }
        let mut disconnected = false;
        let mut failure = None;
        let mut decoder_lines = Vec::new();
        if let Some(stream) = &a.stream {
            stream
                .decoders
                .enabled
                .store(a.decoders_enabled, Ordering::Relaxed);
            while let Ok(line) = stream.decoders.events.try_recv() {
                decoder_lines.push(line);
            }
            loop {
                match stream.frames.try_recv() {
                    Ok(r) => {
                        if !a.paused {
                            a.water.push_front(r.spectrum_dbfs.clone());
                            a.water.truncate(200);
                            a.report = Some(r);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            while let Ok(event) = stream.events.try_recv() {
                match event {
                    crate::radio::StreamEvent::Status(message) => a.status = message,
                    crate::radio::StreamEvent::Failed(detail) => failure = Some(detail),
                }
            }
        }
        for line in decoder_lines {
            decoder_line(&mut a, line);
        }
        if let Some(detail) = failure {
            a.stream = None;
            a.status = "Receiver failed · full diagnostics below · Space retries".into();
            a.output = format!(
                "Receiver diagnostics\n\n{detail}\n\nCheck USB connection and whether another application is using the radio.\nPress Space to retry reception."
            );
            a.tab = 5;
            a.scroll = 0;
        } else if disconnected {
            a.stream = None;
            a.status = "Receiver ended · Space restarts reception".into();
        }
        while let Ok(s) = a.job.try_recv() {
            a.busy = false;
            a.output = s;
            a.tab = 5;
            a.scroll = 0;
            a.status = "Job finished · result in Workbench".into();
            a.history = crate::storage::history().unwrap_or_default();
            if a.reload_config
                && let Ok(c) = Config::load()
            {
                a.c = c;
            }
        }
        terminal.draw(|f| draw(f, &a))?;
        if event::poll(Duration::from_millis(40))? {
            let event = event::read()?;
            if let Event::Mouse(mouse) = event {
                let size = terminal.size()?;
                spectrum_mouse(&mut a, mouse, Rect::new(0, 0, size.width, size.height));
                continue;
            }
            let Event::Key(k) = event else {
                continue;
            };
            if k.kind == KeyEventKind::Release {
                continue;
            }
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                crate::CANCELLED.store(true, Ordering::Relaxed);
                a.status = "Cancelling active operation…".into();
                continue;
            }
            if let Some(input) = &mut a.input {
                match k.code {
                    KeyCode::Esc => {
                        a.input = None;
                        a.editing = None;
                        a.tuning = false;
                        a.editing_bandwidth = false;
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Char(c) => input.push(c),
                    KeyCode::Enter => {
                        let text = a.input.take().unwrap_or_default();
                        if std::mem::take(&mut a.editing_bandwidth) {
                            match config::parse_frequency(&text)
                                .and_then(|hz| Ok(u32::try_from(hz)?))
                            {
                                Ok(width) => {
                                    let mut next = a.c.clone();
                                    next.listen_bandwidth = width;
                                    set_listening_config(&mut a, next);
                                }
                                Err(e) => a.status = e.to_string(),
                            }
                        } else if std::mem::take(&mut a.tuning) {
                            match config::parse_frequency(&text) {
                                Ok(frequency) => set_spectrum_frequency(&mut a, frequency),
                                Err(e) => a.status = e.to_string(),
                            }
                        } else if let Some(key) = a.editing.take() {
                            let mut next = a.c.clone();
                            let r = next.set(&key, &text).and_then(|_| next.save());
                            match r {
                                Ok(()) => {
                                    a.c = next;
                                    a.stream = None;
                                    a.survey = None;
                                    a.audio = None;
                                    a.status = "Settings saved; Space restarts reception".into();
                                }
                                Err(e) => a.status = e.to_string(),
                            }
                        } else {
                            run_command(&mut a, text);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            match k.code {
                KeyCode::Char('d') => {
                    a.tab = 9;
                    a.scroll = 0;
                }
                KeyCode::Char('D') => {
                    a.decoders_enabled = !a.decoders_enabled;
                    let notice = if a.decoders_enabled {
                        "Live addon decoders enabled · Space starts RX"
                    } else {
                        "Live addon decoders paused"
                    };
                    decoder_line(&mut a, notice.into());
                }
                KeyCode::Char('s') if a.tab == 9 => toggle_decoder_log(&mut a),
                KeyCode::Char('s') if a.tab == 0 => {
                    if let Some(report) = &a.report {
                        let (first, end) = visible_bins(&a, report.spectrum_dbfs.len());
                        a.status = match crate::export::spectrum_ascii(
                            report,
                            &a.water,
                            first,
                            end,
                            a.waterfall_floor,
                        ) {
                            Ok((graph, waterfall)) => format!(
                                "ASCII saved in {}: {} + {}",
                                config::config_dir().display(),
                                graph.file_name().unwrap().to_string_lossy(),
                                waterfall.file_name().unwrap().to_string_lossy()
                            ),
                            Err(e) => format!("Spectrum export failed: {e}"),
                        };
                    } else {
                        a.status = "Start reception before exporting the spectrum".into();
                    }
                }
                KeyCode::Char('x') if a.tab == 9 => {
                    a.decoder_log.clear();
                    a.scroll = 0;
                }
                KeyCode::Char('q') => {
                    if a.busy {
                        a.status = "Wait for the active job to finish before quitting".into();
                    } else {
                        break;
                    }
                }
                KeyCode::Tab => {
                    a.tab = (a.tab + 1) % TABS.len();
                    a.selected = 0;
                    a.scroll = 0;
                }
                KeyCode::BackTab => {
                    a.tab = (a.tab + TABS.len() - 1) % TABS.len();
                    a.selected = 0;
                    a.scroll = 0;
                }
                KeyCode::Char(c @ '1'..='9') => {
                    a.tab = c as usize - '1' as usize;
                    a.selected = 0;
                    a.scroll = 0;
                }
                KeyCode::Char('l') => {
                    let frequency = a
                        .selected_frequency
                        .filter(|_| a.tab == 0)
                        .map_or(a.c.frequency, |hz| hz.round().max(0.) as u64);
                    run_command(&mut a, format!("frequency lookup --frequency {frequency}"));
                }
                KeyCode::Char(':') => a.input = Some(String::new()),
                KeyCode::Char(' ') => {
                    if a.busy {
                        a.status = "Wait for active job before starting receiver".into();
                    } else if a.tab == 6 {
                        a.stream = None;
                        a.audio = None;
                        if a.survey.is_some() {
                            a.survey = None;
                            a.status = "Sweep stopped".into();
                        } else {
                            match crate::survey::Survey::start(&a.c) {
                                Ok(s) => {
                                    a.survey = Some(s);
                                    a.panorama = None;
                                    a.status =
                                        "Sweeping sequentially · red peak hold / cyan latest bins"
                                            .into();
                                }
                                Err(e) => a.status = e.to_string(),
                            }
                        }
                    } else if a.tab == 7 || a.tab == 8 {
                        a.stream = None;
                        a.survey = None;
                        if a.audio.is_some() {
                            a.audio = None;
                            a.status = "Listening stopped".into();
                        } else {
                            match crate::listening::Audio::start(&a.c, 3600, false, 0.0) {
                                Ok(s) => {
                                    a.audio = Some(s);
                                    a.status = format!(
                                        "Listening {} · {:.6} MHz · Space stops",
                                        a.c.listen_mode,
                                        a.c.frequency as f64 / 1e6
                                    );
                                }
                                Err(e) => a.status = e.to_string(),
                            }
                        }
                    } else if a.stream.is_some() {
                        a.stream = None;
                        a.status = "Receiver stopped".into();
                    } else {
                        a.survey = None;
                        a.audio = None;
                        match Stream::start(a.c.clone(), a.decoders_enabled) {
                            Ok(s) => {
                                a.report = None;
                                a.water.clear();
                                a.stream = Some(s);
                                a.status = format!(
                                    "Starting {} · {:.3} MHz",
                                    a.c.device,
                                    a.c.frequency as f64 / 1e6
                                );
                            }
                            Err(e) => a.status = e.to_string(),
                        }
                    }
                }
                KeyCode::Char('w') if a.tab == 2 || a.tab == 3 => {
                    a.input =
                        Some("export-pcap 'recording.cs8' 'packets.pcap' --protocol ble".into())
                }
                KeyCode::Char('f') if a.tab == 0 => {
                    if a.busy || a.audio.is_some() || a.survey.is_some() {
                        a.status = "Stop the active job, listening or survey before changing FFT resolution".into();
                    } else {
                        let mut next = a.c.clone();
                        next.fft_size = [2048, 8192, 32768, 65536]
                            .into_iter()
                            .find(|&n| n > a.c.fft_size)
                            .unwrap_or(2048);
                        let (selection, view) = (a.selected_frequency, a.view_center);
                        apply_spectrum_config(&mut a, next);
                        a.selected_frequency = selection;
                        a.view_center = view;
                        a.zoom = a.zoom.min(a.c.fft_size / 8);
                        a.status = format!(
                            "FFT {} bins · {:.1} Hz/bin · f cycles resolution",
                            a.c.fft_size,
                            a.c.sample_rate as f64 / a.c.fft_size as f64
                        );
                    }
                }
                KeyCode::Char(']') if a.tab == 0 => zoom_spectrum(&mut a, true),
                KeyCode::Char('[') if a.tab == 0 => zoom_spectrum(&mut a, false),
                KeyCode::Char('0') if a.tab == 0 => {
                    a.zoom = 1;
                    a.view_center = None;
                    a.status = "Full captured span".into();
                }
                KeyCode::Char('t') if a.tab == 0 => {
                    if let Some(frequency) = a.selected_frequency {
                        set_spectrum_frequency(&mut a, frequency.round().max(0.) as u64);
                    } else {
                        a.status = "Click a peak to select a frequency first".into();
                    }
                }
                KeyCode::Char('m') if a.tab == 0 || a.tab == 7 => {
                    let mut next = a.c.clone();
                    let (mode, width) = match next.listen_mode.as_str() {
                        "nfm" => ("fm", 50000),
                        "fm" => ("wfm", 200000),
                        "wfm" => ("am", 10000),
                        _ => ("nfm", 12500),
                    };
                    next.listen_mode = mode.into();
                    next.listen_bandwidth = width;
                    set_listening_config(&mut a, next);
                }
                KeyCode::Char('b') if a.tab == 0 || a.tab == 7 => {
                    a.editing_bandwidth = true;
                    a.input = Some(String::new());
                }
                KeyCode::Char('a') if a.tab == 0 => toggle_listening(&mut a),
                KeyCode::Char('c') if a.tab == 0 => {
                    a.waterfall_palette = (a.waterfall_palette + 1) % WATERFALL_PALETTES.len();
                    a.status = format!(
                        "Waterfall palette: {}",
                        WATERFALL_PALETTES[a.waterfall_palette]
                    );
                }
                KeyCode::Char(key @ ('+' | '=' | '-')) if a.tab == 0 => {
                    let step = if key == '-' { -5.0 } else { 5.0 };
                    a.waterfall_floor = (a.waterfall_floor + step).clamp(-150.0, -20.0);
                    a.status = format!(
                        "Waterfall threshold: {:.0} dBFS · + hides weaker signals · - reveals weaker signals",
                        a.waterfall_floor
                    );
                }
                KeyCode::Char('p') => a.paused = !a.paused,
                KeyCode::Char('s') => {
                    if a.tab == 6 {
                        if let Some(p) = &a.panorama {
                            let result = serde_json::json!({"acquisition":"sequential sweep, bins have different acquisition times","panorama":p});
                            a.status = match crate::storage::finding("survey", &result.to_string())
                            {
                                Ok(()) => "Saved sequential survey to SQLite findings".into(),
                                Err(e) => e.to_string(),
                            };
                            a.history = crate::storage::history().unwrap_or_default();
                        }
                    } else if let Some(r) = &a.report {
                        a.status = match crate::storage::save(r) {
                            Ok(id) => format!("Saved investigation #{id}"),
                            Err(e) => e.to_string(),
                        };
                        a.history = crate::storage::history().unwrap_or_default();
                    }
                }
                KeyCode::Char('r') => {
                    let ext = if a.c.device == "audio" {
                        "wav"
                    } else if a.c.device == "rtl" {
                        "cu8"
                    } else {
                        "cs8"
                    };
                    a.input = Some(format!(
                        "record '{}' --seconds 5",
                        config::data_dir()
                            .join("recordings")
                            .join(format!("capture-{}.{}", crate::stamp(), ext))
                            .display()
                    ));
                }
                key @ (KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown)
                    if a.tab == 0 =>
                {
                    let step = if matches!(key, KeyCode::Left | KeyCode::Right) {
                        a.c.fine_tune_hz
                    } else {
                        a.c.coarse_tune_hz
                    };
                    let increase = matches!(key, KeyCode::Right | KeyCode::Up | KeyCode::PageUp);
                    tune_spectrum(&mut a, step, increase);
                }
                KeyCode::Down => {
                    a.selected = a.selected.saturating_add(1);
                    a.scroll = a.scroll.saturating_add(1);
                }
                KeyCode::Up => {
                    a.selected = a.selected.saturating_sub(1);
                    a.scroll = a.scroll.saturating_sub(1);
                }
                KeyCode::PageDown => a.scroll = a.scroll.saturating_add(10),
                KeyCode::PageUp => a.scroll = a.scroll.saturating_sub(10),
                KeyCode::Enter if a.tab == 0 => {
                    a.tuning = true;
                    a.input = Some(String::new());
                }
                KeyCode::Enter if a.tab == 7 || a.tab == 8 => {
                    let channels = if a.tab == 7 {
                        Ok(crate::listening::presets())
                    } else {
                        crate::listening::channels()
                    };
                    match channels {
                        Ok(rows) if !rows.is_empty() => {
                            let row = &rows[a.selected % rows.len()];
                            let mut c = a.c.clone();
                            c.frequency = row.rx_hz;
                            c.listen_mode = row.mode.clone();
                            c.listen_bandwidth = row.bandwidth;
                            match c.validate() {
                                Ok(()) => {
                                    a.audio = None;
                                    a.stream = None;
                                    a.survey = None;
                                    a.audio = None;
                                    a.survey = None;
                                    a.c = c;
                                    a.status = format!("Tuned {} · Space listens", row.name);
                                }
                                Err(e) => a.status = e.to_string(),
                            }
                        }
                        Err(e) => a.status = e.to_string(),
                        _ => {}
                    }
                }
                KeyCode::Char('t') if a.tab == 8 => {
                    if let Ok(rows) = crate::listening::channels()
                        && !rows.is_empty()
                    {
                        let row = &rows[a.selected % rows.len()];
                        if let Some(tx) = row.tx_hz {
                            a.input = Some(format!(
                                "talk --frequency {tx} --mode {} --seconds 10 --ctcss {}",
                                row.mode, row.ctcss_hz
                            ));
                            a.status =
                                "TX preparation · append --confirm-tx to transmit microphone audio"
                                    .into();
                        } else {
                            a.status = "No TX frequency configured".into();
                        }
                    }
                }
                KeyCode::Char('e') if a.tab == 8 => {
                    if let Ok(rows) = crate::listening::channels()
                        && !rows.is_empty()
                    {
                        let index = a.selected % rows.len();
                        a.input = Some(format!(
                            "channels set {} rx_hz {}",
                            index + 1,
                            rows[index].rx_hz
                        ));
                        a.status="Edit field/value: name, rx_hz, tx_hz, ctcss_hz, mode, bandwidth, source".into();
                    }
                }
                KeyCode::Char('a') if a.tab == 8 => {
                    a.input=Some("channels add 'New repeater' --rx 145600000 --tx 145000000 --ctcss 0 --source 'User supplied'".into());
                }
                KeyCode::Char('i') if a.tab == 3 => {
                    a.input = Some("identify 'recording.cs8'".into())
                }
                KeyCode::Enter if a.tab == 4 => {
                    let fields = settings(&a.c);
                    let (k, v) = &fields[a.selected % fields.len()];
                    a.editing = Some(k.clone());
                    a.input = Some(v.clone());
                }
                KeyCode::Enter if a.tab == 3 => {
                    if let Ok(modules) = crate::addons::list()
                        && !modules.is_empty()
                    {
                        a.status = crate::addons::toggle(
                            &modules[a.selected % modules.len()].manifest.name,
                        )
                        .unwrap_or_else(|e| e.to_string());
                    }
                }
                KeyCode::Char('?') => {
                    a.tab = 5;
                    a.output = help().into();
                    a.scroll = 0;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
fn decoder_line(a: &mut App, line: String) {
    use std::io::Write;
    if let Some(file) = &mut a.decoder_log_file
        && let Err(error) = writeln!(file, "{line}").and_then(|_| file.flush())
    {
        a.decoder_log_file = None;
        a.status = format!("Decoder log write failed: {error}");
        crate::live::append(&mut a.decoder_log, a.status.clone());
    }
    crate::live::append(&mut a.decoder_log, line);
}

fn toggle_decoder_log(a: &mut App) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if a.decoder_log_file.is_some() {
        decoder_line(a, "Decoder log saving stopped".into());
        if let Some(mut file) = a.decoder_log_file.take() {
            if let Err(error) = file.flush() {
                a.status = format!("Decoder log flush failed: {error}");
                return;
            }
            a.status = format!(
                "Saved {}",
                a.decoder_log_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            );
        }
        return;
    }
    let path = config::config_dir().join(format!("decoder-output-{}.log", crate::stamp()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
    {
        Ok(file) => {
            a.decoder_log_file = Some(std::io::BufWriter::new(file));
            a.decoder_log_path = Some(path.clone());
            a.status = format!("Saving live decoder output to {} · s stops", path.display());
            decoder_line(a, a.status.clone());
        }
        Err(error) => a.status = format!("Cannot start decoder log: {error}"),
    }
}

fn main_rows(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(area)
}

fn spectrum_areas(area: Rect) -> [Rect; 3] {
    let cols = Layout::horizontal([
        Constraint::Min(50),
        Constraint::Length(if area.width >= 130 { 34 } else { 0 }),
    ])
    .split(area);
    let plot =
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).split(cols[0]);
    [plot[0], plot[1], cols[1]]
}

fn visible_bins(a: &App, n: usize) -> (usize, usize) {
    let count = (n / a.zoom).max(8).min(n);
    let center = a.report.as_ref().map_or(a.c.frequency, |r| r.center_hz) as f64;
    let rate = a.report.as_ref().map_or(a.c.sample_rate, |r| r.sample_rate) as f64;
    let position = (((a.view_center.unwrap_or(center) - center) / rate + 0.5) * n as f64)
        .round()
        .clamp(0., n as f64) as usize;
    let first = position.saturating_sub(count / 2).min(n - count);
    (first, first + count)
}

fn zoom_spectrum(a: &mut App, increase: bool) {
    let n = a
        .report
        .as_ref()
        .map_or(a.c.fft_size, |r| r.spectrum_dbfs.len());
    a.zoom = if increase {
        (a.zoom * 2).min((n / 8).max(1))
    } else {
        (a.zoom / 2).max(1)
    };
    if a.selected_frequency.is_some() {
        a.view_center = a.selected_frequency;
    }
    if a.zoom == 1 {
        a.view_center = None;
    }
    a.status = format!(
        "Spectrum zoom {}× · [] or mouse wheel · 0 full span · f FFT resolution",
        a.zoom
    );
}

fn spectrum_mouse(a: &mut App, mouse: MouseEvent, screen: Rect) {
    if a.tab != 0 || a.input.is_some() || screen.width < 80 || screen.height < 24 {
        return;
    }
    let areas = spectrum_areas(main_rows(screen)[2]);
    let position = Position::new(mouse.column, mouse.row);
    let Some(area) = areas[..2]
        .iter()
        .map(|r| panel("").inner(*r))
        .find(|r| r.contains(position))
    else {
        return;
    };
    match mouse.kind {
        MouseEventKind::ScrollUp => zoom_spectrum(a, true),
        MouseEventKind::ScrollDown => zoom_spectrum(a, false),
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(report) = &a.report else {
                a.status = "Space starts reception; select a peak once samples arrive".into();
                return;
            };
            let n = report.spectrum_dbfs.len();
            if n < 2 {
                return;
            }
            let (first, end) = visible_bins(a, n);
            let column = (mouse.column - area.x) as usize;
            let width = area.width as usize;
            let (lo, hi) = column_bins(first, end, column, width);
            let bin = (lo..hi)
                .max_by(|&x, &y| report.spectrum_dbfs[x].total_cmp(&report.spectrum_dbfs[y]))
                .unwrap_or(lo);
            a.selected_frequency = Some(
                report.center_hz as f64 + (bin as f64 / n as f64 - 0.5) * report.sample_rate as f64,
            );
            a.status = format!(
                "{} · t tune · l lookup",
                selected_signal(a)
                    .unwrap_or_default()
                    .lines()
                    .nth(1)
                    .unwrap_or_default()
            );
        }
        _ => {}
    }
}

fn column_bins(first: usize, end: usize, column: usize, width: usize) -> (usize, usize) {
    let span = end - first;
    let width = width.max(1);
    let lo = first + column.min(width - 1) * span / width;
    let hi = (first + (column.min(width - 1) + 1) * span / width)
        .max(lo + 1)
        .min(end);
    (lo, hi)
}

fn waterfall_power(
    row: Option<&Vec<f32>>,
    first: usize,
    end: usize,
    column: usize,
    width: usize,
) -> f32 {
    let Some(row) = row else {
        return -160.;
    };
    let (lo, hi) = column_bins(first, end, column, width);
    row.get(lo..hi)
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(-160., f32::max)
}

fn selected_signal(a: &App) -> Option<String> {
    let hz = a.selected_frequency?;
    let report = a.report.as_ref()?;
    let n = report.spectrum_dbfs.len();
    let resolution = report.sample_rate as f64 / n as f64;
    let bin = (((hz - report.center_hz as f64) / report.sample_rate as f64 + 0.5) * n as f64)
        .round() as usize;
    let power = *report.spectrum_dbfs.get(bin)?;
    let peak = report
        .peaks
        .iter()
        .filter(|p| (p.frequency_hz - hz).abs() <= (p.bandwidth_hz / 2.).max(resolution))
        .min_by(|x, y| {
            (x.frequency_hz - hz)
                .abs()
                .total_cmp(&(y.frequency_hz - hz).abs())
        });
    Some(format!(
        "{:.6} MHz · {:.1} dBFS · SNR {:.1} dB\n{} · FFT {:.0} Hz/bin · {}",
        hz / 1e6,
        power,
        power - report.noise_dbfs,
        peak.map_or_else(
            || "No detected peak".into(),
            |p| format!("Peak BW ≈{:.1} kHz", p.bandwidth_hz / 1000.)
        ),
        resolution,
        crate::dsp::band_context(hz)
    ))
}

fn toggle_listening(a: &mut App) {
    if a.busy || a.survey.is_some() {
        a.status = "Stop the active job or survey before listening".into();
        return;
    }
    if a.audio.is_some() {
        a.audio = None;
        a.status = "Listening stopped · Space resumes spectrum".into();
        return;
    }
    a.stream = None;
    match crate::listening::Audio::start(&a.c, 3600, false, 0.) {
        Ok(audio) => {
            a.audio = Some(audio);
            a.status = "Listening · spectrum held · a stops audio · Space resumes spectrum".into();
        }
        Err(e) => a.status = e.to_string(),
    }
}

fn set_listening_config(a: &mut App, next: Config) {
    if a.busy || a.survey.is_some() {
        a.status = "Stop the active job or survey before changing receive mode".into();
        return;
    }
    if let Err(e) = next.validate() {
        a.status = e.to_string();
        return;
    }
    let running = a.audio.is_some();
    a.audio = None;
    a.c = next;
    a.status = format!(
        "Receive {} · {:.1} kHz bandwidth · session only · a listens",
        a.c.listen_mode.to_uppercase(),
        a.c.listen_bandwidth as f64 / 1000.
    );
    if running {
        match crate::listening::Audio::start(&a.c, 3600, false, 0.) {
            Ok(audio) => a.audio = Some(audio),
            Err(e) => a.status = e.to_string(),
        }
    }
}

fn tune_spectrum(a: &mut App, step: u64, increase: bool) {
    let frequency = if increase {
        a.c.frequency.checked_add(step)
    } else {
        a.c.frequency.checked_sub(step)
    };
    let Some(frequency) = frequency else {
        a.status = "Tuning would exceed the frequency range".into();
        return;
    };
    set_spectrum_frequency(a, frequency);
}

fn set_spectrum_frequency(a: &mut App, frequency: u64) {
    if a.busy || a.audio.is_some() || a.survey.is_some() {
        a.status = "Stop the active job, listening or survey before tuning Spectrum".into();
        return;
    }
    if a.c.device == "audio" {
        a.status = "Audio input has no RF tuning frequency".into();
        return;
    }
    let mut next = a.c.clone();
    next.frequency = frequency;
    apply_spectrum_config(a, next);
}

fn apply_spectrum_config(a: &mut App, next: Config) {
    let frequency = next.frequency;
    if let Err(e) = next.validate() {
        a.status = e.to_string();
        return;
    }
    // Drop joins the old receiver before a replacement opens the USB device.
    let running = a.stream.is_some();
    a.stream = None;
    a.c = next;
    a.view_center = None;
    a.selected_frequency = None;
    a.report = None;
    a.water.clear();
    a.status = format!(
        "Tuned {:.6} MHz · {}",
        frequency as f64 / 1e6,
        if running {
            "restarting RX"
        } else {
            "Space starts RX"
        }
    );
    if running {
        match Stream::start(a.c.clone(), a.decoders_enabled) {
            Ok(stream) => a.stream = Some(stream),
            Err(e) => a.status = e.to_string(),
        }
    }
}

fn run_command(a: &mut App, text: String) {
    if a.busy {
        a.status = "A job is already running".into();
        return;
    }
    let Some(mut args) = shlex::split(&text) else {
        a.status = "Unclosed quote".into();
        return;
    };
    args.insert(0, "thugsrf".into());
    match crate::Cli::try_parse_from(args) {
        Ok(mut cli) => {
            a.reload_config = matches!(
                cli.command,
                Some(crate::Action::Config {
                    action: Some(crate::ConfigAction::Set { .. })
                })
            );
            let inherits_device = cli.device.is_none();
            if cli.device.is_none() {
                cli.device = Some(a.c.device.clone());
            }
            if cli.frequency.is_none() {
                cli.frequency = Some(a.c.frequency);
            }
            if cli.sample_rate.is_none() && inherits_device {
                cli.sample_rate = Some(a.c.sample_rate);
            }
            a.stream = None;
            a.survey = None;
            a.audio = None;
            a.busy = true;
            a.status = format!("Running: {text}");
            let tx = a.send.clone();
            std::thread::spawn(move || {
                let result = crate::execute(cli).unwrap_or_else(|e| format!("Error: {e:#}"));
                let _ = tx.send(result);
            });
        }
        Err(e) => {
            a.output = e.to_string();
            a.tab = 5;
        }
    }
}
fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .title(title)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(54, 66, 84)))
        .style(Style::default().bg(BG))
}
fn draw(f: &mut Frame, a: &App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(Color::White)),
        area,
    );
    if area.width < 80 || area.height < 24 {
        f.render_widget(
            Paragraph::new("THUGS(red) RF\nResize to at least 80 × 24\nq: quit")
                .block(panel(" Terminal too small ")),
            area,
        );
        return;
    }
    let rows = main_rows(area);
    let title = Line::from(vec![
        Span::styled("  ))) thugs", Style::default().fg(Color::White).bold()),
        Span::styled("rf_", Style::default().fg(RED).bold()),
        Span::styled(
            "   THUGS(red)  /  SIGNAL INTELLIGENCE",
            Style::default().fg(MUTED),
        ),
    ]);
    f.render_widget(
        Paragraph::new(title).block(Block::bordered().border_style(Style::default().fg(RED))),
        rows[0],
    );
    let first = if area.width < 125 {
        a.tab.saturating_sub(2).min(TABS.len() - 4)
    } else {
        0
    };
    let visible = if area.width < 125 { 4 } else { TABS.len() };
    f.render_widget(
        Tabs::new(
            TABS.iter()
                .enumerate()
                .skip(first)
                .take(visible)
                .map(|(i, t)| format!("{} {t}", i + 1)),
        )
        .select(a.tab - first)
        .highlight_style(Style::default().fg(RED).bold())
        .divider("│"),
        rows[1],
    );
    match a.tab {
        0 => spectrum(f, rows[2], a),
        1 => {
            let text = if let Some(r) = &a.report {
                let mut t = format!(
                    "Source: {}   RMS {:.1} dBFS   noise {:.1} dBFS\n\n",
                    r.source, r.rms_dbfs, r.noise_dbfs
                );
                for p in &r.peaks {
                    t += &format!(
                        "{:>12.6} MHz  {:>6.1} dBFS  SNR {:>5.1}  BW {:>8.0} Hz\n  {}\n",
                        p.frequency_hz / 1e6,
                        p.power_dbfs,
                        p.snr_db,
                        p.bandwidth_hz,
                        p.context
                    );
                }
                t + "\nContext hints are hypotheses, not decoded protocols.\nUse : decode / addon run / ai for deeper inspection."
            } else {
                "Start reception with Space. Press s to save an investigation.".into()
            };
            f.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .scroll((a.scroll, 0))
                    .block(panel(" Detections / evidence ")),
                rows[2],
            );
        }
        2 => {
            let files = std::fs::read_dir(config::data_dir().join("recordings"))
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .take(100)
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            f.render_widget(Paragraph::new(format!("r: prepare recording command · : analyze <path> · : history --export ID\n\n{}\n\nInvestigations\n{}",files,a.history)).wrap(Wrap{trim:false}).scroll((a.scroll,0)).block(panel(" Recordings / SQLite history ")),rows[2]);
        }
        3 => match crate::addons::list() {
            Ok(modules) if !modules.is_empty() => {
                let mut state =
                    ListState::default().with_selected(Some(a.selected % modules.len()));
                let items = modules
                    .iter()
                    .map(|m| {
                        ListItem::new(format!(
                            "{} [{}] {}\n  {}\n  Formats: {} · requires: {}",
                            if m.manifest.enabled { "ON " } else { "OFF" },
                            m.manifest.kind,
                            m.manifest.name,
                            m.manifest.description,
                            m.manifest.formats.join(","),
                            m.manifest.requires.join(",")
                        ))
                    })
                    .collect::<Vec<_>>();
                f.render_stateful_widget(
                    List::new(items)
                        .highlight_style(Style::default().fg(CYAN))
                        .highlight_symbol("▶ ")
                        .block(panel(
                            " Addons · Enter enable live/file decoder · d console ",
                        )),
                    rows[2],
                    &mut state,
                );
            }
            other => f.render_widget(
                Paragraph::new(match other {
                    Err(e) => e.to_string(),
                    _ => "Install modules with : addon install".into(),
                })
                .block(panel(" Addons ")),
                rows[2],
            ),
        },
        9 => {
            let state = if !a.decoders_enabled {
                "PAUSED"
            } else if a.stream.is_some() {
                "LIVE RX"
            } else if a.audio.is_some() {
                "LISTENING · WFM RDS only"
            } else {
                "WAITING FOR RX"
            };
            let text = if a.decoder_log.is_empty() {
                "Enable decoders in Addons (4), then Space starts reception.\nAll compatible enabled decoders share this rolling console.\nD pauses/resumes decoding · s toggles log saving · x clears · ↑↓/Pg scroll\nLatest entries first; up to 500 entries. Spectrum remains available in panel 1.".into()
            } else {
                a.decoder_log
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };
            f.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .scroll((a.scroll, 0))
                    .block(panel(&format!(
                        " Decoder Console · {state} · {} · s save · D pause · x clear ",
                        if a.decoder_log_file.is_some() {
                            "SAVING"
                        } else {
                            "not saving"
                        }
                    ))),
                rows[2],
            );
        }
        6 => survey_view(f, rows[2], a),
        7 | 8 => listening_view(f, rows[2], a),
        4 => {
            let fields = settings(&a.c);
            let mut state = ListState::default().with_selected(Some(a.selected % fields.len()));
            let items = fields
                .into_iter()
                .map(|(k, v)| ListItem::new(format!("{k:<18} {v}")))
                .collect::<Vec<_>>();
            f.render_stateful_widget(
                List::new(items)
                    .highlight_style(Style::default().fg(CYAN).bg(Color::Rgb(30, 38, 50)))
                    .highlight_symbol("▶ ")
                    .block(panel(
                        " Settings · ↑↓ select · Enter edit · validates before save ",
                    )),
                rows[2],
                &mut state,
            );
        }
        _ => f.render_widget(
            Paragraph::new(a.output.as_str())
                .wrap(Wrap { trim: false })
                .scroll((a.scroll, 0))
                .block(panel(" Workbench · : command · ↑↓ / PgUp PgDn scroll ")),
            rows[2],
        ),
    }
    let status = if let Some(input) = &a.input {
        let label = if a.editing_bandwidth {
            "Receive bandwidth (e.g. 12.5kHz; Enter apply, Esc cancel)"
        } else if a.tuning {
            "Frequency (e.g. 145.252MHz; Enter tune, Esc cancel)"
        } else {
            a.editing.as_deref().unwrap_or("command")
        };
        format!("{label}> {input}█")
    } else {
        if a.tab == 0 {
            if let Some(audio) = &a.audio {
                format!(
                    "{}\n{}",
                    a.status,
                    if a.c.listen_mode == "wfm" {
                        audio.rds_summary()
                    } else {
                        "Spectrum held while listening".into()
                    }
                )
            } else if let Some(details) = selected_signal(a) {
                format!(
                    "{}\n{}",
                    details.lines().next().unwrap_or_default(),
                    a.status
                )
            } else {
                format!("● {}", a.status)
            }
        } else {
            format!("{}{}", if a.busy { "⠿ " } else { "● " }, a.status)
        }
    };
    f.render_widget(
        Paragraph::new(status)
            .style(Style::default().fg(CYAN))
            .wrap(Wrap { trim: false }),
        rows[3],
    );
    f.render_widget(
        Paragraph::new(if a.tab == 0 {
            " [] zoom  click peak  t tune  m mode  b BW  a listen  f detail  d console  ? help"
        } else {
            " Space RX  d decoder console  D pause decoders  : command  Tab panels  ? help  q quit"
        })
        .style(Style::default().fg(MUTED)),
        rows[4],
    );
}
fn spectrum(f: &mut Frame, area: Rect, a: &App) {
    let [spectrum_area, waterfall_area, sidebar] = spectrum_areas(area);
    let spec = a
        .report
        .as_ref()
        .map(|r| r.spectrum_dbfs.clone())
        .unwrap_or_else(|| vec![-120.0; 256]);
    let n = spec.len();
    let center = a.report.as_ref().map_or(a.c.frequency, |r| r.center_hz) as f64;
    let rate = a.report.as_ref().map_or(a.c.sample_rate, |r| r.sample_rate) as f64;
    let (first, end) = visible_bins(a, n);
    let bin_hz = |bin: usize| center + (bin as f64 / n as f64 - 0.5) * rate;
    let spectrum_title = format!(
        " {:.4}–{:.4} MHz · {}× · {} {:.1}kHz ",
        bin_hz(first) / 1e6,
        bin_hz(end - 1) / 1e6,
        a.zoom,
        a.c.listen_mode.to_uppercase(),
        a.c.listen_bandwidth as f64 / 1000.
    );
    let canvas = Canvas::default()
        .block(panel(&spectrum_title))
        .marker(symbols::Marker::Braille)
        .x_bounds([first as f64, (end - 1) as f64])
        .y_bounds([-130.0, 0.0])
        .paint(|ctx| {
            for y in [-100.0, -60.0, -20.0] {
                ctx.draw(&CanvasLine {
                    x1: first as f64,
                    y1: y,
                    x2: (end - 1) as f64,
                    y2: y,
                    color: Color::Rgb(30, 40, 55),
                });
            }
            for i in first + 1..end {
                ctx.draw(&CanvasLine {
                    x1: (i - 1) as f64,
                    y1: spec[i - 1] as f64,
                    x2: i as f64,
                    y2: spec[i] as f64,
                    color: CYAN,
                });
            }
            for (frequency, color) in [
                (center - a.c.listen_bandwidth as f64 / 2., MUTED),
                (center + a.c.listen_bandwidth as f64 / 2., MUTED),
                (a.selected_frequency.unwrap_or(f64::NAN), Color::Yellow),
            ] {
                let bin = ((frequency - center) / rate + 0.5) * n as f64;
                if bin >= first as f64 && bin <= (end - 1) as f64 {
                    ctx.draw(&CanvasLine {
                        x1: bin,
                        y1: -130.,
                        x2: bin,
                        y2: 0.,
                        color,
                    });
                }
            }
        });
    f.render_widget(canvas, spectrum_area);
    let waterfall_title = format!(
        " Waterfall · {} · {:.0} dBFS · {:.0} Hz/bin · f detail ",
        WATERFALL_PALETTES[a.waterfall_palette],
        a.waterfall_floor,
        rate / n as f64
    );
    let block = panel(&waterfall_title);
    let inner = block.inner(waterfall_area);
    f.render_widget(block, waterfall_area);
    for y in 0..inner.height {
        for x in 0..inner.width {
            let top = waterfall_power(
                a.water.get(y as usize * 2),
                first,
                end,
                x as usize,
                inner.width as usize,
            );
            let bottom = waterfall_power(
                a.water.get(y as usize * 2 + 1),
                first,
                end,
                x as usize,
                inner.width as usize,
            );
            f.buffer_mut()[(inner.x + x, inner.y + y)]
                .set_symbol("▀")
                .set_fg(heat(top, a.waterfall_palette, a.waterfall_floor))
                .set_bg(heat(bottom, a.waterfall_palette, a.waterfall_floor));
        }
    }
    if sidebar.width > 0 {
        let r = a.report.as_ref();
        let mut text = format!(
            "\n  ))) thugsrf_\n\n  CAPTURE\n  ANALYZE\n  RESEARCH\n\n  Device    {}\n  Center    {:.6} MHz\n  Rate      {:.3} MS/s\n  FFT       {} bins\n  LNA / VGA {} / {} dB\n\n  Peaks     {}\n  Display   {}\n\n  {}\n\n  Kawaiipantsu\n  Danish hacking community\n  https://thugs.red",
            a.c.device,
            a.c.frequency as f64 / 1e6,
            a.c.sample_rate as f64 / 1e6,
            a.c.fft_size,
            a.c.lna_gain,
            a.c.vga_gain,
            r.map_or(0, |r| r.peaks.len()),
            if a.paused { "FROZEN" } else { "LIVE" },
            if a.c.device == "demo" {
                "SYNTHETIC DEMO"
            } else {
                "PASSIVE RECEIVER"
            }
        );
        if let Some(details) = selected_signal(a) {
            text = format!(
                "{details}\n\nt: tune selected\nl: frequency lookup\n[] / wheel: zoom\nm: mode · b: bandwidth\na: listen/stop\n\n{text}"
            );
        }
        if let Some(audio) = &a.audio {
            text = format!(
                "LISTENING · spectrum held\n{}\n\n{text}",
                if a.c.listen_mode == "wfm" {
                    audio.rds_summary()
                } else {
                    String::new()
                }
            );
        }
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(MUTED))
                .block(panel(" Receiver ")),
            sidebar,
        );
    }
}
fn heat(db: f32, palette: usize, floor: f32) -> Color {
    if !db.is_finite() || db <= floor {
        return Color::Rgb(0, 0, 0);
    }
    let t = ((db - floor) / (-10.0 - floor)).clamp(0.0, 1.0);
    let stops = match palette {
        1 => Some([(0, 0, 0), (180, 0, 0), (255, 150, 0), (255, 255, 220)]),
        2 => Some([(0, 0, 0), (0, 40, 160), (0, 200, 220), (230, 255, 255)]),
        3 => Some([(0, 0, 0), (0, 70, 10), (30, 190, 40), (220, 255, 200)]),
        4 => Some([(0, 0, 0), (85, 85, 85), (170, 170, 170), (255, 255, 255)]),
        _ => None,
    };
    if let Some(stops) = stops {
        let position = t * 3.0;
        let index = (position as usize).min(2);
        let blend = position - index as f32;
        let lo = stops[index];
        let hi = stops[index + 1];
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * blend) as u8;
        return Color::Rgb(mix(lo.0, hi.0), mix(lo.1, hi.1), mix(lo.2, hi.2));
    }
    if t < 0.3 {
        Color::Rgb(8, (t * 80.0) as u8, (25.0 + t * 300.0) as u8)
    } else if t < 0.65 {
        Color::Rgb(((t - 0.3) * 550.0) as u8, 30, (140.0 - t * 130.0) as u8)
    } else {
        Color::Rgb(255, ((t - 0.65) * 700.0) as u8, ((t - 0.65) * 300.0) as u8)
    }
}
fn settings(c: &Config) -> Vec<(String, String)> {
    toml::Value::try_from(c)
        .expect("config serializes")
        .as_table()
        .expect("config table")
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string()),
            )
        })
        .collect()
}
fn help() -> &'static str {
    "THUGS(red) RF · Kawaiipantsu · https://thugs.red\n\nSpace starts/stops RX. Demo is explicitly synthetic.\nSpectrum: Enter opens frequency input (e.g. 145.252MHz); Esc cancels.\nSpectrum: [] or mouse wheel zoom, 0 full span; click a peak, t tunes, l looks up frequency.\nReceive: m cycles NFM/FM/WFM/AM; b edits bandwidth; a starts/stops listening.\nWaterfall: f cycles FFT detail (2048–65536 bins); c cycles colors; + raises threshold (hides weak signals), - lowers it.\n←/→ fine tune (500 kHz); ↑/↓ or PgUp/PgDn coarse (10 MHz).\nSettings: fine_tune_hz / coarse_tune_hz change steps; frequency accepts 145.252MHz.\nKeyboard tuning is session-only; Settings saves defaults.\nd opens the rolling Decoder Console; enabled compatible addons try live RX windows.\nD pauses/resumes live addon decoding; s toggles decoder-output-<timestamp>.log saving; x clears console. Up to 3 decoders run concurrently.\nTab cycles all panels; 1–9 select the first nine. 7 Survey, 8 Listen, 9 VHF/UHF. s in Spectrum exports full-bin ASCII graph and waterfall; s in Detections saves SQLite.\nSettings: ↑↓ and Enter to edit any field. Esc cancels edits.\nAddons: ↑↓ and Enter to enable a reviewed addon.\nr prepares a five-second recording; Enter starts it.\n: opens the command bar; commands run on a worker thread.\nRX stops before jobs so hardware is not opened twice.\n\nExample commands (paths containing spaces need quotes):\n  doctor\n  addon install\n  addon enable all --kind identifiers\n  identify /tmp/signal.cs8\n  frequency sources\n  frequency lookup --frequency 145600000\n  record /tmp/signal.cs8 --seconds 5\n  analyze /tmp/signal.cs8 --png /tmp/spectrum.png\n  decode /tmp/signal.cs8 --mode ook\n  decode /tmp/fm.cs8 --mode rds\n  addon run rtl433 /tmp/signal.cs8\n  demod /tmp/signal.cs8 /tmp/audio.wav --mode fm\n  play /tmp/audio.wav\n  encode /tmp/test.wav --bits 10110010 --mode afsk\n  ai --input /tmp/audio.wav --format wav\n  ai --image /tmp/spectrum.png\n  history\n\nAI: set ai_provider, ai_model and local_url in Settings.\nKeys: OPENAI_API_KEY / ANTHROPIC_API_KEY / THUGSRF_LOCAL_API_KEY.\nAI receives measured features for WAV/IQ, or supplied images.\nAI output is a hypothesis. Audio waveforms are not sent directly.\n\nRF replay uses signed 8-bit IQ and requires --confirm-tx.\nAudio playback uses your selected ALSA device.\nUse --help on any command for options.\n"
}
fn survey_view(f: &mut Frame, area: Rect, a: &App) {
    let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).split(area);
    let status = if let Some(p) = &a.panorama {
        format!(
            "{}–{} MHz · {:.0} kHz bins · pass {} · current coverage {:.1}%\nSequential sweep: bins have different acquisition times. Space start/stop.",
            p.start_mhz,
            p.end_mhz,
            p.bin_hz as f64 / 1000.,
            p.passes + 1,
            p.coverage * 100.
        )
    } else {
        format!(
            "{}–{} MHz sequential HackRF survey · Space starts\nEdit sweep_start_mhz / sweep_end_mhz / sweep_bin_hz in Settings.",
            a.c.sweep_start_mhz, a.c.sweep_end_mhz
        )
    };
    f.render_widget(
        Paragraph::new(status).style(Style::default().fg(CYAN)),
        parts[0],
    );
    let latest: Vec<_> = a
        .panorama
        .as_ref()
        .map(|p| {
            p.power
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        p.start_mhz as f64 + (i as f64 + 0.5) * p.bin_hz as f64 / 1e6,
                        *v as f64,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let peak: Vec<_> = a
        .panorama
        .as_ref()
        .map(|p| {
            p.peak
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        p.start_mhz as f64 + (i as f64 + 0.5) * p.bin_hz as f64 / 1e6,
                        *v as f64,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let (start_mhz, end_mhz) = a.panorama.as_ref().map_or(
        (a.c.sweep_start_mhz as f64, a.c.sweep_end_mhz as f64),
        |p| (p.start_mhz as f64, p.end_mhz as f64),
    );
    // Reserve room for the power axis and keep frequency labels readable.
    let intervals = (parts[1].width.saturating_sub(12) as usize / 12).clamp(1, 12);
    let ticks: Vec<_> = (0..=intervals)
        .map(|i| start_mhz + (end_mhz - start_mhz) * i as f64 / intervals as f64)
        .collect();
    let labels: Vec<_> = ticks.iter().map(|mhz| format!("{mhz:.1}")).collect();
    let dividers: Vec<_> = ticks[1..ticks.len() - 1]
        .iter()
        .map(|&mhz| [(mhz, -100.), (mhz, 0.)])
        .collect();
    let mut datasets: Vec<_> = dividers
        .iter()
        .map(|line| {
            Dataset::default()
                .graph_type(GraphType::Line)
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Rgb(40, 49, 63)))
                .data(line)
        })
        .collect();
    datasets.extend([
        Dataset::default()
            .name("latest")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(CYAN))
            .data(&latest),
        Dataset::default()
            .name("peak hold")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(RED))
            .data(&peak),
    ]);
    f.render_widget(
        Chart::new(datasets)
            .block(panel(" Wideband panorama · relative power "))
            .x_axis(
                Axis::default()
                    .title("MHz")
                    .bounds([start_mhz, end_mhz])
                    .labels(labels),
            )
            .y_axis(
                Axis::default()
                    .bounds([-100., 0.])
                    .labels(["-100", "-50", "0"]),
            ),
        parts[1],
    );
}
fn listening_view(f: &mut Frame, area: Rect, a: &App) {
    let parts = Layout::vertical([
        Constraint::Length(7),
        Constraint::Min(5),
        Constraint::Length(4),
    ])
    .split(area);
    let rds = if a.c.listen_mode == "wfm" {
        a.audio.as_ref().map_or_else(
            || "RDS: start WFM listening to decode station data".into(),
            |audio| audio.rds_summary(),
        )
    } else {
        String::new()
    };
    f.render_widget(Paragraph::new(format!("{:.6} MHz  {}  BW {} Hz  squelch {:.0} dBFS\n↑↓ select · Enter tune · Space listen/stop · m mode · b bandwidth\nALSA: {} · {}\nAM / narrow FM / mono broadcast FM (50 µs de-emphasis)\n{}",a.c.frequency as f64/1e6,a.c.listen_mode,a.c.listen_bandwidth,a.c.squelch_dbfs,a.c.audio_device,if a.audio.is_some(){"LISTENING"}else{"STOPPED"},rds)).wrap(Wrap { trim: false }).style(Style::default().fg(CYAN)),parts[0]);
    let rows = if a.tab == 7 {
        Ok(crate::listening::presets())
    } else {
        crate::listening::channels()
    };
    match rows {
        Ok(rows) => {
            let mut state = ListState::default().with_selected(if rows.is_empty() {
                None
            } else {
                Some(a.selected % rows.len())
            });
            let items = rows
                .iter()
                .map(|r| {
                    ListItem::new(format!(
                        "{} · RX {:.6} · TX {} MHz · {}\n  CTCSS {} Hz · {}",
                        r.name,
                        r.rx_hz as f64 / 1e6,
                        r.tx_hz
                            .map(|v| format!("{:.6}", v as f64 / 1e6))
                            .unwrap_or_else(|| "—".into()),
                        r.mode,
                        r.ctcss_hz,
                        r.source
                    ))
                })
                .collect::<Vec<_>>();
            f.render_stateful_widget(
                List::new(items)
                    .highlight_style(Style::default().fg(RED))
                    .highlight_symbol("▶ ")
                    .block(panel(if a.tab == 7 {
                        " Listening presets "
                    } else {
                        " Channels / repeaters · a add · e edit · t prepare TX "
                    })),
                parts[1],
                &mut state,
            );
        }
        Err(e) => f.render_widget(Paragraph::new(e.to_string()), parts[1]),
    }
    f.render_widget(Paragraph::new(if a.tab==7{"Presets are tuning starting points, not station listings.\nHackRF MW: only upper band ≥1 MHz; lower MW needs an upconverter.\nNESDR needs an HF upconverter for SW/MW."}else{"Directory: ~/.config/thugsrf/repeaters.toml · : channels remove <index>\nDanish listings: https://www.oz1ln.dk/kort_og_lister/\nHalf-duplex: t prepares a finite TX command; append --confirm-tx to send."}).style(Style::default().fg(MUTED)),parts[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_app() -> App {
        let (tx, rx) = mpsc::channel();
        App {
            c: Config::default(),
            tab: 0,
            stream: None,
            survey: None,
            panorama: None,
            audio: None,
            report: None,
            water: VecDeque::new(),
            waterfall_palette: 0,
            waterfall_floor: -110.0,
            zoom: 1,
            view_center: None,
            selected_frequency: None,
            editing_bandwidth: false,
            status: "Ready".into(),
            output: help().into(),
            input: None,
            editing: None,
            tuning: false,
            selected: 0,
            scroll: 0,
            busy: false,
            reload_config: false,
            job: rx,
            send: tx,
            paused: false,
            history: String::new(),
            decoder_log: VecDeque::new(),
            decoder_log_file: None,
            decoder_log_path: None,
            decoders_enabled: true,
            last_rds: String::new(),
        }
    }
    #[test]
    fn layouts_fit() {
        for (w, h) in [(80, 24), (170, 50), (60, 15)] {
            let mut a = test_app();
            let mut t = Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            for tab in 0..TABS.len() {
                a.tab = tab;
                t.draw(|f| draw(f, &a)).unwrap();
            }
            assert!(
                t.backend()
                    .buffer()
                    .content
                    .iter()
                    .any(|c| c.symbol().contains('T') || c.symbol().contains('t'))
            );
        }
    }
    #[test]
    fn waterfall_preserves_thin_peaks_and_zoomed_edges() {
        let mut row = vec![-100.; 8192];
        row[4097] = -15.;
        let column = 4097 * 80 / row.len();
        assert_eq!(waterfall_power(Some(&row), 0, row.len(), column, 80), -15.);
        assert_eq!(waterfall_power(None, 0, row.len(), column, 80), -160.);
        for column in 0..80 {
            let (lo, hi) = column_bins(4094, 4102, column, 80);
            assert!((4094..4102).contains(&lo) && hi > lo && hi <= 4102);
        }
    }

    #[test]
    fn mouse_selection_uses_zoomed_frequency_bins_and_ignores_editing() {
        let mut a = test_app();
        a.c.frequency = 100_000_000;
        a.c.sample_rate = 8_000_000;
        let n = 8192;
        let mut spectrum = vec![-100.; n];
        spectrum[4097] = -15.;
        a.report = Some(Report {
            source: "test".into(),
            center_hz: a.c.frequency,
            sample_rate: a.c.sample_rate,
            samples: n,
            rms_dbfs: -50.,
            noise_dbfs: -100.,
            crest_db: 10.,
            peaks: vec![],
            spectrum_dbfs: spectrum,
            notes: vec![],
        });
        let screen = Rect::new(0, 0, 170, 50);
        let area = panel("").inner(spectrum_areas(main_rows(screen)[2])[0]);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + area.width / 2,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        };
        spectrum_mouse(&mut a, mouse, screen);
        let selected = a.selected_frequency.unwrap();
        assert_eq!(selected, 100_000_000. + 8_000_000. / n as f64);
        assert!(selected_signal(&a).unwrap().contains("-15.0 dBFS"));
        zoom_spectrum(&mut a, true);
        assert_eq!(visible_bins(&a, n), (2049, 6145));
        a.view_center = Some(200_000_000.);
        assert_eq!(visible_bins(&a, n), (4096, 8192));
        a.view_center = Some(0.);
        assert_eq!(visible_bins(&a, n), (0, 4096));
        a.input = Some("145MHz".into());
        spectrum_mouse(&mut a, mouse, screen);
        assert_eq!(a.selected_frequency, Some(selected));
        a.input = None;
        a.tab = 7;
        spectrum_mouse(&mut a, mouse, screen);
        assert_eq!(a.selected_frequency, Some(selected));
    }
}
