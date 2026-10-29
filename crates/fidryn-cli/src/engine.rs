//! Process-cached identity of the executable used for an evaluation export.

use serde_json::{Value, json};
use std::fs::File;
use std::io::Read;
use std::sync::OnceLock;

static EXECUTABLE_IDENTITY: OnceLock<Result<Value, String>> = OnceLock::new();

/// Fingerprint the current executable once without exporting its local path.
/// This identifies available bytes; it does not distribute an executable.
pub(crate) fn executable_identity() -> Result<Value, String> {
    EXECUTABLE_IDENTITY.get_or_init(capture).clone()
}

fn capture() -> Result<Value, String> {
    // The running inode survives replacement of its launch path on Linux.
    #[cfg(target_os = "linux")]
    let mut file = File::open("/proc/self/exe")
        .map_err(|error| format!("cannot read running executable: {error}"))?;
    #[cfg(not(target_os = "linux"))]
    let mut file = File::open(
        std::env::current_exe()
            .map_err(|error| format!("cannot locate current executable: {error}"))?,
    )
    .map_err(|error| format!("cannot read current executable: {error}"))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut bytes = 0u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot fingerprint current executable: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes = bytes
            .checked_add(count as u64)
            .ok_or("executable byte count overflow")?;
    }
    Ok(json!({
        "algorithm":"blake3", "digest":hasher.finalize().to_hex().to_string(), "bytes":bytes,
        "packageVersion":env!("CARGO_PKG_VERSION"), "semanticsVersion":"0.2",
        "arch":std::env::consts::ARCH, "os":std::env::consts::OS,
        "captureMethod": if cfg!(target_os = "linux") { "running_process_inode" } else { "current_executable_file" },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_identity_is_cached_nonempty_and_has_no_local_path() {
        let first = executable_identity().unwrap();
        assert_eq!(first, executable_identity().unwrap());
        assert!(first["bytes"].as_u64().unwrap() > 0);
        assert_eq!(first["digest"].as_str().unwrap().len(), 64);
        assert_eq!(first["algorithm"], "blake3");
        assert_eq!(first["semanticsVersion"], "0.2");
        assert_eq!(first.as_object().unwrap().len(), 8);
        assert!(
            !first
                .as_object()
                .unwrap()
                .keys()
                .any(|key| key.to_ascii_lowercase().contains("path"))
        );
    }
}
