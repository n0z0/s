use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Replay up to N bytes from the end of the log, aligning to the next newline
    TailBytes(u64),
    /// Replay up to N lines from the end of the log
    TailLines(usize),
    /// Replay the entire log file from start to finish
    All,
    /// Do not replay any history
    None,
}

/// Default tail buffer size: 64 KB (instant load, sufficient for multiple screens)
pub const DEFAULT_REPLAY_BYTES: u64 = 64 * 1024;

/// Resolve CLI arguments into a concrete ReplayMode
pub fn resolve_replay_mode(all: bool, lines: Option<usize>) -> ReplayMode {
    if all {
        ReplayMode::All
    } else if let Some(n) = lines {
        if n == 0 {
            ReplayMode::None
        } else {
            ReplayMode::TailLines(n)
        }
    } else {
        ReplayMode::TailBytes(DEFAULT_REPLAY_BYTES)
    }
}

/// Stream file-backed scrollback to terminal according to ReplayMode
pub fn replay_log<R: Read + Seek, W: Write>(
    reader: &mut R,
    mode: ReplayMode,
    writer: &mut W,
) -> std::io::Result<()> {
    match mode {
        ReplayMode::None => Ok(()),

        ReplayMode::All => {
            reader.seek(SeekFrom::Start(0))?;
            copy_stream(reader, writer)
        }

        ReplayMode::TailBytes(limit) => {
            let total_len = reader.seek(SeekFrom::End(0))?;
            if total_len <= limit {
                reader.seek(SeekFrom::Start(0))?;
            } else {
                reader.seek(SeekFrom::End(-(limit as i64)))?;
                // Discard partial initial line to align cleanly on a newline
                let mut single = [0u8; 1];
                while reader.read(&mut single)? == 1 {
                    if single[0] == b'\n' {
                        break;
                    }
                }
            }
            copy_stream(reader, writer)
        }

        ReplayMode::TailLines(target_lines) => {
            if target_lines == 0 {
                return Ok(());
            }
            let total_len = reader.seek(SeekFrom::End(0))?;
            if total_len == 0 {
                return Ok(());
            }

            let chunk_size: u64 = 8192;
            let mut lines_found = 0;
            let mut curr_end = total_len;
            let mut start_offset = 0;

            'outer: while curr_end > 0 {
                let to_read = std::cmp::min(chunk_size, curr_end);
                let chunk_start = curr_end - to_read;
                reader.seek(SeekFrom::Start(chunk_start))?;

                let mut buf = vec![0u8; to_read as usize];
                reader.read_exact(&mut buf)?;

                for i in (0..buf.len()).rev() {
                    let abs_pos = chunk_start + (i as u64);
                    // Ignore trailing newline at the very end of file
                    if abs_pos == total_len - 1 && buf[i] == b'\n' {
                        continue;
                    }

                    if buf[i] == b'\n' {
                        lines_found += 1;
                        if lines_found >= target_lines {
                            start_offset = abs_pos + 1;
                            break 'outer;
                        }
                    }
                }

                curr_end = chunk_start;
            }

            reader.seek(SeekFrom::Start(start_offset))?;
            copy_stream(reader, writer)
        }
    }
}

fn copy_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> std::io::Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buffer[..n])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(unix)]
pub mod unix {
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tokio::signal::unix::{signal, SignalKind};

    use super::{replay_log, ReplayMode};
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
        replay_mode: ReplayMode,
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

        // 2. History Loader: Read persistent log file according to ReplayMode
        if log_p.exists() {
            if let Ok(mut file) = File::open(&log_p) {
                let mut stdout = std::io::stdout();
                let _ = replay_log(&mut file, replay_mode, &mut stdout);
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
    use super::ReplayMode;

    pub async fn attach_or_create(
        _session_name: &str,
        _shell: Option<String>,
        _detach_byte: u8,
        _replay_mode: ReplayMode,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("`s` client requires a Unix/POSIX terminal environment".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_resolve_replay_mode() {
        assert_eq!(resolve_replay_mode(true, None), ReplayMode::All);
        assert_eq!(resolve_replay_mode(true, Some(50)), ReplayMode::All);
        assert_eq!(resolve_replay_mode(false, Some(0)), ReplayMode::None);
        assert_eq!(resolve_replay_mode(false, Some(100)), ReplayMode::TailLines(100));
        assert_eq!(resolve_replay_mode(false, None), ReplayMode::TailBytes(DEFAULT_REPLAY_BYTES));
    }

    #[test]
    fn test_replay_log_none() {
        let mut data = Cursor::new(b"Hello world\nSecond line\n".to_vec());
        let mut out = Vec::new();
        replay_log(&mut data, ReplayMode::None, &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn test_replay_log_all() {
        let content = b"line 1\nline 2\nline 3\n";
        let mut data = Cursor::new(content.to_vec());
        let mut out = Vec::new();
        replay_log(&mut data, ReplayMode::All, &mut out).unwrap();
        assert_eq!(out, content);
    }

    #[test]
    fn test_replay_log_tail_bytes_small() {
        let content = b"small log content";
        let mut data = Cursor::new(content.to_vec());
        let mut out = Vec::new();
        replay_log(&mut data, ReplayMode::TailBytes(1024), &mut out).unwrap();
        assert_eq!(out, content);
    }

    #[test]
    fn test_replay_log_tail_bytes_large() {
        let content = b"header line\nline 1\nline 2\nline 3\n";
        let mut data = Cursor::new(content.to_vec());
        let mut out = Vec::new();
        // Limit to 18 bytes (lands somewhere inside 'line 1'), should discard partial line and output from 'line 2\n...'
        replay_log(&mut data, ReplayMode::TailBytes(18), &mut out).unwrap();
        assert_eq!(out, b"line 2\nline 3\n");
    }

    #[test]
    fn test_replay_log_tail_lines() {
        let content = b"baris 1\nbaris 2\nbaris 3\nbaris 4\nbaris 5\n";
        let mut data = Cursor::new(content.to_vec());
        let mut out = Vec::new();
        replay_log(&mut data, ReplayMode::TailLines(2), &mut out).unwrap();
        assert_eq!(out, b"baris 4\nbaris 5\n");

        let mut out2 = Vec::new();
        replay_log(&mut data, ReplayMode::TailLines(10), &mut out2).unwrap();
        assert_eq!(out2, content);
    }
}
