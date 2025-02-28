// SPDX-License-Identifier: Apache-2.0

use crate::cmdline::{check_required_args, check_unknown_args, parse_args};

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
    fn krun_add_disk2(
        ctx_id: u32,
        c_block_id: *const c_char,
        c_disk_path: *const c_char,
        disk_format: u32,
        read_only: bool,
    ) -> i32;
    fn krun_add_vsock_port(ctx_id: u32, port: u32, c_filepath: *const c_char) -> i32;
    fn krun_add_virtiofs(ctx_id: u32, c_tag: *const c_char, c_path: *const c_char) -> i32;
    fn krun_set_gvproxy_path(ctx_id: u32, c_path: *const c_char) -> i32;
    fn krun_set_net_mac(ctx_id: u32, c_mac: *const u8) -> i32;
    fn krun_set_console_output(ctx_id: u32, c_filepath: *const c_char) -> i32;
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DiskImageFormat {
    #[default]
    Raw = 0,
    Qcow2 = 1,
}

impl FromStr for DiskImageFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "raw" => Ok(DiskImageFormat::Raw),
            "qcow2" => Ok(DiskImageFormat::Qcow2),
            _ => Err(anyhow!("unsupported disk image format")),
        }
    }
}

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
        let args: Vec<String> = s.split(',').map(|s| s.to_string()).collect();

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
            Self::Serial(serial) => serial.krun_ctx_set(id),

            // virtio-input, virtio-gpu, and virtio-rng devices are currently not configured in
            // krun.
            _ => Ok(()),
        }
    }
}

/// Configuration of a virtio-blk device.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlkConfig {
    /// Path of the file to store as the root disk.
    pub path: PathBuf,

    /// Format of the disk image.
    pub format: DiskImageFormat,
}

impl FromStr for BlkConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut blk_config = Self::default();
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-blk", &["path"])?;

        let path = args.remove("path").unwrap();
        blk_config.path =
            PathBuf::from_str(path.as_str()).context("path argument not a valid path")?;

        if let Some(f) = args.remove("format") {
            blk_config.format = DiskImageFormat::from_str(f.as_str())?;
        }

        check_unknown_args(args, "virtio-blk")?;

        Ok(blk_config)
    }
}

/// Set the virtio-blk device to be the krun VM's root disk.
impl KrunContextSet for BlkConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let basename = match self.path.file_name() {
            Some(osstr) => osstr.to_str().unwrap_or("disk"),
            None => "disk",
        };
        let block_id_cstr = CString::new(basename).context("can't convert basename to cstring")?;
        let path_cstr = path_to_cstring(&self.path)?;

        if krun_add_disk2(
            id,
            block_id_cstr.as_ptr(),
            path_cstr.as_ptr(),
            self.format as u32,
            false,
        ) < 0
        {
            return Err(anyhow!(format!(
                "unable to set virtio-blk disk for {}",
                self.path.display()
            )));
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
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-serial", &["logFilePath"])?;

        let log_file_path = args.remove("logFilePath").unwrap();
        check_unknown_args(args, "virtio-serial")?;

        Ok(Self {
            log_file_path: PathBuf::from_str(log_file_path.as_str())
                .context("logFilePath argument not a valid path")?,
        })
    }
}

/// Set the krun console output to be written to the virtio-serial's log file.
impl KrunContextSet for SerialConfig {
    unsafe fn krun_ctx_set(&self, id: u32) -> Result<(), anyhow::Error> {
        let path_cstr = path_to_cstring(&self.log_file_path)?;

        if krun_set_console_output(id, path_cstr.as_ptr()) < 0 {
            return Err(anyhow!(
                "unable to set krun console output redirection to virtio-serial log file"
            ));
        }

        Ok(())
    }
}

/// Configuration of a virtio-vsock device.
#[derive(Clone, Debug, Default, PartialEq)]
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
        let mut vsock_config = Self::default();
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-vsock", &["port", "socketURL"])?;

        let port = args.remove("port").unwrap();
        vsock_config.port = u32::from_str(port.as_str()).context("port argument invalid")?;

        let socket_url = args.remove("socketURL").unwrap();
        vsock_config.socket_url = PathBuf::from_str(socket_url.as_str())
            .context("socketURL argument not a valid path")?;

        if let Some(v) = args.remove("listen") {
            if !v.is_empty() {
                return Err(anyhow!(format!(
                    "unexpected value for virtio-vsock argument: listen={v}"
                )));
            }
            vsock_config.action = VsockAction::from_str("listen")?
        }

        check_unknown_args(args, "virtio-vsock")?;

        Ok(vsock_config)
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
#[derive(Clone, Debug, Default, PartialEq)]
pub enum VsockAction {
    #[default]
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetConfig {
    /// Path to underlying gvproxy socket.
    pub unix_socket_path: PathBuf,

    /// Network MAC address.
    pub mac_address: MacAddress,
}

impl FromStr for NetConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut net_config = Self::default();
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-net", &["unixSocketPath", "mac"])?;

        let unix_socket_path = args.remove("unixSocketPath").unwrap();
        net_config.unix_socket_path = PathBuf::from_str(unix_socket_path.as_str())
            .context("unixSocketPath argument not a valid path")?;

        let mac = args.remove("mac").unwrap();
        net_config.mac_address = MacAddress::from_str(mac.as_str())
            .context("unable to parse mac address from argument")?;

        check_unknown_args(args, "virtio-net")?;

        Ok(net_config)
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FsConfig {
    /// Shared directory with the host.
    pub shared_dir: PathBuf,

    /// Guest mount tag for shared directory.
    pub mount_tag: PathBuf,
}

impl FromStr for FsConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut fs_config = FsConfig::default();
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-fs", &["sharedDir", "mountTag"])?;

        let shared_dir = args.remove("sharedDir").unwrap();
        fs_config.shared_dir = PathBuf::from_str(shared_dir.as_str())
            .context("sharedDir argument is not a valid path")?;

        let mount_tag = args.remove("mountTag").unwrap();
        fs_config.mount_tag =
            PathBuf::from_str(mount_tag.as_str()).context("mountTag argument not a valid path")?;

        check_unknown_args(args, "virtio-fs")?;

        Ok(fs_config)
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuConfig {
    /// Width (pixels).
    pub width: u32,

    /// Height (pixels).
    pub height: u32,
}

impl FromStr for GpuConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut gpu_config = GpuConfig::default();
        let mut args = parse_args(s.to_string())?;
        check_required_args(&args, "virtio-gpu", &["height", "width"])?;

        let width = args.remove("width").unwrap();
        gpu_config.width = u32::from_str(width.as_str()).context(format!(
            "GPU width argument out of range (0x0 - 0x{:x})",
            u32::MAX
        ))?;

        let height = args.remove("height").unwrap();
        gpu_config.height = u32::from_str(height.as_str()).context(format!(
            "GPU height argument out of range (0x0 - 0x{:x})",
            u32::MAX
        ))?;

        check_unknown_args(args, "virtio-gpu")?;

        Ok(gpu_config)
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
        let args = parse_args(s.to_string())?;

        if args.len() != 1 {
            return Err(anyhow!("invalid virtio-input config: {s}"));
        }

        let (key, value) = args.into_iter().next().unwrap();
        if !value.is_empty() {
            return Err(anyhow!(format!(
                "unexpected value for virtio-input argument: {key}={value}"
            )));
        }
        match key.as_str() {
            "keyboard" => Ok(Self::Keyboard),
            "pointing" => Ok(Self::Pointing),
            _ => Err(anyhow!("unknown virtio-input argument: {key}")),
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
