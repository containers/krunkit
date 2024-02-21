// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener},
    os::fd::{FromRawFd, RawFd},
    str::FromStr,
};

use anyhow::{anyhow, Context};
use clap::Parser;

#[link(name = "krun-efi")]
extern "C" {
    fn krun_get_shutdown_eventfd(ctx_id: u32) -> i32;
}

const HTTP_RUNNING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateRunning\"}\0";

const HTTP_STOPPING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateStopping\"}\0";

/// Socket address in which the restful URI socket should listen on. Identical to Rust's
/// SocketAddrV4, but requires a modified FromStr implementation due to how the address is
/// presented on the command line.
#[derive(Clone, Debug, Parser)]
pub struct RestfulUriAddr {
    pub ip_addr: Ipv4Addr,
    pub port: u16,
}

impl FromStr for RestfulUriAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut string = String::from(s);

        if let Some(removed) = string.strip_prefix("tcp://") {
            string = String::from(removed);
        }

        let mut parts: Vec<String> = string.split(':').map(|s| s.to_string()).collect();
        if parts.len() != 2 {
            return Err(anyhow!("restful URI formatted incorrectly"));
        }

        // Ipv4Address's FromStr does not understand that the "localhost" IP address translates to
        // 127.0.0.1, this must be manually translated.
        if &parts[0][..] == "localhost" {
            parts[0] = String::from("127.0.0.1");
        }

        let ip_addr = Ipv4Addr::from_str(&parts[0])
            .context("restful URI IP address formatted incorrectly")?;
        let port =
            u16::from_str(&parts[1]).context("restful URI port number formatted incorrectly")?;

        Ok(Self { ip_addr, port })
    }
}

impl Default for RestfulUriAddr {
    fn default() -> Self {
        Self {
            ip_addr: Ipv4Addr::new(127, 0, 0, 1),
            port: 8081,
        }
    }
}

/// Retrieve the shutdown event file descriptor initialized by libkrun.
pub unsafe fn get_shutdown_eventfd(ctx_id: u32) -> i32 {
    let fd = krun_get_shutdown_eventfd(ctx_id);
    if fd < 0 {
        panic!("unable to retrieve krun shutdown file descriptor");
    }
    fd
}

/// Listen for status and shutdown requests from the client. Shut down the krun VM when prompted.
pub fn status_listener(
    shutdown_eventfd: RawFd,
    addr: Option<RestfulUriAddr>,
) -> Result<(), anyhow::Error> {
    // VM is shut down by writing to the shutdown event file.
    let mut shutdown = unsafe { File::from_raw_fd(shutdown_eventfd) };

    let addr = addr.unwrap_or_default();

    let listener = TcpListener::bind((addr.ip_addr, addr.port)).unwrap();

    for stream in listener.incoming() {
        let mut buf = [0u8; 4096];
        let mut stream = stream.unwrap();

        match stream.read(&mut buf) {
            Ok(_sz) => {
                let request = String::from_utf8_lossy(&buf);
                if request.contains("POST") {
                    // Send a VirtualMachineStateStopping message to the client.
                    if let Err(e) = stream.write_all(HTTP_STOPPING.as_bytes()) {
                        println!("Error writting POST response: {e}");
                    }

                    // Shut down the VM.
                    if let Err(e) = shutdown.write_all(&1u64.to_le_bytes()) {
                        println!("Error writting to shutdown fd: {e}");
                    }
                } else if let Err(e) = stream.write_all(HTTP_RUNNING.as_bytes()) {
                    println!("Error writting GET response: {e}");
                }
            }
            Err(e) => println!("Error reading stream: {}", e),
        }
    }

    Ok(())
}
