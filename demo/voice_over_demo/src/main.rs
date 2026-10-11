mod analyze;
mod cli;
mod config;
mod error;
mod hf;
mod phoneme;
mod render;
mod score;
mod synth;
mod tts;
mod wav;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::error::AppError;

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<String, AppError> {
    if let Some(spec_raw) = &cli.fetch {
        let spec = hf::ModelSpec::parse(spec_raw)?;
        let path = hf::fetch(&spec)?;
        return Ok(format!("cached {} -> {}", spec, path.display()));
    }
    if let Some(text) = &cli.say {
        return say(&cli, text);
    }
    if let Some(path) = &cli.stats_file {
        let (samples, rate) = analyze::read_wav(std::path::Path::new(path))?;
        let st = analyze::stats(&samples, rate);
        return Ok(format!(
            "{} ({:.2}s, {} Hz)\n  {}",
            path,
            samples.len() as f32 / rate as f32,
            rate,
            st
        ));
    }
    let stats = cli.stats;
    let cfg = cli.into_config()?;
    cfg.validate()?;
    for word in &cfg.lyrics {
        phoneme::validate_word(word)?;
    }
    let (samples, rate) = render::render(&cfg);
    wav::write_wav(&cfg.output, rate, &samples)?;
    let mut message = format!(
        "wrote {} ({:.2}s, {} Hz)",
        cfg.output.display(),
        cfg.total_seconds(),
        rate
    );
    if stats {
        let st = analyze::stats(&samples, rate);
        message.push_str(&format!("\n  {st}"));
    }
    Ok(message)
}

fn say(cli: &Cli, text: &str) -> Result<String, AppError> {
    let model = cli
        .model
        .as_deref()
        .ok_or_else(|| AppError::MissingArg("--model".to_owned()))?;
    let config_path = match &cli.model_config {
        Some(path) => std::path::PathBuf::from(path),
        None => std::path::PathBuf::from(format!("{model}.json")),
    };
    let cfg = tts::parse_config(&config_path)?;
    let (samples, rate) = tts::synthesize(std::path::Path::new(model), &cfg, text)?;
    wav::write_wav(std::path::Path::new(&cli.output), rate, &samples)?;
    Ok(format!(
        "wrote {} ({:.2}s, {} Hz)",
        cli.output,
        samples.len() as f32 / rate as f32,
        rate
    ))
}
