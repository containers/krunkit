// SDPX-License-Identifier: Apache-2.0

fn main() {
    #[cfg(target_os = "macos")]
    {
        match std::env::var_os("LIBKRUN_EFI") {
            Some(path) => println!("cargo:rustc-link-search={}", path.into_string().unwrap()),
            None => println!("cargo:rustc-link-search=/opt/homebrew/lib"),
        }
    }
}
