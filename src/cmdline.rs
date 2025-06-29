// SPDX-License-Identifier: Apache-2.0

use crate::{status::RestfulUri, virtio::VirtioDeviceConfig};

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use anyhow::{anyhow, Context, Result};
use clap::Parser;

/// Command line arguments to configure a krun VM.
#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Number of vCPUs for the VM.
    #[arg(long, short)]
    pub cpus: u8,

    /// Amount of RAM available to VM.
    #[arg(long, short)]
    pub memory: u32,

    /// Bootloader configuration.
    #[arg(long)]
    pub bootloader: Option<bootloader::Config>,

    /// virtio devices to configure in the VM.
    #[arg(long = "device")]
    pub devices: Vec<VirtioDeviceConfig>,

    /// URI of the status/shutdown listener.
    #[arg(long = "restful-uri")]
    pub restful_uri: Option<RestfulUri>,

    /// GUI option for compatibility with vfkit (ignored).
    #[arg(long, default_value_t = false)]
    pub gui: bool,

    /// SMBIOS OEM String
    #[arg(long = "oem-string")]
    pub oem_strings: Option<Vec<String>>,

    /// Log level for libkrun (0=off, 1=error, 2=warn, 3=info, 4=debug, 5 or higher=trace)
    #[arg(long = "krun-log-level", default_value_t = 3)]
    pub krun_log_level: u32,

    /// Enable Nested Virtualization.
    #[arg(long, short)]
    pub nested: bool,
}

/// Parse the input string into a hash map of key value pairs, associating the argument with its
/// respective value.
pub fn parse_args(s: String) -> Result<HashMap<String, String>, anyhow::Error> {
    let mut map: HashMap<String, String> = HashMap::new();
    let list: Vec<String> = s.split(',').map(|s| s.to_string()).collect();

    for arg in list {
        let arg_parts: Vec<&str> = arg.split('=').collect();
        let key = arg_parts[0].to_string();
        let val = match arg_parts.len() {
            1 => String::new(),
            2 => arg_parts[1].to_string(),
            _ => return Err(anyhow!(format!("invalid argument format: {arg}"))),
        };
        let res = map.insert(key, val);
        if res.is_some() {
            return Err(anyhow!(format!("argument {arg} is only expected once")));
        }
    }

    Ok(map)
}

/// Check the arguments hash map if all required arguments are present
pub fn check_required_args(
    args: &HashMap<String, String>,
    label: &str,
    required: &[&str],
) -> Result<(), anyhow::Error> {
    for &r in required {
        if !args.contains_key(r) {
            return Err(anyhow!(format!("{label} is missing argument: {r}")));
        }
    }

    Ok(())
}

