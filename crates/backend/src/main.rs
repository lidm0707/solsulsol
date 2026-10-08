//! Composition root: all layers assembled here. Binary entry point.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pos_backend::core::run::run;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(std::io::Error::other)
        .and_then(|rt| {
            rt.block_on(async move {
                let services = match pos_backend::core::state::wire_services().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("wiring error: {e}");
                        std::process::exit(1);
                    }
                };
                run(services).await
            })
        })
        .unwrap_or_else(|e| {
            eprintln!("server error: {e}");
            std::process::exit(1);
        });
}
