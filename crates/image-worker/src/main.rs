use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eyre::Result;
use pp_core::config::{LogConfig, PhotoConfig};
use pp_database::jobs::JobKind;
use pp_image_worker::{
    config::WorkerConfig,
    handlers::{HandlerRegistry, cleanup::CleanupHandler, process_photo::ProcessPhotoHandler},
    render::ImageRenderer,
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

    let renderer = ImageRenderer::new(cli.worker.process.max_pixels)?;
    let storage = PhotoStorage::open(&cli.photo.dir).await?;

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

    let orphan_grace = cli.worker.cleanup.orphan_grace.get();
    let sweep_interval = cli.worker.cleanup.sweep_interval.get();
    let worker = Worker::connect(cli.worker, |database| {
        let process_handler = Arc::new(ProcessPhotoHandler::new(
            database.clone(),
            storage.clone(),
            renderer,
        ));
        let cleanup_handler = Arc::new(CleanupHandler::new(
            database.clone(),
            storage,
            orphan_grace,
            sweep_interval,
        ));

        HandlerRegistry::default()
            .register(JobKind::ProcessPhoto, move |payload, job| {
                Arc::clone(&process_handler).run(payload, job)
            })
            .register(JobKind::CleanupGallery, {
                let cleanup_handler = Arc::clone(&cleanup_handler);

                move |payload, job| Arc::clone(&cleanup_handler).remove_gallery(payload, job)
            })
            .register(JobKind::CleanupPhotos, {
                let cleanup_handler = Arc::clone(&cleanup_handler);

                move |payload, job| Arc::clone(&cleanup_handler).remove_photos(payload, job)
            })
            .register(JobKind::SweepOrphans, move |payload, job| {
                Arc::clone(&cleanup_handler).sweep_orphans(payload, job)
            })
    })
    .await?;

    worker.run(shutdown).await;
    signals.abort();

    tracing::info!("image worker stopping");
    Ok(())
}
