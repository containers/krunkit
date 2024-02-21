// SPDX-License-Identifier: Apache-2.0

use crate::cmdline::{args_parse, val_parse};

use std::{
    ffi::{c_char, CString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};
use mac_address::MacAddress;

#[link(name = "krun-efi")]
extern "C" {
    fn krun_set_root_disk(ctx_id: u32, c_disk_path: *const c_char) -> i32;
    fn krun_set_data_disk(ctx_id: u32, c_disk_path: *const c_char) -> i32;
    fn krun_add_vsock_port(ctx_id: u32, port: u32, c_filepath: *const c_char) -> i32;
    fn krun_add_virtiofs(ctx_id: u32, c_tag: *const c_char, c_path: *const c_char) -> i32;
    fn krun_set_gvproxy_path(ctx_id: u32, c_path: *const c_char) -> i32;
    fn krun_set_net_mac(ctx_id: u32, c_mac: *const u8) -> i32;
}

static mut ROOT_BLK_SET: bool = false;
static mut DATA_BLK_SET: bool = false;

/// Each virito device configures itself with krun differently. This is used by each virtio device
/// to set their respective configurations with libkrun.
pub trait KrunContextSet {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error>;
}

/// virtio device configurations.
#[derive(Clone, Debug, PartialEq)]
pub enum VirtioDeviceConfig {
    Blk(BlkConfig),
    Rng,
    Serial(SerialConfig),
    Vsock(VsockConfig),
    Net(NetConfig),
    Fs(FsConfig),
    Gpu(GpuConfig),
    Input(InputConfig),
}

/// Parse a virtio device configuration with its respective information/data.
impl FromStr for VirtioDeviceConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio", None)?;

        if args.is_empty() {
            return Err(anyhow!("no virtio device config found"));
        }

        // The first string is the virtio device identifier. Subsequent arguments are
        // device-specific.
        let rest = args[1..].join(",");

        match &args[0][..] {
            "virtio-blk" => Ok(Self::Blk(BlkConfig::from_str(&rest)?)),
            "virtio-rng" => Ok(Self::Rng),
            "virtio-serial" => Ok(Self::Serial(SerialConfig::from_str(&rest)?)),
            "virtio-vsock" => Ok(Self::Vsock(VsockConfig::from_str(&rest)?)),
            "virtio-net" => Ok(Self::Net(NetConfig::from_str(&rest)?)),
            "virtio-fs" => Ok(Self::Fs(FsConfig::from_str(&rest)?)),
            "virtio-gpu" => Ok(Self::Gpu(GpuConfig::from_str(&rest)?)),
            "virtio-input" => Ok(Self::Input(InputConfig::from_str(&rest)?)),
            _ => Err(anyhow!(format!(
                "invalid virtio device label specified: {}",
                args[0]
            ))),
        }
    }
}

/// Configure the device in the krun context based on which underlying device is contained.
impl KrunContextSet for VirtioDeviceConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        match self {
            Self::Blk(blk) => blk.krun_ctx_set(id),
            Self::Vsock(vsock) => vsock.krun_ctx_set(id),
            Self::Net(net) => net.krun_ctx_set(id),
            Self::Fs(fs) => fs.krun_ctx_set(id),

            // virtio-input, virtio-gpu, virtio-rng and virtio-serial devices are
            // currently not configured in krun.
            _ => Ok(()),
        }
    }
}

/// Configuration of a virtio-blk device.
#[derive(Clone, Debug, PartialEq)]
pub struct BlkConfig {
    /// Path of the file to store as the root disk.
    pub path: PathBuf,
}

impl FromStr for BlkConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-blk", Some(1))?;

        Ok(Self {
            path: PathBuf::from_str(&val_parse(args[0].clone(), "path")?)
                .context("path argument not a valid path")?,
        })
    }
}

/// Set the virtio-blk device to be the krun VM's root disk.
impl KrunContextSet for BlkConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let path_cstr = path_to_cstring(&self.path)?;

        if !ROOT_BLK_SET {
            if krun_set_root_disk(id, path_cstr.as_ptr()) < 0 {
                return Err(anyhow!("unable to set virtio-blk root disk"));
            }

            ROOT_BLK_SET = true;
        } else if !DATA_BLK_SET {
            if krun_set_data_disk(id, path_cstr.as_ptr()) < 0 {
                return Err(anyhow!("unable to set virtio-blk data disk"));
            }

            DATA_BLK_SET = true;
        } else {
            return Err(anyhow!("krun root and data disk already set"));
        }

        Ok(())
    }
}

/// Configuration of a virtio-serial device.
#[derive(Clone, Debug, PartialEq)]
pub struct SerialConfig {
    /// Path of a file to use as the device's log.
    pub log_file_path: PathBuf,
}

impl FromStr for SerialConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-serial", Some(1))?;

        Ok(Self {
            log_file_path: PathBuf::from_str(&val_parse(args[0].clone(), "logFilePath")?)
                .context("logFilePath argument not a valid path")?,
        })
    }
}

/// Configuration of a virtio-vsock device.
#[derive(Clone, Debug, PartialEq)]
pub struct VsockConfig {
    /// Port to connect to on VM.
    pub port: u32,

    /// Path of underlying socket.
    pub socket_url: PathBuf,

    /// Action of socket.
    pub action: VsockAction,
}

