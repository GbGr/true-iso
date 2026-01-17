use clap::Parser;
use std::path::PathBuf;

use crate::geometry::IsometricRatio;

#[derive(Parser, Debug)]
#[command(name = "true-iso")]
#[command(version, about = "Correct isometric tile sprites to mathematically consistent proportions")]
pub struct Cli {
    /// Input PNG image path
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output path [default: input_corrected.png]
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target isometric ratio (e.g., "2:1")
    #[arg(short, long, default_value = "2:1", value_parser = parse_ratio)]
    pub ratio: IsometricRatio,

    /// Show detection details
    #[arg(long)]
    pub verbose: bool,

    /// Output size (longest side in pixels)
    #[arg(short, long, default_value = "256")]
    pub size: u32,
}

impl Cli {
    pub fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            let stem = self.input.file_stem().unwrap_or_default().to_string_lossy();
            let parent = self.input.parent().unwrap_or(std::path::Path::new("."));
            parent.join(format!("{}_corrected.png", stem))
        })
    }
}

fn parse_ratio(s: &str) -> Result<IsometricRatio, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid ratio format '{}', expected N:M", s));
    }

    let horizontal: f64 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid horizontal value: {}", parts[0]))?;
    let vertical: f64 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid vertical value: {}", parts[1]))?;

    if horizontal <= 0.0 || vertical <= 0.0 {
        return Err("Ratio values must be positive".to_string());
    }

    Ok(IsometricRatio::new(horizontal, vertical))
}
