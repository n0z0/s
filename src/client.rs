#[cfg(unix)]
pub mod unix {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tokio::signal::unix::{signal, SignalKind};

    use crate::protocol::ClientMessage;
    use crate::session::{is_socket_alive, log_path, socket_path};

    struct RawModeGuard;

    impl RawModeGuard {
        pub fn enter() -> std::io::Result<Self> {
            crossterm::terminal::enable_raw_mode()?;
            Ok(RawModeGuard)
        }
    }

    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }

    fn get_terminal_size() -> (u16, u16) {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        let mut res = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ as _, &mut ws) };
        if res != 0 {
            res = unsafe { libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ as _, &mut ws) };
        }
        if res == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            (ws.ws_col, ws.ws_row)
        } else {
            (80, 24)
        }
    }

    pub async fn attach_or_create(
        session_name: &str,
        custom_shell: Option<String>,
        detach_byte: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let sock_p = socket_path(session_name);
        let log_p = log_path(session_name);

        // 1. Ensure server is running; spawn if not
        if !is_socket_alive(&sock_p) {
            let current_exe = std::env::current_exe()?;
            let mut cmd = Command::new(current_exe);
            cmd.arg("server").arg(session_name);
            if let Some(ref sh) = custom_shell {
                cmd.arg("--shell").arg(sh);
            }
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            cmd.spawn()?;

            // Wait for socket to become available (up to 3 seconds)
            let mut ready = false;
            for _ in 0..60 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if is_socket_alive(&sock_p) {
                    ready = true;
                    break;
                }
            }

            if !ready {
                return Err(format!("Timeout waiting for session '{}' server to initialize", session_name).into());
            }
        }

        println!("[s: attached to '{}' (detach: Ctrl+\\)]", session_name);

        // 2. History Loader: Read persistent log file and replay to local screen
        if log_p.exists() {
            if let Ok(mut file) = File::open(&log_p) {
                let mut stdout = std::io::stdout();
                let mut buffer = [0u8; 8192];
                while let Ok(n) = file.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    let _ = stdout.write_all(&buffer[..n]);
                }
                let _ = stdout.flush();
            }
        }

        // 3. Connect to Unix Domain Socket
        let stream = UnixStream::connect(&sock_p).await?;
        let (mut sock_read, mut sock_write) = stream.into_split();

        // 4. Send initial terminal window size to server
        let (cols, rows) = get_terminal_size();
        let resize_msg = ClientMessage::Resize { rows, cols };
        let _ = sock_write.write_all(&resize_msg.encode()).await;

        // 5. Enter terminal raw mode with RAII cleanup guard
        let _guard = RawModeGuard::enter()?;

        // 6. Setup SIGWINCH listener for dynamic terminal resizing
        let mut sigwinch = signal(SignalKind::window_change())?;

        // 7. Interactive forwarder loop
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        let mut stdin_buf = [0u8; 1024];
        let mut sock_buf = [0u8; 4096];

        enum ExitReason {
            Detached,
            Closed,
            Error(std::io::Error),
        }

        let exit_reason: ExitReason;

        loop {
            tokio::select! {
                // Window resize signal from OS
                _ = sigwinch.recv() => {
                    let (cols, rows) = get_terminal_size();
                    let resize_msg = ClientMessage::Resize { rows, cols };
                    if sock_write.write_all(&resize_msg.encode()).await.is_err() {
                        exit_reason = ExitReason::Closed;
                        break;
                    }
                }

                // Output received from Server / PTY
                sock_res = sock_read.read(&mut sock_buf) => {
                    match sock_res {
                        Ok(0) => {
                            // Server closed connection (shell exited)
                            exit_reason = ExitReason::Closed;
                            break;
                        }
                        Ok(n) => {
                            if let Err(e) = stdout.write_all(&sock_buf[..n]).await {
                                exit_reason = ExitReason::Error(e);
                                break;
                            }
                            let _ = stdout.flush().await;
                        }
                        Err(e) => {
                            exit_reason = ExitReason::Error(e);
                            break;
                        }
                    }
                }

                // Input typed by user on local keyboard
                stdin_res = stdin.read(&mut stdin_buf) => {
                    match stdin_res {
                        Ok(0) => {
                            exit_reason = ExitReason::Detached;
                            break;
                        }
                        Ok(n) => {
                            let input = &stdin_buf[..n];
                            // Check if detach key was pressed
                            if let Some(pos) = input.iter().position(|&b| b == detach_byte) {
                                // Send any characters before the detach key
                                if pos > 0 {
                                    let before = ClientMessage::Data(input[..pos].to_vec());
                                    let _ = sock_write.write_all(&before.encode()).await;
                                }
                                // Notify server of detach
                                let detach_msg = ClientMessage::Detach;
                                let _ = sock_write.write_all(&detach_msg.encode()).await;
                                exit_reason = ExitReason::Detached;
                                break;
                            } else {
                                let data_msg = ClientMessage::Data(input.to_vec());
                                if sock_write.write_all(&data_msg.encode()).await.is_err() {
                                    exit_reason = ExitReason::Closed;
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            exit_reason = ExitReason::Error(e);
                            break;
                        }
                    }
                }
            }
        }

        // Drop raw mode guard before printing status message
        drop(_guard);

        match exit_reason {
            ExitReason::Detached => {
                println!("\r\n[s: detached from session '{}']", session_name);
            }
            ExitReason::Closed => {
                println!("\r\n[s: session '{}' closed]", session_name);
            }
            ExitReason::Error(e) => {
                eprintln!("\r\n[s error: {}]", e);
            }
        }

        Ok(())
    }
}

#[cfg(not(unix))]
pub mod unix {
    pub async fn attach_or_create(
        _session_name: &str,
        _shell: Option<String>,
        _detach_byte: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("`s` client requires a Unix/POSIX terminal environment".into())
    }
}
