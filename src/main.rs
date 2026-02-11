//!
//! A Rust implementation of NanoClaw project structure.
//! Features:
//! - WhatsApp integration via MCP
//! - Telegram integration via Bot API
//! - Containerized agent execution
//! - Scheduled task management
//! - SQLite persistence

use nuclaw::config;
use nuclaw::container_runner::{self, ensure_container_system_running};
use nuclaw::db;
use nuclaw::error::{NuClawError, Result};
use nuclaw::task_scheduler::TaskScheduler;
use nuclaw::telegram;
use nuclaw::whatsapp;

use structopt::StructOpt;
use tokio::signal;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(long)]
    auth: bool,

    #[structopt(long)]
    scheduler: bool,

    #[structopt(long)]
    whatsapp: bool,

    #[structopt(long)]
    telegram: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::from_args();

    // Setup logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Starting NuClaw v1.0.0");
    info!("This is a Rust port of NanoClaw");

    // Ensure directories exist
    config::ensure_directories().map_err(|e| NuClawError::FileSystem {
        message: e.to_string()
    })?;

    // Initialize database
    let db = db::Database::new().map_err(|e| NuClawError::Database {
        message: e.to_string()
    })?;
    info!("Database initialized successfully");

    // Handle different modes
    if args.scheduler {
        // Run task scheduler
        run_scheduler(db).await?;
    } else if args.whatsapp {
        // Run WhatsApp bot
        run_whatsapp_bot(db).await?;
    } else if args.telegram {
        // Run Telegram bot
        run_telegram_bot(db).await?;
    } else if args.auth {
        // Show authentication QR code
        run_auth_flow().await?;
    } else {
        // Default: run main application with all features
        run_main_application(db).await?;
    }

    Ok(())
}

/// Run the main application with all features
async fn run_main_application(db: db::Database) -> Result<()> {
    info!("Running main application...");

    // Ensure container system is running
    ensure_container_system_running().ok();

    // Setup signal handlers for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Clone db for the scheduler
    let scheduler_db = db.clone();

    // Run scheduler in background
    let scheduler_handle = tokio::spawn(async move {
        let mut scheduler = TaskScheduler::new(scheduler_db);
        let _ = scheduler.run().await;
    });

    // Run WhatsApp bot in background
    let _whatsapp_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    info!("NuClaw is running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal...");
        }
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal...");
        }
    }

    // Graceful shutdown
    let _ = shutdown_tx.send(()).await;
    scheduler_handle.abort();

    info!("NuClaw shutdown complete.");
    Ok(())
}

/// Run the task scheduler
async fn run_scheduler(db: db::Database) -> Result<()> {
    info!("Starting task scheduler...");

    let mut scheduler = TaskScheduler::new(db);
    scheduler.run().await?;

    Ok(())
}

/// Run the WhatsApp bot
async fn run_whatsapp_bot(db: db::Database) -> Result<()> {
    info!("Starting WhatsApp bot...");

    // Check if WhatsApp MCP is configured
    if std::env::var("WHATSAPP_MCP_URL").is_err() {
        info!("WHATSAPP_MCP_URL not set. Run with --auth to set up authentication.");
        info!("Then start the WhatsApp MCP server and run with --whatsapp.");
        return Ok(());
    }

    // Create WhatsApp client
    let mut client = whatsapp::WhatsAppClient::new(db);

    // Connect to WhatsApp
    client.connect().await?;
    info!("Connected to WhatsApp");

    // Start message listener
    client.start_message_listener().await;

    Ok(())
}

/// Run the authentication flow
async fn run_auth_flow() -> Result<()> {
    info!("Starting authentication flow...");

    whatsapp::start_auth_flow().await;
    info!("Use WHATSAPP_MCP_URL to configure WhatsApp connection");

    Ok(())
}

/// Run the Telegram bot
async fn run_telegram_bot(db: db::Database) -> Result<()> {
    info!("Starting Telegram bot...");

    // Check if Telegram bot token is configured
    if std::env::var("TELEGRAM_BOT_TOKEN").is_err() {
        info!("TELEGRAM_BOT_TOKEN not set. Configure it to use Telegram bot.");
        info!("Usage:");
        info!("  export TELEGRAM_BOT_TOKEN=your_bot_token");
        info!("  export TELEGRAM_WEBHOOK_URL=https://your-domain.com");
        info!("  ./nuclaw --telegram");
        return Ok(());
    }

    // Create Telegram client
    let mut client = telegram::TelegramClient::new(db)?;

    // Connect to Telegram
    client.connect().await?;
    info!("Connected to Telegram");

    // Start webhook server
    client.start_webhook_server().await?;

    Ok(())
}
