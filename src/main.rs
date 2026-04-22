//!
//! A Rust implementation of NuClaw project structure.
//! Features:
//! - WhatsApp integration via MCP
//! - Telegram integration via Bot API
//! - Containerized agent execution
//! - Scheduled task management
//! - SQLite persistence

use nuclaw::config;
use nuclaw::container_runner::ensure_container_system_running;
use nuclaw::db;
use nuclaw::error::{NuClawError, Result};
use nuclaw::feishu;
use nuclaw::logging;
use nuclaw::onboard;
use nuclaw::task_scheduler::TaskScheduler;
use nuclaw::telegram;
use nuclaw::whatsapp;

use clap::Parser;
use tokio::signal;
use tracing::{info, warn};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    auth: bool,

    #[arg(long)]
    scheduler: bool,

    #[arg(long)]
    whatsapp: bool,

    #[arg(long)]
    telegram: bool,

    #[arg(long)]
    weixin: bool,

    #[arg(long)]
    feishu: bool,

    #[structopt(long)]
    onboard: bool,

    #[structopt(long)]
    start: bool,

    #[structopt(long)]
    stop: bool,

    #[structopt(long)]
    restart: bool,

    #[structopt(long)]
    status: bool,

    #[structopt(long)]
    telegram_pair: bool,

    #[structopt(long)]
    telegram_pair_list: bool,

    #[structopt(long)]
    telegram_pair_revoke: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load .env file if exists (auto-loaded for convenience)
    config::load_env_file();

    // Initialize logging
    logging::init();

    info!("Starting NuClaw v1.0.0");
    info!("This is a Rust port of NuClaw");

    // Ensure directories exist
    config::ensure_directories().map_err(|e| NuClawError::FileSystem {
        message: e.to_string(),
    })?;

    // Initialize database
    let db = db::Database::new().map_err(|e| NuClawError::Database {
        message: e.to_string(),
    })?;
    info!("Database initialized successfully");

    // Handle different modes
    if args.start {
        run_start_command()?;
    } else if args.stop {
        run_stop_command()?;
    } else if args.restart {
        run_restart_command()?;
    } else if args.status {
        run_status_command()?;
    } else if args.telegram_pair {
        run_telegram_pair_command()?;
    } else if args.telegram_pair_list {
        run_telegram_pair_list_command()?;
    } else if args.telegram_pair_revoke.is_some() {
        run_telegram_pair_revoke_command(args.telegram_pair_revoke.unwrap())?;
    } else if args.scheduler {
        // Run task scheduler
        run_scheduler(db).await?;
    } else if args.whatsapp {
        // Run WhatsApp bot
        run_whatsapp_bot(db).await?;
    } else if args.telegram {
        // Run Telegram bot
        run_telegram_bot(db).await?;
    } else if args.feishu {
        // Run Feishu bot
        run_feishu_bot(db).await?;
    } else if args.auth {
        // Show authentication QR code
        run_auth_flow().await?;
    } else if args.onboard {
        // Run onboard wizard
        onboard::run_onboard()?;
    } else {
        // Default: run main application with all features
        run_main_application(db).await?;
    }

    Ok(())
}