/// Check the arguments hash map if any unknown arguments exist
pub fn check_unknown_args(args: HashMap<String, String>, label: &str) -> Result<(), anyhow::Error> {
    if !args.is_empty() {
        let unknown_args: Vec<String> = args
            .into_iter()
            .map(|arg| format!("{}={}", arg.0, arg.1))
            .collect();
        return Err(anyhow!(format!(
            "unknown {} arguments: {:?}",
            label, unknown_args
        )));
    }

    Ok(())
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
            let mut args = parse_args(s.to_string())?;
            check_required_args(&args, "bootloader", &["efi", "variable-store", "create"])?;

            let fw = args.remove("efi").unwrap();
            if !fw.is_empty() {
                return Err(anyhow!(format!("unknown bootloader argument: efi={fw}")));
            }

            let v = args.remove("variable-store").unwrap();

            let action = args.remove("create").unwrap();
            if !action.is_empty() {
                return Err(anyhow!(format!(
                    "unknown bootloader argument: create={action}"
                )));
            }

            check_unknown_args(args, "bootloader")?;

            let fw = BootloaderFw::from_str("efi")?;
            let v = Vstore::from_str(v.as_str())?;
            let action = Action::from_str("create")?;

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
            Ok(Self(
                PathBuf::from_str(s).context("variable-store argument not a valid path")?,
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
    #[test]
    fn virtio_blk_argument_ordering() {
        let in_order =
            super::parse_args(String::from("path=/Users/user/disk-image.raw,format=raw")).unwrap();
        let out_of_order =
            super::parse_args(String::from("format=raw,path=/Users/user/disk-image.raw")).unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert("path".to_string(), "/Users/user/disk-image.raw".to_string());
        expected.insert("format".to_string(), "raw".to_string());

        assert_eq!(in_order, out_of_order);
        assert_eq!(in_order, expected);
    }

    #[test]
    fn virtio_net_argument_ordering() {
        let in_order = super::parse_args(String::from(
            "unixSocketPath=/Users/user/vm-network.sock,mac=ff:ff:ff:ff:ff:ff",
        ))
        .unwrap();
        let out_of_order = super::parse_args(String::from(
            "mac=ff:ff:ff:ff:ff:ff,unixSocketPath=/Users/user/vm-network.sock",
        ))
        .unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert(
            "unixSocketPath".to_string(),
            "/Users/user/vm-network.sock".to_string(),
        );
        expected.insert("mac".to_string(), "ff:ff:ff:ff:ff:ff".to_string());

        assert_eq!(in_order, out_of_order);
        assert_eq!(in_order, expected);
    }

    #[test]
    fn virtio_vsock_argument_ordering() {
        let in_order = super::parse_args(String::from(
            "port=1025,socketURL=/Users/user/vsock2.sock,listen",
        ))
        .unwrap();
        let out_of_order = super::parse_args(String::from(
            "port=1025,listen,socketURL=/Users/user/vsock2.sock",
        ))
        .unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert("port".to_string(), "1025".to_string());
        expected.insert(
            "socketURL".to_string(),
            "/Users/user/vsock2.sock".to_string(),
        );
        expected.insert("listen".to_string(), String::new());

        assert_eq!(in_order, out_of_order);
        assert_eq!(in_order, expected);
    }

    #[test]
    fn virtio_fs_argument_ordering() {
        let in_order = super::parse_args(String::from(
            "sharedDir=/Users/user/shared-dir,mountTag=MOUNT_TAG",
        ))
        .unwrap();
        let out_of_order = super::parse_args(String::from(
            "mountTag=MOUNT_TAG,sharedDir=/Users/user/shared-dir",
        ))
        .unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert(
            "sharedDir".to_string(),
            "/Users/user/shared-dir".to_string(),
        );
        expected.insert("mountTag".to_string(), "MOUNT_TAG".to_string());

        assert_eq!(in_order, out_of_order);
        assert_eq!(in_order, expected);
    }

    #[test]
    fn virtio_gpu_argument_ordering() {
        let in_order = super::parse_args(String::from("height=50,width=25")).unwrap();
        let out_of_order = super::parse_args(String::from("width=25,height=50")).unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert("height".to_string(), "50".to_string());
        expected.insert("width".to_string(), "25".to_string());

        assert_eq!(in_order, out_of_order);
        assert_eq!(in_order, expected);
    }

    #[test]
    fn argument_parsing() {
        let s = String::from("port=1025,socketURL=/Users/user/vsock2.sock,listen");
        let args = super::parse_args(s).unwrap();

        let mut expected = std::collections::HashMap::new();
        expected.insert("port".to_string(), "1025".to_string());
        expected.insert(
            "socketURL".to_string(),
            "/Users/user/vsock2.sock".to_string(),
        );
        expected.insert("listen".to_string(), String::new());
        assert_eq!(expected, args);
    }

    #[test]
    fn required_args() {
        let required = &["port", "socketURL"];
        let s = String::from("port=1025,socketURL=/Users/user/vsock2.sock,listen");
        let args = super::parse_args(s).unwrap();

        assert_eq!(
            super::check_required_args(&args, "", required).is_ok(),
            true
        );

        let required = &["port", "wrong"];
        assert_ne!(
            super::check_required_args(&args, "", required).is_ok(),
            true
        );
    }

    #[test]
    fn unknown_args() {
        use std::collections::HashMap;

        let args: HashMap<String, String> = HashMap::new();
        assert_eq!(super::check_unknown_args(args, "").is_ok(), true);

        let mut args: HashMap<String, String> = HashMap::new();
        args.insert("foo".to_string(), "bar".to_string());
        assert_ne!(super::check_unknown_args(args, "").is_ok(), true);
    }

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
            "virtio-blk,path=/Users/user/root.qcow2,format=qcow2",
            "--device",
            "virtio-rng",
            "--device",
            "virtio-serial,logFilePath=/Users/user/serial.log",
            "--device",
            "virtio-blk,path=/Users/user/data.raw,format=raw",
            "--device",
            "virtio-vsock,port=1024,socketURL=/Users/user/vsock1.sock,listen",
            "--device",
            "virtio-net,unixSocketPath=/Users/user/net.sock,mac=00:00:00:00:00:00",
            "--device",
            "virtio-fs,sharedDir=/Users/user/fs,mountTag=guest-dir",
            "--device",
            "virtio-vsock,port=1025,socketURL=/Users/user/vsock2.sock,listen",
            "--device",
            "virtio-gpu,width=800,height=600",
            "--device",
            "virtio-input,keyboard",
            "--restful-uri",
            "tcp://localhost:49573",
            "--gui",
            "--krun-log-level",
            "5",
        ];

        let mut args = Args::try_parse_from(cmdline).unwrap();

        let input = args
            .devices
            .pop()
            .expect("expected 10th virtio device config");
        if let VirtioDeviceConfig::Input(input) = input {
            assert_eq!(input, InputConfig::Keyboard);
        } else {
            panic!("expected virtio-input device as 10th device config argument");
        }

        let gpu = args
            .devices
            .pop()
            .expect("expected 9th virtio device config");
        if let VirtioDeviceConfig::Gpu(gpu) = gpu {
            assert_eq!(gpu.width, 800);
            assert_eq!(gpu.height, 600);
        } else {
            panic!("expected virtio-gpu device as 9th device config argument");
        }

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
            assert_eq!(blk.format, DiskImageFormat::Raw);
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
            assert_eq!(
                blk.path,
                PathBuf::from_str("/Users/user/root.qcow2").unwrap()
            );
            assert_eq!(blk.format, DiskImageFormat::Qcow2);
        } else {
            panic!("expected virtio-blk device as 1st device config argument");
        }

        let restful_uri = args.restful_uri.expect("restful-uri argument not found");

        assert_eq!(
            restful_uri,
            RestfulUri::Tcp(Ipv4Addr::new(127, 0, 0, 1), 49573)
        );

        assert_eq!(args.gui, true);
        assert_eq!(args.krun_log_level, 5);
    }
}
