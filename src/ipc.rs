use std::{env, path::PathBuf};
use tokio::sync::mpsc;
use tracing::error;

// ── Unix socket path (user-specific) ─────────────────────────────────────────
//
// macOS : $TMPDIR  is process-owner-specific (/var/folders/…/T/).
// Linux : $XDG_RUNTIME_DIR is per-login-user (/run/user/<uid>).
//         Headless fallback: embed UID from /proc/self to avoid multi-user
//         collisions on the shared /tmp.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(unix)]
pub fn sock_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    return PathBuf::from(env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()))
        .join("voxelruler_ipc.sock");

    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(dir) = env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir).join("voxelruler_ipc.sock");
        }
        // Headless / container fallback: embed UID to avoid multi-user collisions
        #[cfg(target_os = "linux")]
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            use std::os::unix::fs::MetadataExt;
            return PathBuf::from(format!("/tmp/voxelruler_ipc_{}.sock", meta.uid()));
        }
        PathBuf::from("/tmp/voxelruler_ipc.sock")
    }
}

/// Remove the IPC socket on clean exit (Unix only). Safe to call multiple times.
#[cfg(unix)]
pub fn cleanup() {
    let _ = std::fs::remove_file(sock_path());
}

#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\voxelruler_ipc";

/// First `voxelruler://` URL found in argv, or `None`.
fn first_deeplink_arg() -> Option<String> {
    env::args().skip(1).find(|a| a.starts_with("voxelruler://"))
}

/// Decode raw IPC bytes and forward every `voxelruler://` line to `tx`.
fn dispatch_ipc_bytes(data: &[u8], tx: &mpsc::UnboundedSender<String>) {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    s.lines()
        .filter(|l| l.starts_with("voxelruler://"))
        .for_each(|l| {
            let _ = tx.send(l.to_string());
        });
}

/// Set up single-instance IPC.
///
/// Returns `true`  — this process is the **primary** instance.
/// Returns `false` — a primary instance is already running; caller should exit.
///
/// Platform notes
/// ──────────────
/// macOS   — Apple Events (`url_handler`) already route deeplinks to the running
///           instance without launching a second process.  The Unix-socket path
///           here is kept as a safety net for CLI / test invocations.
/// Linux   — cargo-packager passes the URL as argv[1] on second launch.
///           Socket path is user-specific (XDG_RUNTIME_DIR preferred).
/// Windows — Named pipe; the pipe lives only while the server runs.
pub async fn setup_ipc(tx: mpsc::UnboundedSender<String>) -> bool {
    #[cfg(unix)]
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{UnixListener, UnixStream};

        let sock = sock_path();

        // Primary instance already running — forward deeplink and exit
        if let Ok(mut stream) = UnixStream::connect(&sock).await {
            if let Some(url) = first_deeplink_arg() {
                let _ = stream.write_all(url.as_bytes()).await;
            }
            return false;
        }

        // Become the primary instance
        let _ = std::fs::remove_file(&sock); // remove stale socket from a crash
        let Ok(listener) = UnixListener::bind(&sock) else {
            return true;
        };

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    continue;
                };
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = socket.read(&mut buf).await else {
                        return;
                    };
                    dispatch_ipc_bytes(&buf[..n], &tx_clone);
                });
            }
        });
        true
    }

    #[cfg(windows)]
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

        // Primary instance already running — forward deeplink and exit
        if let Ok(mut client) = ClientOptions::new().open(PIPE_NAME) {
            if let Some(url) = first_deeplink_arg() {
                let _ = client.write_all(url.as_bytes()).await;
            }
            return false;
        }

        // Become the primary instance
        let Ok(mut server) = ServerOptions::new()
            .first_pipe_instance(true)
            .create(PIPE_NAME)
        else {
            error!("Failed to create IPC named pipe");
            return true;
        };

        tokio::spawn(async move {
            loop {
                if server.connect().await.is_err() {
                    break;
                }

                // Prepare the next pipe instance before handling the current client
                let Ok(next_server) = ServerOptions::new().create(PIPE_NAME) else {
                    error!("Failed to create next pipe instance");
                    break;
                };
                let mut socket = std::mem::replace(&mut server, next_server);
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = socket.read(&mut buf).await else {
                        return;
                    };
                    dispatch_ipc_bytes(&buf[..n], &tx_clone);
                });
            }
        });
        true
    }
}
