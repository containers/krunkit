// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::{
    status::{get_shutdown_eventfd, status_listener, RestfulUri},
    virtio::KrunContextSet,
};

use std::ffi::{c_char, CString};
use std::{convert::TryFrom, ptr, thread};

use anyhow::{anyhow, Context};

#[link(name = "krun-efi")]
extern "C" {
    fn krun_create_ctx() -> i32;
    fn krun_set_log_level(level: u32) -> i32;
    fn krun_set_gpu_options2(ctx_id: u32, virgl_flags: u32, shm_size: u64) -> i32;
    fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
    fn krun_set_smbios_oem_strings(ctx_id: u32, oem_strings: *const *const c_char) -> i32;
    fn krun_set_nested_virt(ctx_id: u32, enabled: bool) -> i32;
    fn krun_check_nested_virt() -> i32;
    fn krun_start_enter(ctx_id: u32) -> i32;
}

const VIRGLRENDERER_VENUS: u32 = 1 << 6;
const VIRGLRENDERER_NO_VIRGL: u32 = 1 << 7;

/// A wrapper of all data used to configure the krun VM.
pub struct KrunContext {
    id: u32,
    args: Args,
}

/// Create a krun context from the command line arguments.
impl TryFrom<Args> for KrunContext {
    type Error = anyhow::Error;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        // Start by setting up the desired log level for libkrun.
        unsafe { krun_set_log_level(args.krun_log_level) };

        let log_level = match args.krun_log_level {
            0 => "off",
            1 => "error",
            2 => "warn",
            3 => "info",
            4 => "debug",
            _ => "trace",
        };
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
            .init();

        // Create a new context in libkrun. Store identifier to later use to configure VM
        // resources and devices.
        let id = unsafe { krun_create_ctx() };
        if id < 0 {
            return Err(anyhow!("unable to create libkrun context"));
        }

        // Safe to unwrap, as it's already ensured that id >= 0.
        let id = u32::try_from(id).unwrap();

        // Set the krun VM's number of vCPUs and amount of memory allocated.
        //
        // libkrun has a max of 8 vCPUs allowed.
        if args.cpus == 0 {
            return Err(anyhow!("zero vcpus inputted (invalid)"));
        } else if args.cpus > 8 {
            return Err(anyhow!("too many vCPUs configured (max 8)"));
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

        if uri != Some(RestfulUri::None) {
            thread::spawn(move || status_listener(shutdown_eventfd, uri).unwrap());
        }

        // Run the workload.
        if unsafe { krun_start_enter(self.id) } < 0 {
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
