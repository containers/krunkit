// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener},
    os::fd::{FromRawFd, RawFd},
};

#[link(name = "krun-efi")]
extern "C" {
    fn krun_get_shutdown_eventfd(ctx_id: u32) -> i32;
}

const HTTP_RUNNING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateRunning\"}\0";

const HTTP_STOPPING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateStopping\"}\0";

/// Retrieve the shutdown event file descriptor initialized by libkrun.
pub unsafe fn get_shutdown_eventfd(ctx_id: u32) -> i32 {
    let fd = krun_get_shutdown_eventfd(ctx_id);
    if fd < 0 {
        panic!("unable to retrieve krun shutdown file descriptor");
    }
    fd
}

/// Listen for status and shutdown requests from the client. Shut down the krun VM when prompted.
pub fn status_listener(shutdown_eventfd: RawFd) -> Result<(), anyhow::Error> {
    // VM is shut down by writing to the shutdown event file.
    let mut shutdown = unsafe { File::from_raw_fd(shutdown_eventfd) };

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 8081)).unwrap();

    for stream in listener.incoming() {
        let mut buf = [0u8; 4096];
        let mut stream = stream.unwrap();

        match stream.read(&mut buf) {
            Ok(_) => {
                let request = String::from_utf8_lossy(&buf);
                if request.contains("POST") {
                    if let Err(e) = stream.write_all(HTTP_RUNNING.as_bytes()) {
                        println!("Error writting POST response: {e}");
                    }
                    if let Err(e) = shutdown.write_all(&1u64.to_le_bytes()) {
                        println!("Error writting to shutdown fd: {e}");
                    }
                } else if let Err(e) = stream.write_all(HTTP_STOPPING.as_bytes()) {
                    println!("Error writting GET response: {e}");
                }
            }
            Err(e) => println!("Error reading stream: {}", e),
        }
    }

    Ok(())
}
