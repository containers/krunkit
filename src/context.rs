// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::convert::TryFrom;

use anyhow::anyhow;

extern "C" {
    fn krun_create_ctx() -> i32;
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

        Ok(Self { id, args })
    }
}
