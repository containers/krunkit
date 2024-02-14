// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

mod cmdline;

use cmdline::Args;

use clap::Parser;

fn main() {
    let _args = Args::parse();
}
