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
    let a = list()?
        .into_iter()
        .find(|a| a.manifest.name == name)
        .context("addon not found")?;
    ensure!(
        a.manifest.enabled,
        "addon disabled; review its code then enable it with addon toggle {name}"
    );
    let mut cmd = Command::new(&a.manifest.command[0]);
    cmd.args(&a.manifest.command[1..])
        .current_dir(a.path.parent().context("addon directory")?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().context("launching addon")?;
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
        if start.elapsed() > Duration::from_secs(a.manifest.timeout_seconds) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("addon timed out");
        }
        thread::sleep(Duration::from_millis(20));
    };
    ensure!(status.success(), "addon exited with {status}");
    // Join only finished I/O workers: a misbehaving descendant must not hold the host hostage.
    while !reader.is_finished() || !writer.is_finished() {
        ensure!(
            start.elapsed() < Duration::from_secs(a.manifest.timeout_seconds),
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
    Ok(
        serde_json::json!({"api_version":1,"action":"analyze","input":{"path":path.canonicalize()?,"format":format,"sample_rate":rate,"center_hz":center}}),
    )
}
