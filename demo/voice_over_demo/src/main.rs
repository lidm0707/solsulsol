mod cli;
mod config;
mod error;
mod phoneme;
mod render;
mod score;
mod synth;
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
    let cfg = cli.into_config()?;
    cfg.validate()?;
    for word in &cfg.lyrics {
        phoneme::validate_word(word)?;
    }
    let (samples, rate) = render::render(&cfg);
    wav::write_wav(&cfg.output, rate, &samples)?;
    Ok(format!(
        "wrote {} ({:.2}s, {} Hz)",
        cfg.output.display(),
        cfg.total_seconds(),
        rate
    ))
}
