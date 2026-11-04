use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eyre::Result;
use pp_core::config::{LogConfig, PhotoConfig};
use pp_database::jobs::JobKind;
use pp_image_worker::{
    config::WorkerConfig,
    handlers::HandlerRegistry,
    process_photo::{ProcessContext, process_photo},
    render,
    runtime::Worker,
};
use pp_storage::PhotoStorage;
use tokio::{
    signal::{
        self,
        unix::{SignalKind, signal},
    },
    task,
};
use tokio_util::sync::CancellationToken;

#[derive(Parser, ConfigParser)]
#[command(name = "pp-image-worker", about = "Gallery image worker")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "PP_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    log: LogConfig,
    #[command(flatten)]
    worker: WorkerConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    photo: PhotoConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse_with_config();
    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    let storage = PhotoStorage::open(&cli.photo.dir).await?;

    render::install()?;

    tracing::info!("image worker started");

    // Listening starts before the database is reached so a stop signal during startup is
    // not lost.
    let mut sigterm = signal(SignalKind::terminate())?;
    let shutdown = CancellationToken::new();
    let signals = task::spawn({
        let shutdown = shutdown.clone();
        async move {
            tokio::select! {
                _ = signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            shutdown.cancel();
        }
    });

    let max_pixels = cli.worker.photo_process.max_pixels;
    let worker = Worker::connect(cli.worker, |database| {
        let context = Arc::new(ProcessContext {
            database: database.clone(),
            storage,
            max_pixels,
        });

        HandlerRegistry::default().register(JobKind::ProcessPhoto, move |payload| {
            process_photo(context.clone(), payload)
        })
    })
    .await?;

    worker.run(shutdown).await;
    signals.abort();

    tracing::info!("image worker stopping");
    Ok(())
}
