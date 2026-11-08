use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eyre::Result;
use pp_core::config::{ExportConfig, LogConfig, PhotoConfig};
use pp_database::jobs::JobKind;
use pp_image_worker::{
    config::WorkerConfig,
    handlers::{
        CompleteJob, HandlerRegistry,
        cleanup::CleanupHandler,
        export::{ExportFinalizer, ExportHandler},
        process_photo::{ProcessPhotoFinalizer, ProcessPhotoHandler},
    },
    maintenance::Maintenance,
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
    #[command(flatten)]
    #[config(no_flatten)]
    export: ExportConfig,
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
    let notification_retention = cli.worker.cleanup.notification_retention.get();
    let retention = cli.export.retention.get();
    let maintenance_storage = storage.clone();
    let worker = Worker::connect(cli.worker, |database| {
        let process_handler = Arc::new(ProcessPhotoHandler::new(
            database.clone(),
            storage.clone(),
            renderer,
        ));
        let process_finalizer = ProcessPhotoFinalizer::new(database.clone(), storage.clone());
        let export_handler = Arc::new(ExportHandler::new(database.clone(), storage.clone()));
        let export_finalizer = ExportFinalizer::new(database.clone(), storage.clone());
        let cleanup_handler = Arc::new(CleanupHandler::new(storage));

        HandlerRegistry::default()
            .register(
                JobKind::ProcessPhoto,
                move |payload| Arc::clone(&process_handler).run(payload),
                process_finalizer,
            )
            .register(
                JobKind::CleanupGallery,
                {
                    let cleanup_handler = Arc::clone(&cleanup_handler);

                    move |payload| Arc::clone(&cleanup_handler).remove_gallery(payload)
                },
                CompleteJob::new(database.clone()),
            )
            .register(
                JobKind::CleanupPhotos,
                move |payload| Arc::clone(&cleanup_handler).remove_photos(payload),
                CompleteJob::new(database.clone()),
            )
            .register(
                JobKind::ZipExport,
                move |payload| Arc::clone(&export_handler).run(payload),
                export_finalizer,
            )
    })
    .await?;
    let maintenance = Maintenance::new(
        worker.database().clone(),
        maintenance_storage,
        sweep_interval,
        orphan_grace,
        retention,
        notification_retention,
    );
    let worker = worker.with_maintenance(maintenance);

    worker.run(shutdown).await;
    signals.abort();

    tracing::info!("image worker stopping");
    Ok(())
}
