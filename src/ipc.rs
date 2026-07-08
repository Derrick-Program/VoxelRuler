use std::{env, path::PathBuf};
use tokio::sync::mpsc;
use tracing::error;

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
        #[cfg(target_os = "linux")]
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            use std::os::unix::fs::MetadataExt;
            return PathBuf::from(format!("/tmp/voxelruler_ipc_{}.sock", meta.uid()));
        }
        PathBuf::from("/tmp/voxelruler_ipc.sock")
    }
}

#[cfg(unix)]
pub fn cleanup() {
    let _ = std::fs::remove_file(sock_path());
}

#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\voxelruler_ipc";

fn first_deeplink_arg() -> Option<String> {
    env::args().skip(1).find(|a| a.starts_with("voxelruler://"))
}

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

pub async fn setup_ipc(tx: mpsc::UnboundedSender<String>) -> bool {
    #[cfg(unix)]
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{UnixListener, UnixStream};

        let sock = sock_path();

        if let Ok(mut stream) = UnixStream::connect(&sock).await {
            if let Some(url) = first_deeplink_arg() {
                let _ = stream.write_all(url.as_bytes()).await;
            }
            return false;
        }

        let _ = std::fs::remove_file(&sock); // stale socket from a crashed prior instance would make bind() fail
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

        if let Ok(mut client) = ClientOptions::new().open(PIPE_NAME) {
            if let Some(url) = first_deeplink_arg() {
                let _ = client.write_all(url.as_bytes()).await;
            }
            return false;
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_ipc_bytes() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let data = b"voxelruler://auth?code=123\nsome other data\nvoxelruler://test";
        dispatch_ipc_bytes(data, &tx);

        assert_eq!(rx.blocking_recv().unwrap(), "voxelruler://auth?code=123");
        assert_eq!(rx.blocking_recv().unwrap(), "voxelruler://test");
    }

    #[cfg(unix)]
    #[test]
    fn test_sock_path_is_absolute() {
        let path = sock_path();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("voxelruler_ipc"));
    }
}
