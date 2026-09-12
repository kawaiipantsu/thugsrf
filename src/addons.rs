use crate::config;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub api_version: u32,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default = "timeout")]
    pub timeout_seconds: u64,
}
fn timeout() -> u64 {
    10
}
#[derive(Clone, Debug)]
pub struct Addon {
    pub path: PathBuf,
    pub manifest: Manifest,
}
pub fn list() -> Result<Vec<Addon>> {
    let mut result = Vec::new();
    for kind in ["decoders", "identifiers"] {
        let dir = config::config_dir().join(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path().join("addon.toml");
            if !path.is_file() {
                continue;
            }
            let m: Manifest = toml::from_str(&std::fs::read_to_string(&path)?)
                .with_context(|| format!("invalid manifest {}", path.display()))?;
            ensure!(m.api_version == 1, "unsupported addon API: {}", m.name);
            ensure!(m.kind == kind, "addon kind mismatch: {}", m.name);
            ensure!(
                !m.name.is_empty() && !m.command.is_empty(),
                "addon name/command missing"
            );
            ensure!(
                (1..=120).contains(&m.timeout_seconds),
                "addon timeout must be 1..120 seconds"
            );
            result.push(Addon { path, manifest: m });
        }
    }
    result.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    for pair in result.windows(2) {
        ensure!(
            pair[0].manifest.name != pair[1].manifest.name,
            "duplicate addon name: {}",
            pair[0].manifest.name
        );
    }
    Ok(result)
}
pub fn toggle(name: &str) -> Result<String> {
    let a = list()?
        .into_iter()
        .find(|a| a.manifest.name == name)
        .context("addon not found")?;
    let mut m = a.manifest;
    m.enabled = !m.enabled;
    std::fs::write(a.path, toml::to_string_pretty(&m)?)?;
    Ok(format!("{} enabled={}", m.name, m.enabled))
}
pub fn run(name: &str, request: &serde_json::Value) -> Result<serde_json::Value> {
    run_inner(name, request, || false, 120)
}

pub fn run_live(
    name: &str,
    request: &serde_json::Value,
    stop: &std::sync::atomic::AtomicBool,
    enabled: &std::sync::atomic::AtomicBool,
) -> Result<serde_json::Value> {
    run_inner(
        name,
        request,
        || {
            stop.load(std::sync::atomic::Ordering::Relaxed)
                || !enabled.load(std::sync::atomic::Ordering::Relaxed)
        },
        15,
    )
}

fn run_inner(
    name: &str,
    request: &serde_json::Value,
    cancelled: impl Fn() -> bool,
    max_seconds: u64,
) -> Result<serde_json::Value> {
    let a = list()?
        .into_iter()
        .find(|a| a.manifest.name == name)
        .context("addon not found")?;
    ensure!(
        a.manifest.enabled,
        "addon disabled; review its code then enable it with addon toggle {name}"
    );
    if !a.manifest.formats.is_empty() {
        let format = request["input"]["format"]
            .as_str()
            .context("input format missing")?;
        ensure!(
            a.manifest.formats.iter().any(|f| f == format),
            "{} supports: {}",
            name,
            a.manifest.formats.join(", ")
        );
    }
    let timeout_seconds = a.manifest.timeout_seconds.min(max_seconds);
    let mut cmd = Command::new(&a.manifest.command[0]);
    cmd.args(&a.manifest.command[1..])
        .current_dir(a.path.parent().context("addon directory")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
    let mut child = cmd.spawn().context("launching addon")?;
    let _group = ProcessGroup(child.id());
    let mut input = child.stdin.take().context("addon stdin")?;
    let data = serde_json::to_vec(request)?;
    let writer = thread::spawn(move || input.write_all(&data));
    let output = child.stdout.take().context("addon stdout")?;
    let reader = thread::spawn(move || {
        let mut b = Vec::new();
        output.take(1_048_577).read_to_end(&mut b).map(|_| b)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        }
        if crate::CANCELLED.load(std::sync::atomic::Ordering::Relaxed)
            || cancelled()
            || start.elapsed() > Duration::from_secs(timeout_seconds)
        {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("addon cancelled or timed out");
        }
        thread::sleep(Duration::from_millis(20));
    };
    ensure!(status.success(), "addon exited with {status}");
    // Join only finished I/O workers: a misbehaving descendant must not hold the host hostage.
    while !reader.is_finished() || !writer.is_finished() {
        ensure!(
            start.elapsed() < Duration::from_secs(timeout_seconds) && !cancelled(),
            "addon pipe timeout"
        );
        thread::sleep(Duration::from_millis(10));
    }
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("addon writer panicked"))??;
    let data = reader
        .join()
        .map_err(|_| anyhow::anyhow!("addon reader panicked"))??;
    ensure!(data.len() <= 1_048_576, "addon output exceeds 1 MiB");
    let result: serde_json::Value =
        serde_json::from_slice(&data).context("addon must emit one JSON object")?;
    ensure!(result.is_object(), "addon result must be a JSON object");
    Ok(result)
}
pub fn request(path: &Path, format: &str, rate: u32, center: u64) -> Result<serde_json::Value> {
    let format = if format == "auto" {
        match path.extension().and_then(|v| v.to_str()).unwrap_or("") {
            "cs8" | "iq" => "cs8",
            "cu8" => "cu8",
            "wav" => "wav",
            "pcap" => "pcap",
            "pcapng" => "pcapng",
            "nmea" => "nmea",
            "ts" => "ts",
            "hex" => "hex",
            _ => anyhow::bail!("cannot infer format; use --format"),
        }
    } else {
        format
    };
    Ok(
        serde_json::json!({"api_version":1,"action":"analyze","input":{"path":path.canonicalize()?,"format":format,"sample_rate":rate,"center_hz":center}}),
    )
}

