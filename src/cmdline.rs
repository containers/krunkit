// SPDX-License-Identifier: Apache-2.0

use crate::virtio::VirtioDeviceConfig;

use std::{path::PathBuf, str::FromStr};

use anyhow::{anyhow, Context, Result};
use clap::Parser;

/// Command line arguments to configure a krun VM.
#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Number of vCPUs for the VM.
    #[arg(long)]
    pub cpus: u8,

    /// Amount of RAM available to VM.
    #[arg(long)]
    pub memory: u32,

    /// Bootloader configuration.
    #[arg(long)]
    pub bootloader: bootloader::Config,

    /// virtio devices to configure in the VM.
    #[arg(long = "device")]
    pub devices: Vec<VirtioDeviceConfig>,

    /// URI of the status/shutdown listener.
    #[arg(long = "restful-uri")]
    pub restful_uri: String,
}

/// Parse a string into a vector of substrings, all of which are separated by commas.
pub fn args_parse(s: String, label: &str, sz: Option<usize>) -> Result<Vec<String>> {
    let list: Vec<String> = s.split(',').map(|s| s.to_string()).collect();

    // If an expected size is given, ensure that the parsed vector is of the expected size.
    if let Some(size) = sz {
        if list.len() != size {
            return Err(anyhow!(
                "expected --{} argument to have {} comma-separated sub-arguments, found {}",
                label,
                size,
                list.len()
            ));
        }
    }

    Ok(list)
}

/// Parse the value of some expected label, in which the two are separated by an '=' character.
///
/// For example, if a string is hello=world, "hello" is the label and "world" is the value.
pub fn val_parse(s: String, label: &str) -> Result<String> {
    let vals: Vec<&str> = s.split('=').collect();

    match vals.len() {
        1 => Ok(vals[0].to_string()),
        2 => {
            // Ensure that the label is as expected.
            let label_found = vals[0];
            if label_found != label {
                return Err(anyhow!(format!(
                    "expected label {}, found {}",
                    label, label_found
                )));
            }

            Ok(vals[1].to_string())
        }
        _ => Err(anyhow!(format!("invalid argument format: {}", s.clone()))),
    }
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
            let args = args_parse(s.to_string(), "bootloader", Some(3))?;

            let fw = BootloaderFw::from_str(&args[0])?;
            let v = Vstore::from_str(&args[1])?;
            let action = Action::from_str(&args[2])?;

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
            let value = val_parse(s.to_string(), "variable-store")?;

            Ok(Self(
                PathBuf::from_str(&value).context("variable-store argument not a valid path")?,
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
