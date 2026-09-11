use crate::{
    config::{self, Config},
    dsp::Report,
    radio::Stream,
};
use anyhow::{Result, ensure};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
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
const TABS: [&str; 9] = [
    "Spectrum",
    "Detections",
    "Recordings",
    "Addons",
    "Settings",
    "Workbench",
    "Survey",
    "Listen",
    "VHF/UHF",
];
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
    status: String,
    output: String,
    input: Option<String>,
    editing: Option<String>,
    selected: usize,
    scroll: u16,
    busy: bool,
    reload_config: bool,
    job: mpsc::Receiver<String>,
    send: mpsc::Sender<String>,
    paused: bool,
    history: String,
}
pub fn run(c: Config) -> Result<()> {
    ensure!(
        io::stdout().is_terminal() && io::stdin().is_terminal(),
        "TUI requires an interactive terminal; use doctor or analyze for headless operation"
    );
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
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
        status: "Ready · Space starts reception · : opens command bar".into(),
        output: help().into(),
        input: None,
        editing: None,
        selected: 0,
        scroll: 0,
        busy: false,
        reload_config: false,
        job,
        send,
        paused: false,
        history: crate::storage::history().unwrap_or_default(),
    };
    while !crate::CANCELLED.load(Ordering::Relaxed) || a.busy {
        if let Some(audio) = &mut a.audio {
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
        if let Some(stream) = &a.stream {
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
        if event::poll(Duration::from_millis(40))?
            && let Event::Key(k) = event::read()?
        {
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
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Char(c) => input.push(c),
                    KeyCode::Enter => {
                        let text = a.input.take().unwrap_or_default();
                        if let Some(key) = a.editing.take() {
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
                    let frequency = a.c.frequency;
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
                    } else if a.tab >= 7 {
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
                        match Stream::start(a.c.clone()) {
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
                KeyCode::Enter if a.tab >= 7 => {
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
fn tune_spectrum(a: &mut App, step: u64, increase: bool) {
    if a.busy || a.audio.is_some() || a.survey.is_some() {
        a.status = "Stop the active job, listening or survey before tuning Spectrum".into();
        return;
    }
    if a.c.device == "audio" {
        a.status = "Audio input has no RF tuning frequency".into();
        return;
    }
    let mut next = a.c.clone();
    let frequency = if increase {
        next.frequency.checked_add(step)
    } else {
        next.frequency.checked_sub(step)
    };
    let Some(frequency) = frequency else {
        a.status = "Tuning would exceed the frequency range".into();
        return;
    };
    next.frequency = frequency;
    if let Err(e) = next.validate() {
        a.status = e.to_string();
        return;
    }
    // Drop joins the old receiver before a replacement opens the USB device.
    let running = a.stream.is_some();
    a.stream = None;
    a.c = next;
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
        match Stream::start(a.c.clone()) {
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
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(area);
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
                            " Addons · ↑↓ select · Enter toggle · i identify file ",
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
        format!("{}> {}█", a.editing.as_deref().unwrap_or("command"), input)
    } else {
        format!("{}{}", if a.busy { "⠿ " } else { "● " }, a.status)
    };
    f.render_widget(
        Paragraph::new(status)
            .style(Style::default().fg(CYAN))
            .wrap(Wrap { trim: false }),
        rows[3],
    );
    f.render_widget(
        Paragraph::new(if a.tab == 0 {
            " ←→ fine  ↑↓/Pg coarse  Space RX  : command  Tab panels  ? help  q quit"
        } else {
            " Space RX  p freeze  s save  r record  : command  Tab panels  ? help  q quit"
        })
        .style(Style::default().fg(MUTED)),
        rows[4],
    );
}
fn spectrum(f: &mut Frame, area: Rect, a: &App) {
    let cols = Layout::horizontal([
        Constraint::Min(50),
        Constraint::Length(if area.width >= 130 { 34 } else { 0 }),
    ])
    .split(area);
    let plot =
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).split(cols[0]);
    let spec = a
        .report
        .as_ref()
        .map(|r| r.spectrum_dbfs.clone())
        .unwrap_or_else(|| vec![-120.0; 256]);
    let n = spec.len();
    let center = a.report.as_ref().map_or(a.c.frequency, |r| r.center_hz) as f64;
    let rate = a.report.as_ref().map_or(a.c.sample_rate, |r| r.sample_rate) as f64;
    let spectrum_title = format!(
        " Spectrum · {:.3} ↔ {:.3} MHz · dBFS ",
        (center - rate / 2.0) / 1e6,
        (center + rate / 2.0) / 1e6
    );
    let canvas = Canvas::default()
        .block(panel(&spectrum_title))
        .marker(symbols::Marker::Braille)
        .x_bounds([0.0, n as f64])
        .y_bounds([-130.0, 0.0])
        .paint(|ctx| {
            for y in [-100.0, -60.0, -20.0] {
                ctx.draw(&CanvasLine {
                    x1: 0.0,
                    y1: y,
                    x2: n as f64,
                    y2: y,
                    color: Color::Rgb(30, 40, 55),
                });
            }
            for i in 1..n {
                ctx.draw(&CanvasLine {
                    x1: (i - 1) as f64,
                    y1: spec[i - 1] as f64,
                    x2: i as f64,
                    y2: spec[i] as f64,
                    color: CYAN,
                });
            }
        });
    f.render_widget(canvas, plot[0]);
    let block = panel(" Waterfall · newest at top · relative power ");
    let inner = block.inner(plot[1]);
    f.render_widget(block, plot[1]);
    for y in 0..inner.height {
        for x in 0..inner.width {
            let top = a
                .water
                .get(y as usize * 2)
                .and_then(|r| r.get(x as usize * r.len() / inner.width.max(1) as usize))
                .copied()
                .unwrap_or(-160.0);
            let bottom = a
                .water
                .get(y as usize * 2 + 1)
                .and_then(|r| r.get(x as usize * r.len() / inner.width.max(1) as usize))
                .copied()
                .unwrap_or(-160.0);
            f.buffer_mut()[(inner.x + x, inner.y + y)]
                .set_symbol("▀")
                .set_fg(heat(top))
                .set_bg(heat(bottom));
        }
    }
    if cols[1].width > 0 {
        let r = a.report.as_ref();
        let text = format!(
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
        f.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(MUTED))
                .block(panel(" Receiver ")),
            cols[1],
        );
    }
}
fn heat(db: f32) -> Color {
    let t = ((db + 110.0) / 100.0).clamp(0.0, 1.0);
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
    "THUGS(red) RF · Kawaiipantsu · https://thugs.red\n\nSpace starts/stops RX. Demo is explicitly synthetic.\nSpectrum: ←/→ fine tune (500 kHz); ↑/↓ or PgUp/PgDn coarse (10 MHz).\nSettings: fine_tune_hz / coarse_tune_hz change steps; frequency accepts 145.252MHz.\nKeyboard tuning is session-only; Settings saves defaults.\nTab / 1–9 switch panels. 7 Survey, 8 Listen, 9 VHF/UHF. s saves current spectrum to SQLite.\nSettings: ↑↓ and Enter to edit any field. Esc cancels edits.\nAddons: ↑↓ and Enter to enable a reviewed addon.\nr prepares a five-second recording; Enter starts it.\n: opens the command bar; commands run on a worker thread.\nRX stops before jobs so hardware is not opened twice.\n\nExample commands (paths containing spaces need quotes):\n  doctor\n  addon install\n  addon enable all --kind identifiers\n  identify /tmp/signal.cs8\n  frequency sources\n  frequency lookup --frequency 145600000\n  record /tmp/signal.cs8 --seconds 5\n  analyze /tmp/signal.cs8 --png /tmp/spectrum.png\n  decode /tmp/signal.cs8 --mode ook\n  addon run rtl433 /tmp/signal.cs8\n  demod /tmp/signal.cs8 /tmp/audio.wav --mode fm\n  play /tmp/audio.wav\n  encode /tmp/test.wav --bits 10110010 --mode afsk\n  ai --input /tmp/audio.wav --format wav\n  ai --image /tmp/spectrum.png\n  history\n\nAI: set ai_provider, ai_model and local_url in Settings.\nKeys: OPENAI_API_KEY / ANTHROPIC_API_KEY / THUGSRF_LOCAL_API_KEY.\nAI receives measured features for WAV/IQ, or supplied images.\nAI output is a hypothesis. Audio waveforms are not sent directly.\n\nRF replay uses signed 8-bit IQ and requires --confirm-tx.\nAudio playback uses your selected ALSA device.\nUse --help on any command for options.\n"
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
    f.render_widget(
        Chart::new(vec![
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
        ])
        .block(panel(" Wideband panorama · relative power "))
        .x_axis(
            Axis::default()
                .title("MHz")
                .bounds([a.c.sweep_start_mhz as f64, a.c.sweep_end_mhz as f64])
                .labels([
                    a.c.sweep_start_mhz.to_string(),
                    a.c.sweep_end_mhz.to_string(),
                ]),
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
        Constraint::Length(5),
        Constraint::Min(5),
        Constraint::Length(4),
    ])
    .split(area);
    f.render_widget(Paragraph::new(format!("{:.6} MHz  {}  BW {} Hz  squelch {:.0} dBFS\n↑↓ select · Enter tune · Space listen/stop · Settings edit frequency/mode\nALSA: {} · {}\nAM / narrow FM / mono broadcast FM (50 µs de-emphasis)",a.c.frequency as f64/1e6,a.c.listen_mode,a.c.listen_bandwidth,a.c.squelch_dbfs,a.c.audio_device,if a.audio.is_some(){"LISTENING"}else{"STOPPED"})).style(Style::default().fg(CYAN)),parts[0]);
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
    #[test]
    fn layouts_fit() {
        for (w, h) in [(80, 24), (170, 50), (60, 15)] {
            let (tx, rx) = mpsc::channel();
            let mut a = App {
                c: Config::default(),
                tab: 0,
                stream: None,
                survey: None,
                panorama: None,
                audio: None,
                report: None,
                water: VecDeque::new(),
                status: "Ready".into(),
                output: help().into(),
                input: None,
                editing: None,
                selected: 0,
                scroll: 0,
                busy: false,
                reload_config: false,
                job: rx,
                send: tx,
                paused: false,
                history: String::new(),
            };
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
}
