// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{BufReader, Read, Write},
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

pub unsafe fn get_shutdown_eventfd(ctx_id: u32) -> i32 {
    let fd = krun_get_shutdown_eventfd(ctx_id);
    if fd < 0 {
        panic!("unable to retrieve krun shutdown file descriptor");
    }
    fd
}

pub fn status_listener(shutdown_eventfd: RawFd) -> Result<(), anyhow::Error> {
    let mut shutdown = unsafe { File::from_raw_fd(shutdown_eventfd) };

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 8081)).unwrap();

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut request = String::new();

        reader.read_to_string(&mut request).unwrap();
        if request.contains("POST") {
            stream.write_all(HTTP_RUNNING.as_bytes()).unwrap();
        } else {
            stream.write_all(HTTP_STOPPING.as_bytes()).unwrap();
            shutdown.write_all(&request.as_bytes()[..8]).unwrap();

            break;
        }
    }

    Ok(())
}
