// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener},
    os::{
        fd::{FromRawFd, RawFd},
        unix::net::UnixListener,
    },
    str::FromStr,
};

use anyhow::{anyhow, Context};

#[link(name = "krun-efi")]
extern "C" {
    fn krun_get_shutdown_eventfd(ctx_id: u32) -> i32;
}

const HTTP_RUNNING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateRunning\"}\0";

const HTTP_STOPPING: &str =
    "HTTP/1.1 200 OK\r\nContent-type: application/json\r\n\r\n{\"state\": \"VirtualMachineStateStopping\"}\0";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UriScheme {
    Tcp,
    Unix,
    #[default]
    None,
}

impl FromStr for UriScheme {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tcp" => Ok(Self::Tcp),
            "unix" => Ok(Self::Unix),
            "none" => Ok(Self::None),
            _ => Err(anyhow!("invalid scheme")),
        }
    }
}

/// Socket address in which the restful URI socket should listen on. Identical to Rust's
/// SocketAddrV4, but requires a modified FromStr implementation due to how the address is
/// presented on the command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RestfulUri {
    Tcp(Ipv4Addr, u16),
    Unix(String),
    #[default]
    None,
}

impl FromStr for RestfulUri {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expression = regex::Regex::new(r"^(?P<scheme>none|tcp|unix)://(?P<value>.*)").unwrap();
        let Some(cap) = expression.captures(s) else {
            return Err(anyhow!("invalid scheme input"));
        };
        let scheme = &cap["scheme"];
        let value = &cap["value"];
        match UriScheme::from_str(scheme)? {
            UriScheme::Tcp => {
                let (ip_addr, port) = parse_tcp_input(value)?;
                Ok(Self::Tcp(ip_addr, port))
            }
            UriScheme::Unix => {
                if value.is_empty() {
                    return Err(anyhow!("empty unix socket path"));
                }
                Ok(Self::Unix(value.to_string()))
            }
            UriScheme::None => Ok(Self::None),
        }
    }
}

fn parse_tcp_input(input: &str) -> Result<(Ipv4Addr, u16), anyhow::Error> {
    let mut parts: Vec<String> = input.split(':').map(|s| s.to_string()).collect();
    if parts.len() != 2 {
        return Err(anyhow!("restful URI formatted incorrectly"));
    }

    // Ipv4Address's FromStr does not understand that the "localhost" IP address translates to
    // 127.0.0.1, this must be manually translated.
    if &parts[0][..] == "localhost" {
        parts[0] = String::from("127.0.0.1");
    }

    let ip_addr =
        Ipv4Addr::from_str(&parts[0]).context("restful URI IP address formatted incorrectly")?;
    let port = u16::from_str(&parts[1]).context("restful URI port number formatted incorrectly")?;
    Ok((ip_addr, port))
}

/// Retrieve the shutdown event file descriptor initialized by libkrun.
pub unsafe fn get_shutdown_eventfd(ctx_id: u32) -> i32 {
    let fd = krun_get_shutdown_eventfd(ctx_id);
    if fd < 0 {
        panic!("unable to retrieve krun shutdown file descriptor");
    }
    fd
}

fn handle_incoming_stream<T: Read + Write>(stream: &mut T, shutdown_fd: &mut File) {
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(_sz) => {
            let request = String::from_utf8_lossy(&buf);
            if request.contains("POST") {
                // Send a VirtualMachineStateStopping message to the client.
                if let Err(e) = stream.write_all(HTTP_STOPPING.as_bytes()) {
                    log::error!("Failure writing POST response: {e}");
                }

                // Shut down the VM.
                if let Err(e) = shutdown_fd.write_all(&1u64.to_le_bytes()) {
                    log::error!("Failure writing to shutdown fd: {e}");
                }
            } else if let Err(e) = stream.write_all(HTTP_RUNNING.as_bytes()) {
                log::error!("Failure writing GET response: {e}");
            }
        }
        Err(e) => log::error!("Failure reading stream: {e}"),
    }
}

/// Listen for status and shutdown requests from the client. Shut down the krun VM when prompted.
pub fn status_listener(
    shutdown_eventfd: RawFd,
    addr: Option<RestfulUri>,
) -> Result<(), anyhow::Error> {
    // VM is shut down by writing to the shutdown event file.
    let mut shutdown = unsafe { File::from_raw_fd(shutdown_eventfd) };

    let addr = addr.unwrap_or_default();

    match addr {
        RestfulUri::Tcp(addr, port) => {
            let listener = TcpListener::bind((addr, port))
                .map_err(|e| anyhow!("Unable to bind to TCP listener: {}", e))?;

            for stream in listener.incoming() {
                handle_incoming_stream(&mut stream.unwrap(), &mut shutdown)
            }
        }
        RestfulUri::Unix(path) => {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != ErrorKind::NotFound {
                    return Err(anyhow!("failed to remove socket with error {e}"));
                }
            }
            let listener = UnixListener::bind(path)
                .map_err(|e| anyhow!("Unable to bind to unix socket: {}", e))?;

            for stream in listener.incoming() {
                handle_incoming_stream(&mut stream.unwrap(), &mut shutdown)
            }
        }
        RestfulUri::None => unreachable!(),
    }

    Ok(())
}

#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_unix_scheme() {
        assert_eq!(
            RestfulUri::Unix("/tmp/path".to_string()),
            RestfulUri::from_str("unix:///tmp/path").unwrap()
        );
    }

    #[test]
    fn parse_unix_scheme_missing_path() {
        assert_eq!(
            anyhow!("empty unix socket path").to_string(),
            RestfulUri::from_str("unix://").err().unwrap().to_string()
        );
    }

    #[test]
    fn parse_unix_scheme_missing_slashes() {
        assert_eq!(
            anyhow!("invalid scheme input").to_string(),
            RestfulUri::from_str("unix:").err().unwrap().to_string()
        );
    }

    #[test]
    fn parse_unix_scheme_misspelling() {
        assert_eq!(
            anyhow!("invalid scheme input").to_string(),
            RestfulUri::from_str("uni://path")
                .err()
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn parse_valid_tcp_scheme() {
        assert_eq!(
            RestfulUri::Tcp(Ipv4Addr::new(127, 0, 0, 1), 8080),
            RestfulUri::from_str("tcp://localhost:8080").unwrap(),
        );
    }

    #[test]
    fn parse_tcp_scheme_missing_port() {
        assert_eq!(
            anyhow!("restful URI formatted incorrectly").to_string(),
            RestfulUri::from_str("tcp://localhost")
                .err()
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn parse_tcp_scheme_with_unix_path() {
        assert_eq!(
            anyhow!("restful URI formatted incorrectly").to_string(),
            RestfulUri::from_str("tcp:///tmp/path")
                .err()
                .unwrap()
                .to_string(),
        );
    }

    #[test]
    fn parse_valid_none_scheme() {
        assert_eq!(RestfulUri::None, RestfulUri::from_str("none://").unwrap());
    }

    #[test]
    fn parse_none_scheme_missing_postfix() {
        assert_eq!(
            anyhow!("invalid scheme input").to_string(),
            RestfulUri::from_str("none").err().unwrap().to_string(),
        );
    }

    #[test]
    fn parse_random_string_scheme() {
        assert_eq!(
            anyhow!("invalid scheme input").to_string(),
            RestfulUri::from_str("foobar").err().unwrap().to_string(),
        );
    }
}
