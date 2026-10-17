mod cli;
mod client;
mod protocol;
mod server;
mod session;

use clap::Parser;
use cli::{Cli, Commands, parse_detach_key};
use session::{list_sessions, log_path, pid_path, sanitize_session_name, socket_path};
use std::fs;

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let detach_byte = parse_detach_key(&cli.detach_key);

    match cli.command {
        Some(Commands::Server { name }) => {
            let session_name = sanitize_session_name(&name)?;
            server::unix::run_server(&session_name, cli.shell).await?;
        }

        Some(Commands::Attach { name }) => {
            let session_name = sanitize_session_name(&name)?;
            let replay_mode = client::resolve_replay_mode(cli.all, cli.lines);
            client::unix::attach_or_create(&session_name, cli.shell, detach_byte, replay_mode).await?;
        }

        Some(Commands::List) => {
            let sessions = list_sessions()?;
            if sessions.is_empty() {
                println!("No active `s` sessions found in {:?}", session::session_dir());
                return Ok(());
            }

            println!("{:<16} {:<10} {:<8} {:<12} {:<24} {}", "SESSION", "STATUS", "PID", "LOG SIZE", "SOCKET", "LOG FILE");
            println!("{:-<16} {:-<10} {:-<8} {:-<12} {:-<24} {:-<24}", "", "", "", "", "", "");

            for s in sessions {
                let status = if s.is_alive { "Active" } else { "Stale" };
                let pid_str = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
                let size_str = format_bytes(s.log_size_bytes);
                println!(
                    "{:<16} {:<10} {:<8} {:<12} {:<24} {}",
                    s.name,
                    status,
                    pid_str,
                    size_str,
                    s.socket_path.display(),
                    s.log_path.display()
                );
            }
        }

        Some(Commands::Kill { name }) => {
            let session_name = sanitize_session_name(&name)?;
            let pid_p = pid_path(&session_name);
            let sock_p = socket_path(&session_name);
            let log_p = log_path(&session_name);

            if let Ok(pid_str) = fs::read_to_string(&pid_p) {
                if let Ok(_pid) = pid_str.trim().parse::<i32>() {
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(_pid, libc::SIGTERM);
                    }
                }
            }

            let _ = fs::remove_file(&sock_p);
            let _ = fs::remove_file(&pid_p);
            let _ = fs::remove_file(&log_p);
            println!("[s: session '{}' terminated]", session_name);
        }

        Some(Commands::Log { name, path }) => {
            let session_name = sanitize_session_name(&name)?;
            let log_p = log_path(&session_name);

            if path {
                println!("{}", log_p.display());
            } else if log_p.exists() {
                let content = fs::read_to_string(&log_p)
                    .unwrap_or_else(|_| "[Unable to read log file as UTF-8]".to_string());
                print!("{}", content);
            } else {
                println!("Log file for session '{}' does not exist yet ({})", session_name, log_p.display());
            }
        }

        None => {
            if let Some(name) = cli.session_name {
                let session_name = sanitize_session_name(&name)?;
                let replay_mode = client::resolve_replay_mode(cli.all, cli.lines);
                client::unix::attach_or_create(&session_name, cli.shell, detach_byte, replay_mode).await?;
            } else {
                // If no session name given, show quickstart guide and active sessions
                println!("s - terminal session manager\n");
                println!("Quickstart:");
                println!("  s <name>           Buat atau masuk ke sesi (mis. `s main`)");
                println!("  s <name> -n 100    Masuk dengan memuat 100 baris riwayat terakhir");
                println!("  s <name> --all     Masuk dengan memuat seluruh riwayat log dari awal");
                println!("  s list (atau ls)   Lihat daftar sesi aktif");
                println!("  s kill <name>      Matikan sesi (mis. `s kill main`)");
                println!("  s log <name>       Lihat isi log riwayat sesi (tambah -p untuk path)");
                println!("  Ctrl + \\           Detach / keluar sementara dari dalam sesi");

                let sessions = list_sessions()?;
                if !sessions.is_empty() {
                    println!("\nSesi aktif saat ini:");
                    for s in sessions {
                        let status = if s.is_alive { "active" } else { "stale" };
                        let pid_str = s.pid.map(|p| format!("pid: {}", p)).unwrap_or_else(|| "no-pid".to_string());
                        let size_str = format_bytes(s.log_size_bytes);
                        println!("  * {:<14} ({}, {}, log: {})", s.name, status, pid_str, size_str);
                    }
                    println!("\nKetik `s <nama>` untuk masuk ke sesi.");
                } else {
                    println!("\nBelum ada sesi aktif. Jalankan `s main` untuk memulai.");
                }
            }
        }
    }

    Ok(())
}
