// SPDX-License-Identifier: Apache-2.0

use crate::{status::RestfulUriAddr, virtio::VirtioDeviceConfig};

use std::{path::PathBuf, str::FromStr};

use anyhow::{anyhow, Context, Result};
use clap::Parser;

/// Command line arguments to configure a krun VM.
#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Number of vCPUs for the VM.
    #[arg(long)]
    pub cpus: u8,

    /// Amount of RAM available to VM.
    #[arg(long)]
    pub memory: u32,

    /// Bootloader configuration.
    #[arg(long)]
    pub bootloader: Option<bootloader::Config>,

    /// virtio devices to configure in the VM.
    #[arg(long = "device")]
    pub devices: Vec<VirtioDeviceConfig>,

    /// URI of the status/shutdown listener.
    #[arg(long = "restful-uri")]
    pub restful_uri: RestfulUriAddr,
}

/// Parse a string into a vector of substrings, all of which are separated by commas.
pub fn args_parse(s: String, label: &str, sz: Option<usize>) -> Result<Vec<String>> {
    let list: Vec<String> = s.split(',').map(|s| s.to_string()).collect();

    // If an expected size is given, ensure that the parsed vector is of the expected size.
    if let Some(size) = sz {
        if list.len() != size {
            return Err(anyhow!(
                "expected --{} argument to have {} comma-separated sub-arguments, found {}",
                label,
                size,
                list.len()
            ));
        }
    }

    Ok(list)
}

/// Parse the value of some expected label, in which the two are separated by an '=' character.
///
/// For example, if a string is hello=world, "hello" is the label and "world" is the value.
pub fn val_parse(s: String, label: &str) -> Result<String> {
    let vals: Vec<&str> = s.split('=').collect();

    match vals.len() {
        1 => Ok(vals[0].to_string()),
        2 => {
            // Ensure that the label is as expected.
            let label_found = vals[0];
            if label_found != label {
                return Err(anyhow!(format!(
                    "expected label {}, found {}",
                    label, label_found
                )));
            }

            Ok(vals[1].to_string())
        }
        _ => Err(anyhow!(format!("invalid argument format: {}", s.clone()))),
    }
}

/// A wrapper of all data associated with the bootloader argument.
mod bootloader {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct Config {
        fw: BootloaderFw,
        vstore: PathBuf,
        action: Action,
    }

    impl FromStr for Config {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let args = args_parse(s.to_string(), "bootloader", Some(3))?;

            let fw = BootloaderFw::from_str(&args[0])?;
            let v = Vstore::from_str(&args[1])?;
            let action = Action::from_str(&args[2])?;

            Ok(Self {
                fw,
                vstore: v.0,
                action,
            })
        }
    }

    /// Bootloader firmware identifier.
    #[derive(Clone, Debug)]
    pub enum BootloaderFw {
        Efi,
    }

    impl FromStr for BootloaderFw {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let string = s.to_string().to_lowercase();

            match string.as_str() {
                "efi" => Ok(Self::Efi),
                _ => Err(anyhow!("invalid bootloader firmware option: {}", string)),
            }
        }
    }

    /// Variable store.
    #[derive(Clone, Debug)]
    pub struct Vstore(PathBuf);

    impl FromStr for Vstore {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let value = val_parse(s.to_string(), "variable-store")?;

            Ok(Self(
                PathBuf::from_str(&value).context("variable-store argument not a valid path")?,
            ))
        }
    }

    /// Bootloader action.
    #[derive(Clone, Debug)]
    pub enum Action {
        Create,
    }

    impl FromStr for Action {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let string = s.to_string().to_lowercase();

            match string.as_str() {
                "create" => Ok(Self::Create),
                _ => Err(anyhow!("invalid bootloader action: {}", string)),
            }
        }
    }
}

mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn mac_cmdline_ordering_argtest() {
        use super::*;
        use crate::virtio::*;

        use std::net::Ipv4Addr;

        use mac_address::MacAddress;

        let cmdline = vec![
            "krunkit",
            "--cpus",
            "4",
            "--memory",
            "2048",
            "--bootloader",
            "efi,variable-store=/Users/user/bootloader,create",
            "--device",
            "virtio-blk,path=/Users/user/root.raw",
            "--device",
            "virtio-rng",
            "--device",
            "virtio-serial,logFilePath=/Users/user/serial.log",
            "--device",
            "virtio-blk,path=/Users/user/data.raw",
            "--device",
            "virtio-vsock,port=1024,socketURL=/Users/user/vsock1.sock,listen",
            "--device",
            "virtio-net,unixSocketPath=/Users/user/net.sock,mac=00:00:00:00:00:00",
            "--device",
            "virtio-fs,sharedDir=/Users/user/fs,mountTag=guest-dir",
            "--device",
            "virtio-vsock,port=1025,socketURL=/Users/user/vsock2.sock,listen",
            "--restful-uri",
            "tcp://localhost:49573",
        ];

        let mut args = Args::try_parse_from(cmdline).unwrap();

        let vsock = args
            .devices
            .pop()
            .expect("expected 8th virtio device config");
        if let VirtioDeviceConfig::Vsock(v) = vsock {
            assert_eq!(v.port, 1025);
            assert_eq!(
                v.socket_url,
                PathBuf::from_str("/Users/user/vsock2.sock").unwrap()
            );
            assert_eq!(v.action, VsockAction::Listen);
        } else {
            panic!("expected virtio-vsock device as 8th device config argument");
        }

        let fs = args
            .devices
            .pop()
            .expect("expected 7th virtio device config");
        if let VirtioDeviceConfig::Fs(fs) = fs {
            assert_eq!(fs.shared_dir, PathBuf::from_str("/Users/user/fs").unwrap());
            assert_eq!(fs.mount_tag, PathBuf::from_str("guest-dir").unwrap());
        } else {
            panic!("expected virtio-fs device as 7th device config argument");
        }

        let net = args
            .devices
            .pop()
            .expect("expected 6th virtio device config");
        if let VirtioDeviceConfig::Net(net) = net {
            assert_eq!(
                net.unix_socket_path,
                PathBuf::from_str("/Users/user/net.sock").unwrap()
            );
            assert_eq!(net.mac_address, MacAddress::new([0, 0, 0, 0, 0, 0]));
        } else {
            panic!("expected virtio-net device as 6th device config argument");
        }

        let vsock = args
            .devices
            .pop()
            .expect("expected 5th virtio device config");
        if let VirtioDeviceConfig::Vsock(v) = vsock {
            assert_eq!(v.port, 1024);
            assert_eq!(
                v.socket_url,
                PathBuf::from_str("/Users/user/vsock1.sock").unwrap()
            );
            assert_eq!(v.action, VsockAction::Listen);
        } else {
            panic!("expected virtio-vsock device as 5th device config argument");
        }

        let blk = args
            .devices
            .pop()
            .expect("expected 4th virtio device config");
        if let VirtioDeviceConfig::Blk(blk) = blk {
            assert_eq!(blk.path, PathBuf::from_str("/Users/user/data.raw").unwrap());
        } else {
            panic!("expected virtio-blk device as 4th device config argument");
        }

        let serial = args
            .devices
            .pop()
            .expect("expected 3rd virtio device config");
        if let VirtioDeviceConfig::Serial(serial) = serial {
            assert_eq!(
                serial.log_file_path,
                PathBuf::from_str("/Users/user/serial.log").unwrap()
            );
        } else {
            panic!("expected virtio-serial device as 3rd device config argument");
        }

        let rng = args
            .devices
            .pop()
            .expect("expected 2nd virtio device config");

        if VirtioDeviceConfig::Rng != rng {
            panic!("expected virtio-rng device as 2nd device config argument");
        }

        let blk = args
            .devices
            .pop()
            .expect("expected 1st virtio device config");
        if let VirtioDeviceConfig::Blk(blk) = blk {
            assert_eq!(blk.path, PathBuf::from_str("/Users/user/root.raw").unwrap());
        } else {
            panic!("expected virtio-blk device as 1st device config argument");
        }

        assert_eq!(args.restful_uri.ip_addr, Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(args.restful_uri.port, 49573);
    }
}
