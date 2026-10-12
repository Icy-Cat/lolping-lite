// Optional static libvpx (VP9 decoder only) for v2 WebM emotes.
// Looks in $LIBVPX_DIR or ./vendor/libvpx (built by scripts/build-libvpx.sh).
// If absent, the app is built without VP9 and uses v1 WebP emotes.
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(has_vp9)");
    println!("cargo:rerun-if-env-changed=LIBVPX_DIR");
    println!("cargo:rerun-if-env-changed=LOLPING_NO_VP9");
    println!("cargo:rerun-if-changed=csrc/vp9shim.c");
    println!("cargo:rerun-if-changed=vendor/libvpx/lib/libvpx.a");
    if std::env::var_os("LOLPING_NO_VP9").is_some() {
        return;
    }
    let dir = std::env::var_os("LIBVPX_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("vendor/libvpx"));
    let lib = dir.join("lib");
    if !lib.join("libvpx.a").is_file() && !lib.join("vpx.lib").is_file() {
        println!("cargo:warning=libvpx not found ({}): building without VP9 (v1 WebP emotes only). Run scripts/build-libvpx.sh", dir.display());
        return;
    }
    cc::Build::new()
        .file("csrc/vp9shim.c")
        .include(dir.join("include"))
        .opt_level(2)
        .compile("lpvp9shim");
    println!("cargo:rustc-link-search=native={}", std::fs::canonicalize(&lib).unwrap().display());
    println!("cargo:rustc-link-lib=static=vpx");
    println!("cargo:rustc-cfg=has_vp9");
}
