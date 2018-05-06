extern crate cc;

use std::env;

use std::path::{Component, Path, PathBuf, Prefix};
use std::process::Command;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;

fn build_from_source() {
    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();
    let src = env::current_dir().unwrap();
    let dst = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    if !Path::new("libpcap/.git").exists() {
        let _ = Command::new("git")
            .args(&["submodule", "update", "--init"])
            .status();
    }

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=pcap");
    println!("cargo:root={}", dst.display());
    println!("cargo:include={}/include", dst.display());

    // println!("cargo:rustc-link-lib=nl-3");
    // println!("cargo:rustc-link-lib=nl-genl-3");
    // println!("cargo:rustc-link-lib=dbus-1");

    let cfg = cc::Build::new();
    let compiler = cfg.get_compiler();

    let _ = fs::create_dir(&dst.join("build"));

    let mut cmd = Command::new("sh");
    let mut cflags = OsString::new();
    for arg in compiler.args() {
        cflags.push(arg);
        cflags.push(" ");
    }

    cmd.env("CC", compiler.path())
        .env("CFLAGS", cflags)
        .env("LD", &which("ld").unwrap())
        .env("VERBOSE", "1")
        .current_dir(&dst.join("build"))
        .arg(msys_compatible(&src.join("libpcap/configure")));

    // These aren't really needed...
    cmd.arg("--enable-dbus=no");
    cmd.arg("--enable-packet-ring=no");
    cmd.arg("--enable-usb=no");
    cmd.arg("--without-libnl");

    // cmd.arg("--enable-static=yes");
    cmd.arg("--enable-shared=no");
    cmd.arg(format!("--prefix={}", msys_compatible(&dst)));

    if target != host {
        // NOTE GNU terminology
        // BUILD = machine where we are (cross) compiling pcap
        // HOST = machine where the compiled pcap will be used
        // TARGET = only relevant when compiling compilers
        cmd.arg(format!("--build={}", host));
        cmd.arg(format!("--host={}", target));
        cmd.arg("--with-pcap=linux");
    }

    run(&mut cmd, "sh");
    run(
        make()
            .arg(&format!("-j{}", env::var("NUM_JOBS").unwrap()))
            .current_dir(&dst.join("build")),
        "make",
    );
    run(
        make().arg("install").current_dir(&dst.join("build")),
        "make",
    );
}

fn run(cmd: &mut Command, program: &str) {
    println!("running: {:?}", cmd);
    let status = match cmd.status() {
        Ok(status) => status,
        Err(ref e) if e.kind() == ErrorKind::NotFound => {
            fail(&format!(
                "failed to execute command: {}\nis `{}` not installed?",
                e, program
            ));
        }
        Err(e) => fail(&format!("failed to execute command: {}", e)),
    };
    if !status.success() {
        fail(&format!(
            "command did not execute successfully, got: {}",
            status
        ));
    }
}

fn fail(s: &str) -> ! {
    panic!("\n{}\n\nbuild script failed, must exit now", s)
}

fn make() -> Command {
    let cmd = if cfg!(target_os = "freebsd") {
        "gmake"
    } else {
        "make"
    };
    let mut cmd = Command::new(cmd);
    // We're using the MSYS make which doesn't work with the mingw32-make-style
    // MAKEFLAGS, so remove that from the env if present.
    if cfg!(windows) {
        cmd.env_remove("MAKEFLAGS").env_remove("MFLAGS");
    }
    return cmd;
}

fn which(cmd: &str) -> Option<PathBuf> {
    let cmd = format!("{}{}", cmd, env::consts::EXE_SUFFIX);
    let paths = env::var_os("PATH").unwrap();
    env::split_paths(&paths)
        .map(|p| p.join(&cmd))
        .find(|p| fs::metadata(p).is_ok())
}

fn msys_compatible(path: &Path) -> String {
    let mut path_string = path.to_str().unwrap().to_string();
    if !cfg!(windows) {
        return path_string;
    }

    // Replace e.g. C:\ with /c/
    if let Component::Prefix(prefix_component) = path.components().next().unwrap() {
        if let Prefix::Disk(disk) = prefix_component.kind() {
            let from = format!("{}:\\", disk as char);
            let to = format!("/{}/", (disk as char).to_ascii_lowercase());
            path_string = path_string.replace(&from, &to);
        }
    }
    path_string.replace("\\", "/")
}

fn link_to_system_lib() {
    if let Ok(libdir) = env::var("PCAP_LIBDIR") {
        println!("cargo:rustc-link-search=native={}", libdir);
    }
}

fn main() {
    if let Ok(_) = env::var("CARGO_FEATURE_BUILD") {
        build_from_source();
    } else {
        link_to_system_lib();
    }
}
