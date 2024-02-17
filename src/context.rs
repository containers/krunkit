// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::{status::status_listener, virtio::KrunContextSet};

use std::{convert::TryFrom, thread};

use anyhow::anyhow;

extern "C" {
    fn krun_create_ctx() -> i32;
    fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
    fn krun_start_enter(ctx_id: u32) -> i32;
}

pub struct KrunContext {
    id: u32,
    args: Args,
}

impl TryFrom<Args> for KrunContext {
    type Error = anyhow::Error;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        let id = unsafe { krun_create_ctx() };
        if id < 0 {
            return Err(anyhow!("unable to create libkrun context"));
        }

        // Safe to unwrap, as it's already ensured that id >= 0.
        let id = u32::try_from(id).unwrap();

        if args.cpus == 0 {
            return Err(anyhow!("zero vcpus inputted (invalid)"));
        }

        if args.memory == 0 {
            return Err(anyhow!("zero MiB RAM inputted (invalid)"));
        }

        if unsafe { krun_set_vm_config(id, args.cpus, args.memory) } < 0 {
            return Err(anyhow!("unable to set krun vCPU/RAM configuration"));
        }

        for device in &args.devices {
            unsafe { device.krun_ctx_set(id)? }
        }

        Ok(Self { id, args })
    }
}

impl KrunContext {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        let id = self.id;

        thread::spawn(move || unsafe { status_listener(id).unwrap() });

        if unsafe { krun_start_enter(self.id) } < 0 {
            return Err(anyhow!("unable to begin running krun workload"));
        }

        Ok(())
    }
}