impl FromStr for VsockConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-vsock", Some(3))?;

        let port =
            u32::from_str(&val_parse(args[0].clone(), "port")?).context("port argument invalid")?;
        let socket_url = PathBuf::from_str(&val_parse(args[1].clone(), "socketURL")?)
            .context("socketURL argument not a valid path")?;
        let action = VsockAction::from_str(&args[2])?;

        Ok(Self {
            port,
            socket_url,
            action,
        })
    }
}

/// Map the virtio-vsock's guest port and host path to enable the krun VM to communicate with the
/// socket on the host.
impl KrunContextSet for VsockConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let path_cstr = path_to_cstring(&self.socket_url)?;

        if krun_add_vsock_port(id, self.port, path_cstr.as_ptr()) < 0 {
            return Err(anyhow!(format!(
                "unable to add vsock port {} for path {}",
                self.port,
                &self.socket_url.display()
            )));
        }

        Ok(())
    }
}

/// virtio-vsock action.
#[derive(Clone, Debug, PartialEq)]
pub enum VsockAction {
    Listen,
}

impl FromStr for VsockAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_string().to_lowercase();

        match &s[..] {
            "listen" => Ok(Self::Listen),
            _ => Err(anyhow!("invalid vsock action")),
        }
    }
}

/// Configuration of a virtio-net device.
#[derive(Clone, Debug, PartialEq)]
pub struct NetConfig {
    /// Path to underlying gvproxy socket.
    pub unix_socket_path: PathBuf,

    /// Network MAC address.
    pub mac_address: MacAddress,
}

impl FromStr for NetConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-net", Some(2))?;

        Ok(Self {
            unix_socket_path: PathBuf::from_str(&val_parse(args[0].clone(), "unixSocketPath")?)
                .context("unixSocketPath argument not a valid path")?,
            mac_address: MacAddress::from_str(&val_parse(args[1].clone(), "mac")?)
                .context("unable to parse mac address from argument")?,
        })
    }
}

/// Set the gvproxy's path and network MAC address.
impl KrunContextSet for NetConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let path_cstr = path_to_cstring(&self.unix_socket_path)?;
        let mac = self.mac_address.bytes();

        if krun_set_gvproxy_path(id, path_cstr.as_ptr()) < 0 {
            return Err(anyhow!(format!(
                "unable to set gvproxy path {}",
                &self.unix_socket_path.display()
            )));
        }

        if krun_set_net_mac(id, mac.as_ptr()) < 0 {
            return Err(anyhow!(format!(
                "unable to set net MAC address {}",
                self.mac_address
            )));
        }

        Ok(())
    }
}

/// Configuration of a virtio-fs device.
#[derive(Clone, Debug, PartialEq)]
pub struct FsConfig {
    /// Shared directory with the host.
    pub shared_dir: PathBuf,

    /// Guest mount tag for shared directory.
    pub mount_tag: PathBuf,
}

impl FromStr for FsConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-fs", None)?;

        if args.len() < 2 {
            return Err(anyhow!(
                "expected at least 2 arguments, found {}",
                args.len()
            ));
        }

        let shared_dir = PathBuf::from_str(&val_parse(args[0].clone(), "sharedDir")?)
            .context("sharedDir argument not a valid path")?;
        let mount_tag = PathBuf::from_str(&val_parse(args[1].clone(), "mountTag")?)
            .context("mountTag argument not a valid path")?;

        Ok(Self {
            shared_dir,
            mount_tag,
        })
    }
}

/// Set the shared directory with its guest mount tag.
impl KrunContextSet for FsConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let shared_dir_cstr = path_to_cstring(&self.shared_dir)?;
        let mount_tag_cstr = path_to_cstring(&self.mount_tag)?;

        if krun_add_virtiofs(id, mount_tag_cstr.as_ptr(), shared_dir_cstr.as_ptr()) < 0 {
            return Err(anyhow!(format!(
                "unable to add virtiofs shared directory {} with mount tag {}",
                &self.shared_dir.display(),
                &self.mount_tag.display()
            )));
        }

        Ok(())
    }
}

/// Configuration of a virtio-gpu device.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuConfig {
    /// Width (pixels).
    pub width: u32,

    /// Height (pixels).
    pub height: u32,
}

impl FromStr for GpuConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-gpu", Some(2))?;

        let width = u32::from_str(&val_parse(args[0].clone(), "width")?)
            .context("GPU width argument not a valid u32")?;
        let height = u32::from_str(&val_parse(args[1].clone(), "height")?)
            .context("GPU height argument not a valid u32")?;

        Ok(Self { width, height })
    }
}

/// Configuration of a virtio-input device. This is an enum indicating which virtio-input device a
/// user would like to include with the VM.
#[derive(Clone, Debug, PartialEq)]
pub enum InputConfig {
    Keyboard,
    Pointing,
}

impl FromStr for InputConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args = args_parse(s.to_string(), "virtio-input", Some(1))?;

        match &args[0].to_lowercase()[..] {
            "keyboard" => Ok(Self::Keyboard),
            "pointing" => Ok(Self::Pointing),
            _ => Err(anyhow!("invalid virtio-input config")),
        }
    }
}

/// Construct a NULL-terminated C string from a Rust Path object.
fn path_to_cstring(path: &Path) -> Result<CString, anyhow::Error> {
    let cstring = CString::new(path.as_os_str().as_bytes()).context(format!(
        "unable to convert path {} into NULL-terminated C string",
        path.display()
    ))?;

    Ok(cstring)
}
