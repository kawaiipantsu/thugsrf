//! ASCII exports use FFT bins and waterfall samples, independent of terminal pixels.
use crate::{config, dsp::Report};
use anyhow::{Result, ensure};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
};

pub fn spectrum_ascii(
    report: &Report,
    water: &VecDeque<Vec<f32>>,
    first: usize,
    end: usize,
    floor: f32,
) -> Result<(PathBuf, PathBuf)> {
    ensure!(
        first < end && end <= report.spectrum_dbfs.len(),
        "no visible FFT bins"
    );
    let stamp = crate::stamp();
    let graph = config::config_dir().join(format!("spectrum-{stamp}.txt"));
    let waterfall = config::config_dir().join(format!("waterfall-{stamp}.txt"));
    use std::os::unix::fs::OpenOptionsExt;
    let create = |path: &PathBuf| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
    };
    let graph_file = create(&graph)?;
    let waterfall_file = match create(&waterfall) {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&graph);
            return Err(error.into());
        }
    };
    let result = write_exports(graph_file, waterfall_file, report, water, first, end, floor);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&graph);
        let _ = std::fs::remove_file(&waterfall);
        return Err(error);
    }
    Ok((graph, waterfall))
}

fn write_exports(
    graph: File,
    waterfall: File,
    report: &Report,
    water: &VecDeque<Vec<f32>>,
    first: usize,
    end: usize,
    floor: f32,
) -> Result<()> {
    let mut graph = BufWriter::new(graph);
    let mut waterfall = BufWriter::new(waterfall);
    let n = report.spectrum_dbfs.len();
    let step = report.sample_rate as f64 / n as f64;
    let start_hz = report.center_hz as f64 + (first as f64 - n as f64 / 2.) * step;
    let width = end - first;
    for output in [&mut graph, &mut waterfall] {
        writeln!(
            output,
            "# THUGSRF ASCII export; source: {}",
            report.source.escape_default()
        )?;
        writeln!(
            output,
            "# One column per FFT bin; columns={width}, total_fft_bins={n}, first_bin={first}"
        )?;
        writeln!(
            output,
            "# First column: {start_hz:.6} Hz; last: {:.6} Hz; step: {step:.6} Hz/column",
            start_hz + (width - 1) as f64 * step
        )?;
        writeln!(
            output,
            "# Center={} Hz; sample_rate={} Hz; uncalibrated dBFS",
            report.center_hz, report.sample_rate
        )?;
        writeln!(
            output,
            "# View without line wrapping in a monospace editor. Horizontal labels are offsets in columns."
        )?;
    }
    write_graph(&mut graph, &report.spectrum_dbfs[first..end])?;
    writeln!(
        waterfall,
        "# Newest row first; one stored time sample per row; {} rows",
        water.len()
    )?;
    writeln!(
        waterfall,
        "# Intensity floor={floor:.1} dBFS; saturation=-10 dBFS; ramp=' .,:;irsXA253hMHGS#9B&@'"
    )?;
    let ramp = b" .,:;irsXA253hMHGS#9B&@";
    for row in water {
        ensure!(row.len() == n, "waterfall FFT size differs from spectrum");
        let line: Vec<u8> = row[first..end]
            .iter()
            .map(|&db| {
                let value = if db.is_finite() {
                    ((db - floor) / (-10. - floor)).clamp(0., 1.)
                } else {
                    0.
                };
                ramp[(value * (ramp.len() - 1) as f32).round() as usize]
            })
            .collect();
        waterfall.write_all(&line)?;
        waterfall.write_all(b"\n")?;
    }
    graph.flush()?;
    waterfall.flush()?;
    Ok(())
}

fn write_graph(out: &mut impl Write, spectrum: &[f32]) -> Result<()> {
    writeln!(
        out,
        "# Signal graph: 131 rows, 1 dB per row (0 to -130 dBFS); '*' is an FFT sample"
    )?;
    let levels: Vec<_> = spectrum
        .iter()
        .map(|&db| {
            if db.is_finite() {
                (-db).round().clamp(0., 130.) as usize
            } else {
                130
            }
        })
        .collect();
    for row in 0..=130 {
        write!(out, "{:>4} |", -(row as i32))?;
        let line: Vec<_> = levels
            .iter()
            .map(|&level| {
                if level == row {
                    b'*'
                } else if row % 10 == 0 {
                    b'.'
                } else {
                    b' '
                }
            })
            .collect();
        out.write_all(&line)?;
        out.write_all(b"\n")?;
    }
    write!(out, "     +")?;
    out.write_all(&vec![b'-'; spectrum.len()])?;
    out.write_all(b"\n      ")?;
    let mut axis = vec![b' '; spectrum.len()];
    for column in (0..spectrum.len()).step_by(128) {
        for (offset, byte) in column.to_string().bytes().enumerate() {
            if column + offset < axis.len() {
                axis[column + offset] = byte;
            }
        }
    }
    out.write_all(&axis)?;
    out.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ascii_graph_keeps_every_bin_at_one_db_vertical_resolution() {
        let spectrum = [-100., -10., -80., -40.];
        let mut bytes = Vec::new();
        write_graph(&mut bytes, &spectrum).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let rows: Vec<_> = text.lines().filter(|line| line.contains('|')).collect();
        assert_eq!(rows.len(), 131);
        assert_eq!(rows[10], " -10 |.*..");
        assert_eq!(rows[100], "-100 |*...");
        assert!(rows.iter().all(|row| row.len() == 6 + spectrum.len()));
    }
}
