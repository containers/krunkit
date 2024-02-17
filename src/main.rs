// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

mod cmdline;
mod context;
mod status;
mod virtio;

use cmdline::Args;
use context::KrunContext;

use clap::Parser;

fn main() -> Result<(), anyhow::Error> {
    let _ctx = KrunContext::try_from(Args::parse())?;

    Ok(())
}
