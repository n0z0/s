#[cfg(unix)]
pub mod unix {
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::sync::broadcast;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::protocol::{MSG_DATA, MSG_RESIZE, MSG_DETACH};
    use crate::session::{socket_path, log_path, pid_path, cleanup_stale_session};

    struct ServerState {
        master_write: Arc<Mutex<File>>,
        master_raw_fd: i32,
        tx: broadcast::Sender<Vec<u8>>,
    }

    pub async fn run_server(session_name: &str, custom_shell: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let sock_p = socket_path(session_name);
        let log_p = log_path(session_name);
        let pid_p = pid_path(session_name);

        // Clean up stale session files if any
        cleanup_stale_session(session_name);

        if sock_p.exists() {
            return Err(format!("Socket already exists at {:?}", sock_p).into());
        }

        // Save current PID
        let my_pid = std::process::id();
        fs::write(&pid_p, my_pid.to_string())?;

        // Open log file in append mode (File-Backed Buffer)
        let log_file = Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_p)?
        ));

        // Create PTY pair
        let pty = nix::pty::openpty(None, None)?;
        let master_fd = pty.master.into_raw_fd();
        let slave_fd = pty.slave.into_raw_fd();

        // Determine shell to execute
        let shell = custom_shell
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        // Spawn child process attached to slave PTY
        let mut cmd = Command::new(&shell);
        unsafe {
            cmd.pre_exec(move || {
                // Create a new session
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Acquire controlling terminal on stdin (slave PTY is on fd 0)
                libc::ioctl(0, libc::TIOCSCTTY as _, 0);
                libc::tcsetpgrp(0, libc::getpid());
                Ok(())
            });
        }

        let slave_in = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
        let slave_out = unsafe { Stdio::from_raw_fd(libc::dup(slave_fd)) };
        let slave_err = unsafe { Stdio::from_raw_fd(slave_fd) };

        cmd.stdin(slave_in)
           .stdout(slave_out)
           .stderr(slave_err)
           .env("TERM", std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()))
           .env("S_SESSION", session_name);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = fs::remove_file(&pid_p);
                return Err(format!("Failed to spawn shell '{}': {}", shell, e).into());
            }
        };

        // Channel for broadcasting PTY output to all connected clients
        let (tx, _rx) = broadcast::channel::<Vec<u8>>(1024);

        // Duplicate master_fd for separate reading and writing
        let master_read_fd = unsafe { libc::dup(master_fd) };
        let mut master_read_file = unsafe { File::from_raw_fd(master_read_fd) };
        let master_write_file = Arc::new(Mutex::new(unsafe { File::from_raw_fd(master_fd) }));

        let state = Arc::new(ServerState {
            master_write: master_write_file,
            master_raw_fd: master_fd,
            tx: tx.clone(),
        });

        // Dedicated thread to read from PTY master and write to log + broadcast to clients
        let log_file_clone = Arc::clone(&log_file);
        let tx_clone = tx.clone();
        let (pty_eof_tx, mut pty_eof_rx) = tokio::sync::mpsc::channel::<()>(1);

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match master_read_file.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let chunk = &buf[..n];
                        // 1. Persist to log file immediately
                        if let Ok(mut file) = log_file_clone.lock() {
                            let _ = file.write_all(chunk);
                            let _ = file.flush();
                        }
                        // 2. Broadcast to connected clients
                        let _ = tx_clone.send(chunk.to_vec());
                    }
                    Err(e) => {
                        // On Linux PTYs, EIO indicates slave was closed / child process exited
                        if e.raw_os_error() == Some(libc::EIO) {
                            break;
                        }
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        break;
                    }
                }
            }
            let _ = pty_eof_tx.blocking_send(());
        });

        // Bind Unix Domain Socket
        let listener = UnixListener::bind(&sock_p)?;

        // Setup cleanup guard
        let cleanup_sock = sock_p.clone();
        let cleanup_pid = pid_p.clone();

        tokio::select! {
            _ = pty_eof_rx.recv() => {
                // Shell exited
            }
            _ = tokio::signal::ctrl_c() => {
                // Interrupted
            }
            res = accept_clients(listener, state) => {
                if let Err(e) = res {
                    eprintln!("[s-server error] {}", e);
                }
            }
        }

        // Wait for child process to finish
        let _ = child.wait();

        // Clean up socket and pid files (keep log file as eternal scrollback buffer)
        let _ = fs::remove_file(&cleanup_sock);
        let _ = fs::remove_file(&cleanup_pid);

        Ok(())
    }

    async fn accept_clients(listener: UnixListener, state: Arc<ServerState>) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let (stream, _) = listener.accept().await?;
            let state_clone = Arc::clone(&state);
            tokio::spawn(async move {
                if let Err(e) = handle_client(stream, state_clone).await {
                    eprintln!("[s-client connection closed: {}]", e);
                }
            });
        }
    }

    async fn handle_client(stream: UnixStream, state: Arc<ServerState>) -> Result<(), Box<dyn std::error::Error>> {
        let (mut reader, mut writer) = stream.into_split();
        let mut rx = state.tx.subscribe();

        // Task 1: Forward PTY output from broadcast channel to this client socket
        let forward_pty_to_client = async move {
            loop {
                match rx.recv().await {
                    Ok(data) => {
                        if writer.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Lagged messages skipped, continue streaming
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        };

        // Task 2: Read client messages from socket and forward to PTY master
        let master_write = Arc::clone(&state.master_write);
        let master_fd = state.master_raw_fd;

        let handle_client_input = async move {
            let mut header_buf = [0u8; 1];
            loop {
                // Read 1-byte message type
                if reader.read_exact(&mut header_buf).await.is_err() {
                    break;
                }
                match header_buf[0] {
                    MSG_DATA => {
                        let mut len_buf = [0u8; 2];
                        if reader.read_exact(&mut len_buf).await.is_err() {
                            break;
                        }
                        let len = u16::from_be_bytes(len_buf) as usize;
                        let mut payload = vec![0u8; len];
                        if reader.read_exact(&mut payload).await.is_err() {
                            break;
                        }
                        // Write to PTY master
                        if let Ok(mut file) = master_write.lock() {
                            let _ = file.write_all(&payload);
                            let _ = file.flush();
                        }
                    }
                    MSG_RESIZE => {
                        let mut dims = [0u8; 4];
                        if reader.read_exact(&mut dims).await.is_err() {
                            break;
                        }
                        let rows = u16::from_be_bytes([dims[0], dims[1]]);
                        let cols = u16::from_be_bytes([dims[2], dims[3]]);
                        let ws = libc::winsize {
                            ws_row: rows,
                            ws_col: cols,
                            ws_xpixel: 0,
                            ws_ypixel: 0,
                        };
                        unsafe {
                            libc::ioctl(master_fd, libc::TIOCSWINSZ as _, &ws);
                        }
                    }
                    MSG_DETACH => {
                        // Client requested detach cleanly
                        break;
                    }
                    _ => {
                        // Unknown message, disconnect
                        break;
                    }
                }
            }
        };

        tokio::select! {
            _ = forward_pty_to_client => {}
            _ = handle_client_input => {}
        }

        Ok(())
    }
}

#[cfg(not(unix))]
pub mod unix {
    pub async fn run_server(_session_name: &str, _shell: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        Err("`s` server requires a Unix/POSIX environment (PTY & Unix Domain Sockets)".into())
    }
}