/// Run the main application with all features
async fn run_main_application(db: db::Database) -> Result<()> {
    info!("Running main application...");

    // Ensure container system is running and log any errors
    if let Err(e) = ensure_container_system_running() {
        warn!(
            "Container system not available: {}. Continuing anyway...",
            e
        );
    }

    // Setup signal handlers for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Clone db for the scheduler
    let scheduler_db = db.clone();

    // Run scheduler in background
    let scheduler_handle = tokio::spawn(async move {
        let mut scheduler = TaskScheduler::new(scheduler_db);
        let _ = scheduler.run().await;
    });

    // Auto-start WhatsApp bot if WHATSAPP_MCP_URL is configured
    let whatsapp_db = db.clone();
    let _whatsapp_handle = tokio::spawn(async move {
        if whatsapp::is_configured() {
            info!("Auto-starting WhatsApp bot (WHATSAPP_MCP_URL is set)...");
            match run_whatsapp_bot_internal(whatsapp_db).await {
                Ok(_) => info!("WhatsApp bot started successfully"),
                Err(e) => warn!("Failed to start WhatsApp bot: {}. Continuing without WhatsApp...", e),
            }
        } else {
            info!("WHATSAPP_MCP_URL not set. WhatsApp bot will not auto-start.");
        }
        // Keep task alive - WhatsApp uses long polling
        futures::future::pending::<()>().await;
    });

    // Auto-start Telegram bot if TELEGRAM_BOT_TOKEN is configured
    let telegram_db = db.clone();
    let telegram_handle = tokio::spawn(async move {
        if telegram::should_auto_start_telegram() {
            info!("Auto-starting Telegram bot (TELEGRAM_BOT_TOKEN is set)...");
            match run_telegram_bot_internal(telegram_db).await {
                Ok(_) => info!("Telegram bot started successfully"),
                Err(e) => warn!("Failed to start Telegram bot: {}. Continuing without Telegram...", e),
            }
        } else {
            info!("TELEGRAM_BOT_TOKEN not set. Telegram bot will not auto-start.");
        }
    });

    // Auto-start Feishu bot if FEISHU_APP_ID and FEISHU_APP_SECRET are configured
    let feishu_db = db.clone();
    let feishu_handle = tokio::spawn(async move {
        if feishu::should_auto_start_feishu() {
            info!("Auto-starting Feishu bot (FEISHU_APP_ID is set)...");
            match run_feishu_bot_internal(feishu_db).await {
                Ok(_) => info!("Feishu bot started successfully"),
                Err(e) => warn!("Failed to start Feishu bot: {}. Continuing without Feishu...", e),
            }
        } else {
            info!("FEISHU_APP_ID not set. Feishu bot will not auto-start.");
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
    telegram_handle.abort();
    feishu_handle.abort();

    info!("NuClaw shutdown complete.");
    Ok(())
}

/// Internal function to start Telegram bot (used by auto-start)
async fn run_telegram_bot_internal(db: db::Database) -> Result<()> {
    // Check if Telegram bot token is configured
    if std::env::var("TELEGRAM_BOT_TOKEN").is_err() {
        return Err(NuClawError::Config {
            message: "TELEGRAM_BOT_TOKEN not set".to_string(),
        });
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

/// Internal function to start Feishu bot (used by auto-start)
async fn run_feishu_bot_internal(db: db::Database) -> Result<()> {
    let mut client = feishu::FeishuClient::new(db)?;
    client.connect().await?;
    info!("Connected to Feishu");
    client.start_webhook_server().await?;
    Ok(())
}

/// Run the Feishu bot
async fn run_feishu_bot(db: db::Database) -> Result<()> {
    info!("Starting Feishu bot...");
    if std::env::var("FEISHU_APP_ID").is_err() {
        info!("FEISHU_APP_ID not set. Configure it to use Feishu bot.");
        info!("Usage:");
        info!("  export FEISHU_APP_ID=your_app_id");
        info!("  export FEISHU_APP_SECRET=your_app_secret");
        info!("  ./nuclaw --feishu");
        return Ok(());
    }
    run_feishu_bot_internal(db).await
}
async fn run_scheduler(db: db::Database) -> Result<()> {
    info!("Starting task scheduler...");

    let mut scheduler = TaskScheduler::new(db);
    scheduler.run().await?;

    Ok(())
}

/// Internal function to start WhatsApp bot (used by auto-start)
async fn run_whatsapp_bot_internal(db: db::Database) -> Result<()> {
    let runtime = std::sync::Arc::new(nuclaw::runtime::DockerRuntime);
    let router = std::sync::Arc::new(nuclaw::router::EventRouter::new(runtime));
    let mut client = nuclaw::whatsapp::WhatsAppClient::new(db, router);
    client.connect().await?;
    info!("Connected to WhatsApp");
    client.start_message_listener().await;
    Ok(())
}

/// Run the WhatsApp bot
async fn run_whatsapp_bot(db: db::Database) -> Result<()> {
    info!("Starting WhatsApp bot...");
    if std::env::var("WHATSAPP_MCP_URL").is_err() {
        info!("WHATSAPP_MCP_URL not set. Run with --auth to set up authentication.");
        info!("Then start the WhatsApp MCP server and run with --whatsapp.");
        return Ok(());
    }
    run_whatsapp_bot_internal(db).await
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

    // Use internal function
    run_telegram_bot_internal(db).await
}

fn pid_file_path() -> std::path::PathBuf {
    config::nuclaw_home().join("nuclaw.pid")
}

fn read_pid() -> Option<u32> {
    std::fs::read_to_string(pid_file_path())
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn write_pid(pid: u32) -> std::io::Result<()> {
    std::fs::write(pid_file_path(), pid.to_string())
}

fn remove_pid_file() -> std::io::Result<()> {
    std::fs::remove_file(pid_file_path())
}

fn is_process_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_start_command() -> Result<()> {
    config::load_env_file();

    if let Some(pid) = read_pid() {
        if is_process_running(pid) {
            println!("NuClaw is already running (PID: {})", pid);
            println!("Use 'nuclaw --restart' to restart");
            return Ok(());
        } else {
            let _ = remove_pid_file();
        }
    }

    let exe_path = std::env::current_exe().map_err(|e| NuClawError::FileSystem {
        message: e.to_string(),
    })?;

    let child =
        std::process::Command::new(&exe_path)
            .spawn()
            .map_err(|e| NuClawError::FileSystem {
                message: e.to_string(),
            })?;

    let pid = child.id();
    write_pid(pid).map_err(|e| NuClawError::FileSystem {
        message: e.to_string(),
    })?;

    println!("✓ NuClaw started (PID: {})", pid);
    println!("  Use 'nuclaw --status' to check status");
    println!("  Use 'nuclaw --stop' to stop");
    println!("  Use 'nuclaw --restart' to restart");

    Ok(())
}

fn run_stop_command() -> Result<()> {
    if let Some(pid) = read_pid() {
        if is_process_running(pid) {
            println!("Stopping NuClaw (PID: {})...", pid);
            std::process::Command::new("kill")
                .arg(pid.to_string())
                .output()
                .map_err(|e| NuClawError::FileSystem {
                    message: e.to_string(),
                })?;

            for _ in 0..10 {
                if !is_process_running(pid) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            if is_process_running(pid) {
                if let Err(e) = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output()
                {
                    warn!("Failed to kill process {}: {}", pid, e);
                }
            }

            remove_pid_file().ok();
            println!("✓ NuClaw stopped");
        } else {
            println!("NuClaw is not running (stale PID file)");
            remove_pid_file().ok();
        }
    } else {
        println!("NuClaw is not running");
    }

    Ok(())
}

fn run_restart_command() -> Result<()> {
    run_stop_command()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    run_start_command()
}

fn run_status_command() -> Result<()> {
    let telegram_status = if telegram::should_auto_start_telegram() {
        "Auto-enabled (TELEGRAM_BOT_TOKEN set)"
    } else {
        "Disabled (run with --telegram to enable)"
    };
    
    if let Some(pid) = read_pid() {
        if is_process_running(pid) {
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║                        NuClaw Status                        ║");
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║  Status:    Running                                         ║");
            println!(
                "║  PID:       {}                                             ║",
                pid
            );
            println!(
                "║  Telegram:  {}                                             ║",
                telegram_status
            );
            println!("╚══════════════════════════════════════════════════════════════╝");
        } else {
            println!("NuClaw is not running (stale PID file: {})", pid);
            remove_pid_file().ok();
        }
    } else {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                        NuClaw Status                        ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║  Status:    Not running                                     ║");
        println!(
            "║  Telegram:  {}                                             ║",
            telegram_status
        );
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!("Run 'nuclaw --start' to start the service");
    }

    Ok(())
}

fn run_telegram_pair_command() -> Result<()> {
    use nuclaw::telegram::PairingManager;

    let _bot_token = std::env::var("TELEGRAM_BOT_TOKEN").map_err(|_| NuClawError::Config {
        message: "TELEGRAM_BOT_TOKEN not set".to_string(),
    })?;

    let mut manager = PairingManager::new()?;
    let code = manager.generate_code("pending", 0)?;

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     Pairing Information                   ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!(
        "║  Pairing Code: {}                                       ║",
        code
    );
    println!("║  Expires in: 10 minutes                                 ║");
    println!("║                                                              ║");
    println!("║  Instructions:                                           ║");
    println!("║  1. Open Telegram and find your Bot                      ║");
    println!(
        "║  2. Send the pairing code: {}                         ║",
        code
    );
    println!("║  3. Wait for authorization confirmation                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Note: When a user sends the pairing code to the bot,");
    println!("      they will be automatically authorized.");
    println!();
    println!("The bot must be running to complete pairing.");
    println!("Run 'nuclaw --telegram' to start the bot, or");
    println!("run 'nuclaw --start' if you want the full service.");

    Ok(())
}

fn run_telegram_pair_list_command() -> Result<()> {
    use nuclaw::telegram::PairingManager;

    let manager = PairingManager::new()?;
    let authorized = manager.list_authorized();

    if authorized.is_empty() {
        println!("No authorized users.");
        println!("Run 'nuclaw --telegram-pair' to generate a pairing code.");
    } else {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                   Authorized Users                        ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        for user in authorized {
            println!(
                "║  User ID: {}                                          ║",
                user.user_id
            );
            println!(
                "║  Chat ID: {}                                          ║",
                user.chat_id
            );
            println!(
                "║  Authorized: {}                                        ║",
                user.authorized_at
            );
            println!("╟──────────────────────────────────────────────────────────────╢");
        }
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    Ok(())
}

fn run_telegram_pair_revoke_command(user_id: String) -> Result<()> {
    use nuclaw::telegram::PairingManager;

    let mut manager = PairingManager::new()?;
    if manager.deauthorize_user(&user_id)? {
        println!("✓ User {} revoked successfully.", user_id);
    } else {
        println!("User {} not found.", user_id);
    }

    Ok(())
}
