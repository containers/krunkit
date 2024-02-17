// SDPX-License-Identifier: Apache-2.0

fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-search=/opt/homebrew/lib");
}
