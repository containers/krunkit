// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::{
    status::{get_shutdown_eventfd, status_listener, RestfulUri},
    virtio::{DiskImageFormat, KrunContextSet},
};

use std::os::fd::{AsRawFd, RawFd};
use std::process::Command;
use std::{convert::TryFrom, ptr, thread};
use std::{
    ffi::{c_char, c_int, CString},
    fs::{File, OpenOptions},
    io::{self, Read},
    str::FromStr,
};

use anyhow::{anyhow, Context};
use env_logger::{Builder, Env, Target};
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
    fn krun_add_net_unixstream(
        ctx_id: u32,
        c_path: *const c_char,
        fd: c_int,
        c_mac: *const u8,
        features: u32,
        flags: u32,
    ) -> i32;
    fn krun_create_ctx() -> i32;
    fn krun_init_log(target: RawFd, level: u32, style: u32, options: u32) -> i32;
    fn krun_set_gpu_options2(ctx_id: u32, virgl_flags: u32, shm_size: u64) -> i32;
    fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
    fn krun_set_smbios_oem_strings(ctx_id: u32, oem_strings: *const *const c_char) -> i32;
    fn krun_set_nested_virt(ctx_id: u32, enabled: bool) -> i32;
    fn krun_check_nested_virt() -> i32;
    fn krun_start_enter(ctx_id: u32) -> i32;
}

const VIRGLRENDERER_VENUS: u32 = 1 << 6;
const VIRGLRENDERER_NO_VIRGL: u32 = 1 << 7;

/// The logging library will attempt to use escape sequences for things such as color if the target
/// supports it. However, if the target does not support these escape sequences, the library will
/// not force it and ignore them.
pub const KRUN_LOG_STYLE_AUTO: u32 = 0;

/// By using the RUST_LOG environment variable, it's possible for the user to configure the log
/// level for Rust projects.
///
/// By passing the `KRUN_LOG_OPTION_ENV` constant to the logging crate, we allow the RUST_LOG
/// environment variable to override the behavior specified by the `--krun-log-level` cmdline
/// option.
///
/// On the contrary, the `KRUN_LOG_OPTION_NO_ENV` will do the opposite. The constant will prevent
/// the RUST_LOG environment variable from overriding the behavior specified by the cmdline.
pub const KRUN_LOG_OPTION_ENV: u32 = 0;
pub const KRUN_LOG_OPTION_NO_ENV: u32 = 1;

const QCOW_MAGIC: [u8; 4] = [0x51, 0x46, 0x49, 0xfb];

fn get_image_format(disk_image: String) -> Result<DiskImageFormat, io::Error> {
    let mut file = File::open(disk_image)?;

    let mut buffer = [0u8; 4];
    file.read_exact(&mut buffer)?;

    if buffer == QCOW_MAGIC {
        println!("Image detected as Qcow2");
        Ok(DiskImageFormat::Qcow2)
    } else {
        println!("Image deteced as Raw");
        Ok(DiskImageFormat::Raw)
    }
}

/// A wrapper of all data used to configure the krun VM.
pub struct KrunContext {
    id: u32,
    args: Args,
}

/// Create a krun context from the command line arguments.
impl TryFrom<Args> for KrunContext {
    type Error = anyhow::Error;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let (log_level, options) = match args.krun_log_level {
            Some(l) => (l, KRUN_LOG_OPTION_NO_ENV),
            // If the user doesn't specify a log level, default to INFO and allow RUST_LOG
            // environment variable to override it if the variable is set.
            None => (3, KRUN_LOG_OPTION_ENV),
        };

        let (fd, target) = match args.log_file {
            Some(ref path) => {
                let file = OpenOptions::new().append(true).create(true).open(path)?;
                (file.as_raw_fd(), Target::Pipe(Box::new(file)))
            }
            None => (io::stderr().as_raw_fd(), Target::Stderr),
        };

        let ret = unsafe { krun_init_log(fd, log_level, KRUN_LOG_STYLE_AUTO, options) };
        if ret < 0 {
            return Err(anyhow!("unable to init libkrun logs: {ret:?}"));
        }

        let log_level = match log_level {
            0 => "off",
            1 => "error",
            2 => "warn",
            3 => "info",
            4 => "debug",
            _ => "trace",
        };

        let mut builder = if options == KRUN_LOG_OPTION_NO_ENV {
            let mut builder = Builder::new();
            builder.parse_filters(log_level).parse_write_style("auto");
            builder
        } else {
            env_logger::Builder::from_env(
                Env::new()
                    .default_filter_or(log_level)
                    .default_write_style_or("auto"),
            )
        };
        builder.target(target).init();

        // Create a new context in libkrun. Store identifier to later use to configure VM
        // resources and devices.
        let id = unsafe { krun_create_ctx() };
        if id < 0 {
            return Err(anyhow!("unable to create libkrun context"));
        }

        // Safe to unwrap, as it's already ensured that id >= 0.
        let id = u32::try_from(id).unwrap();

        // Set the krun VM's number of vCPUs and amount of memory allocated.
        if args.cpus == 0 {
            return Err(anyhow!("vcpus must be a minimum of 1 (0 is invalid)"));
        }

        if args.memory == 0 {
            return Err(anyhow!("zero MiB RAM inputted (invalid)"));
        } else if args.memory > 61440 {
            // Limit RAM to 60 GiB of the 62 GiB upper bound to leave room for VRAM.
            return Err(anyhow!(
                "requested RAM larger than upper limit of 61440 MiB"
            ));
        }