pub fn enable(name: &str, kind: Option<&str>, enabled: bool) -> Result<String> {
    if let Some(kind) = kind {
        ensure!(
            ["decoders", "identifiers"].contains(&kind),
            "kind must be decoders or identifiers"
        );
    }
    let mut count = 0;
    for a in list()? {
        if (name == "all" || a.manifest.name == name) && kind.is_none_or(|k| k == a.manifest.kind) {
            let mut m = a.manifest;
            m.enabled = enabled;
            std::fs::write(a.path, toml::to_string_pretty(&m)?)?;
            count += 1;
        }
    }
    ensure!(count > 0, "no matching addons installed");
    Ok(format!("{count} addons enabled={enabled}"))
}

pub fn run_enabled(kind: &str, request: &serde_json::Value) -> Result<serde_json::Value> {
    ensure!(
        ["decoders", "identifiers"].contains(&kind),
        "kind must be decoders or identifiers"
    );
    let modules: Vec<_> = list()?
        .into_iter()
        .filter(|a| a.manifest.enabled && a.manifest.kind == kind)
        .collect();
    ensure!(
        !modules.is_empty(),
        "no enabled {kind}; enable modules in the Addons tab or with addon enable all --kind {kind}"
    );
    let mut results = Vec::new();
    for group in modules.chunks(3) {
        ensure!(
            !crate::CANCELLED.load(std::sync::atomic::Ordering::Relaxed),
            "analysis cancelled"
        );
        std::thread::scope(|scope| {
            let handles: Vec<_> = group.iter().map(|a| scope.spawn(move || {
                if !a.manifest.formats.is_empty() && !a.manifest.formats.iter().any(|f|request["input"]["format"] == *f) {
                    return serde_json::json!({"module":a.manifest.name,"status":"skipped","reason":"unsupported input format"});
                }
                match run(&a.manifest.name, request) {
                    Ok(result) => serde_json::json!({"module":a.manifest.name,"result":result}),
                    Err(e) => serde_json::json!({"module":a.manifest.name,"status":"error","error":e.to_string()}),
                }
            })).collect();
            for h in handles {
                results.push(h.join().unwrap_or_else(
                    |_| serde_json::json!({"status":"error","error":"addon worker panicked"}),
                ));
            }
        });
    }
    Ok(serde_json::json!({"kind":kind,"input":request["input"],"results":results}))
}

// Kill owned descendants on timeout/cancellation as well as normal addon exit.
struct ProcessGroup(u32);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.0)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub fn install() -> Result<String> {
    let mut candidates = vec![
        std::path::PathBuf::from("/usr/share/thugsrf/addons"),
        std::path::PathBuf::from("/usr/local/share/thugsrf/addons"),
    ];
    if let Ok(exe) = std::env::current_exe()
        && let Some(root) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
    {
        candidates.insert(0, root.join("addons"));
    }
    let source = candidates
        .into_iter()
        .find(|p| p.join("_shared/v0_2/runner.py").exists())
        .context("bundled addons not found; run make addons from the source checkout")?;
    fn copy(source: &std::path::Path, target: &std::path::Path) -> Result<usize> {
        std::fs::create_dir_all(target)?;
        let mut count = 0;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == "__pycache__" {
                continue;
            }
            let to = target.join(name);
            if entry.file_type()?.is_dir() {
                count += copy(&entry.path(), &to)?;
            } else if entry.file_type()?.is_file() && !to.exists() {
                std::fs::copy(entry.path(), to)?;
                count += 1;
            }
        }
        Ok(count)
    }
    let count = copy(&source, &crate::config::config_dir())?;
    Ok(format!(
        "Installed {count} bundled files; existing user files preserved. Enable modules in Addons or with addon enable."
    ))
}
