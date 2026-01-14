// SPDX-License-Identifier: Apache-2.0

use crate::sleep_notifier::start_power_monitor;
use crate::sleep_notifier::Activity;
use crate::virtio::VsockConfig;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

fn sync_time(socket_url: PathBuf) {
    let mut stream = match UnixStream::connect(&socket_url) {
        Ok(stream) => stream,
        Err(e) => {
            log::error!(
                "Failed to connect to timesync socket {:?}: {}",
                socket_url,
                e
            );
            return;
        }
    };

    let time_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cmd =
        format!("{{\"execute\": \"guest-set-time\", \"arguments\":{{\"time\": {time_ns}}}}}\n");

    if let Err(e) = stream.write_all(cmd.as_bytes()) {
        log::error!("Failed to write to timesync socket: {}", e);
        return;
    }

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    match reader.read_line(&mut response) {
        Ok(_) => {
            log::debug!("Time synced to {time_ns}");
        }
        Err(e) => {
            log::error!("Failed to read qemu-guest-agent response: {e}");
        }
    }
}

pub fn timesync_listener(vsock_config: VsockConfig) {
    let rx = start_power_monitor();
    for activity in rx {
        match activity {
            Activity::Sleep => {
                log::info!("System is going to sleep");
            }
            Activity::Wake => {
                log::info!("System is waking up. Syncing time...");
                sync_time(vsock_config.socket_url.clone());
            }
        }
    }
}