        if unsafe { krun_set_vm_config(id, args.cpus, args.memory) } < 0 {
            return Err(anyhow!("unable to set krun vCPU/RAM configuration"));
        }

        // Temporarily enable GPU by default
        let virgl_flags = VIRGLRENDERER_VENUS | VIRGLRENDERER_NO_VIRGL;
        let sys = sysinfo::System::new_all();
        // Limit RAM + VRAM to 64 GB (36 bit IPA address limit) minus 2 GB (start address plus rounding).
        let rounded_mem = ((args.memory as u64) / 1024 + 1) * 1024;
        let vram = std::cmp::min((63488 - rounded_mem) * 1024 * 1024, sys.total_memory());
        if unsafe { krun_set_gpu_options2(id, virgl_flags, vram) } < 0 {
            return Err(anyhow!("unable to set krun vCPU/RAM configuration"));
        }

        if let Some(ref disk_image) = args.disk_image {
            let image_format = get_image_format(disk_image.to_string())?;

            if unsafe {
                krun_add_disk2(
                    id,
                    CString::new("root").unwrap().as_ptr(),
                    CString::new(disk_image.clone()).unwrap().as_ptr(),
                    image_format as u32,
                    false,
                )
            } < 0
            {
                return Err(anyhow!("error configuring disk image"));
            }

            match Command::new("gvproxy")
                .arg("-listen-qemu")
                .arg("unix:///tmp/gvproxy-krun.sock")
                .spawn()
            {
                Ok(child) => {
                    println!("Process spawned successfully with PID: {}", child.id());
                    unsafe {
                        if krun_add_net_unixstream(
                            id,
                            CString::new("/tmp/gvproxy-krun.sock").unwrap().as_ptr(),
                            -1,
                            MacAddress::from_str("56:c8:d4:db:e1:47")
                                .unwrap()
                                .bytes()
                                .as_ptr(),
                            0,
                            0,
                        ) < 0
                        {
                            return Err(anyhow!(format!(
                                "virtio-net unable to configure default interface",
                            )));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to spawn process: {}", e);
                }
            }
        }

        // Configure each virtio device to include in the VM.
        for device in &args.devices {
            unsafe { device.krun_ctx_set(id)? }
        }

        set_smbios_oem_strings(id, &args.oem_strings)?;

        if args.nested {
            match unsafe { krun_check_nested_virt() } {
                1 => {
                    if unsafe { krun_set_nested_virt(id, args.nested) } < 0 {
                        return Err(anyhow!("krun nested virtualization reported as supported, yet failed to enable"));
                    }
                }
                0 => log::debug!("nested virtualization is not supported on this host. -n,--nested argument ignored"),
                _ => return Err(anyhow!("unable to check nested virtualization is supported on this host")),
            }
        }

        Ok(Self { id, args })
    }
}

impl KrunContext {
    /// Spawn a thread to listen for shutdown requests and run the workload. If behaving properly,
    /// the main thread will never return from this function.
    pub fn run(&self) -> Result<(), anyhow::Error> {
        // Get the krun shutdown file descriptor and listen to shutdown requests on a new thread.
        let shutdown_eventfd = unsafe { get_shutdown_eventfd(self.id) };
        let uri = self.args.restful_uri.clone();

        // Only spawn a listener thread if the user specified unix:// or tcp://
        if uri.as_ref().is_some_and(|u| *u != RestfulUri::None) {
            thread::spawn(move || status_listener(shutdown_eventfd, uri).unwrap());
        }

        // If the user provides a pidfile path, wait until the last second before running the VM to
        // write to it. We want to wait until the last minute to avoid scenarios where we write the
        // PID to the file, only for the command to be incorrect or for parsing to fail shortly
        // after. This could mislead users to belive the guest has started when it hasn't.
        if let Some(pidfile) = &self.args.pidfile {
            let pid = std::process::id();
            std::fs::write(pidfile, pid.to_string())?;
        }

        // Run the workload.
        if unsafe { krun_start_enter(self.id) } < 0 {
            if let Some(pidfile) = &self.args.pidfile {
                // Since the VM never started, remove the pidfile.
                std::fs::remove_file(pidfile)?;
            }
            return Err(anyhow!("unable to begin running krun workload"));
        }

        Ok(())
    }
}

fn set_smbios_oem_strings(
    ctx_id: u32,
    oem_strings: &Option<Vec<String>>,
) -> Result<(), anyhow::Error> {
    let Some(oem_strings) = oem_strings else {
        return Ok(());
    };

    if oem_strings.len() > u8::MAX as usize {
        return Err(anyhow!("invalid number of SMBIOS OEM strings"));
    }

    let mut cstr_vec = Vec::with_capacity(oem_strings.len());
    for s in oem_strings {
        let cs = CString::new(s.as_str()).context("invalid SMBIOS OEM string")?;
        cstr_vec.push(cs);
    }
    let mut ptr_vec: Vec<_> = cstr_vec.iter().map(|s| s.as_ptr()).collect();
    // libkrun requires an NULL terminator to indicate the end of the array
    ptr_vec.push(ptr::null());

    let ret = unsafe { krun_set_smbios_oem_strings(ctx_id, ptr_vec.as_ptr()) };
    if ret < 0 {
        return Err(anyhow!("unable to set SMBIOS OEM Strings"));
    }
    Ok(())
}
