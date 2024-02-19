// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::{
    status::{get_shutdown_eventfd, status_listener},
    virtio::KrunContextSet,
};

use std::{convert::TryFrom, thread};

use anyhow::anyhow;

#[link(name = "krun-efi")]
extern "C" {
    fn krun_create_ctx() -> i32;
    fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
    fn krun_start_enter(ctx_id: u32) -> i32;
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
            return Err(anyhow!("zero vcpus inputted (invalid)"));
        }

        if args.memory == 0 {
            return Err(anyhow!("zero MiB RAM inputted (invalid)"));
        }

        if unsafe { krun_set_vm_config(id, args.cpus, args.memory) } < 0 {
            return Err(anyhow!("unable to set krun vCPU/RAM configuration"));
        }

        // Configure each virtio device to include in the VM.
        for device in &args.devices {
            unsafe { device.krun_ctx_set(id)? }
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

        thread::spawn(move || status_listener(shutdown_eventfd).unwrap());

        // Run the workload.
        if unsafe { krun_start_enter(self.id) } < 0 {
            return Err(anyhow!("unable to begin running krun workload"));
        }

        Ok(())
    }
}
