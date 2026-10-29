//! Compiles the variadic log shim (see `csrc/log-printf-shim.c` for why it is C).

fn main() {
    println!("cargo:rerun-if-changed=csrc/log-printf-shim.c");
    cc::Build::new()
        .file("csrc/log-printf-shim.c")
        .warnings(true)
        .compile("nostyra_log_shim");
}
