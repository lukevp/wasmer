//! This file was mostly taken from the llvm-sys crate.
//! (https://bitbucket.org/tari/llvm-sys.rs/src/21ab524ec4df1450035df895209c3f8fbeb8775f/build.rs?at=default&fileviewer=file-view-default)

use lazy_static::lazy_static;
use regex::Regex;
use semver::Version;
use std::env;
use std::ffi::OsStr;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::Command;

lazy_static! {
    /// LLVM version used by this version of the crate.
    static ref CRATE_VERSION: Version = {
        let crate_version = Version::parse(env!("CARGO_PKG_VERSION"))
            .expect("Crate version is somehow not valid semver");
        Version {
            major: crate_version.major / 10,
            minor: crate_version.major % 10,
            .. crate_version
        }
    };

    static ref LLVM_CONFIG_BINARY_NAMES: Vec<String> = {
        vec![
            "llvm-config".into(),
            // format!("llvm-config-{}", CRATE_VERSION.major),
            // format!("llvm-config-{}.{}", CRATE_VERSION.major, CRATE_VERSION.minor),
        ]
    };

    /// Filesystem path to an llvm-config binary for the correct version.
    static ref LLVM_CONFIG_PATH: PathBuf = {
        // Try llvm-config via PATH first.
        if let Some(name) = locate_system_llvm_config() {
            return name.into();
        } else {
            println!("Didn't find usable system-wide LLVM.");
        }

        // Did the user give us a binary path to use? If yes, try
        // to use that and fail if it doesn't work.
        let binary_prefix_var = "LLVM_SYS_70_PREFIX";

        let path = if let Some(path) = env::var_os(&binary_prefix_var) {
            Some(path.to_str().unwrap().to_owned())
        } else if let Ok(mut file) = std::fs::File::open(".llvmenv") {
            use std::io::Read;
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();
            s.truncate(s.len() - 4);
            Some(s)
        } else {
            None
        };

        if let Some(path) = path {
            for binary_name in LLVM_CONFIG_BINARY_NAMES.iter() {
                let mut pb: PathBuf = path.clone().into();
                pb.push("bin");
                pb.push(binary_name);

                let ver = llvm_version(&pb)
                    .expect(&format!("Failed to execute {:?}", &pb));
                if is_compatible_llvm(&ver) {
                    return pb;
                } else {
                    println!("LLVM binaries specified by {} are the wrong version.
                              (Found {}, need {}.)", binary_prefix_var, ver, *CRATE_VERSION);
                }
            }
        }

        println!("No suitable version of LLVM was found system-wide or pointed
                  to by {}.
                  
                  Consider using `llvmenv` to compile an appropriate copy of LLVM, and
                  refer to the llvm-sys documentation for more information.
                  
                  llvm-sys: https://crates.io/crates/llvm-sys
                  llvmenv: https://crates.io/crates/llvmenv", binary_prefix_var);
        panic!("Could not find a compatible version of LLVM");
    };
}

/// Try to find a system-wide version of llvm-config that is compatible with
/// this crate.
///
/// Returns None on failure.
fn locate_system_llvm_config() -> Option<&'static str> {
    for binary_name in LLVM_CONFIG_BINARY_NAMES.iter() {
        match llvm_version(binary_name) {
            Ok(ref version) if is_compatible_llvm(version) => {
                // Compatible version found. Nice.
                return Some(binary_name);
            }
            Ok(version) => {
                // Version mismatch. Will try further searches, but warn that
                // we're not using the system one.
                println!(
                    "Found LLVM version {} on PATH, but need {}.",
                    version, *CRATE_VERSION
                );
            }
            Err(ref e) if e.kind() == ErrorKind::NotFound => {
                // Looks like we failed to execute any llvm-config. Keep
                // searching.
            }
            // Some other error, probably a weird failure. Give up.
            Err(e) => panic!("Failed to search PATH for llvm-config: {}", e),
        }
    }

    None
}

/// Check whether the given LLVM version is compatible with this version of
/// the crate.
fn is_compatible_llvm(llvm_version: &Version) -> bool {
    let strict = env::var_os(format!(
        "LLVM_SYS_{}_STRICT_VERSIONING",
        env!("CARGO_PKG_VERSION_MAJOR")
    ))
    .is_some()
        || cfg!(feature = "strict-versioning");
    if strict {
        llvm_version.major == CRATE_VERSION.major && llvm_version.minor == CRATE_VERSION.minor
    } else {
        llvm_version.major >= CRATE_VERSION.major
            || (llvm_version.major == CRATE_VERSION.major
                && llvm_version.minor >= CRATE_VERSION.minor)
    }
}

/// Get the output from running `llvm-config` with the given argument.
///
/// Lazily searches for or compiles LLVM as configured by the environment
/// variables.
fn llvm_config(arg: &str) -> String {
    llvm_config_ex(&*LLVM_CONFIG_PATH, arg).expect("Surprising failure from llvm-config")
}

/// Invoke the specified binary as llvm-config.
///
/// Explicit version of the `llvm_config` function that bubbles errors
/// up.
fn llvm_config_ex<S: AsRef<OsStr>>(binary: S, arg: &str) -> io::Result<String> {
    Command::new(binary)
        .arg(arg)
        .arg("--link-static") // Don't use dylib for >= 3.9
        .output()
        .map(|output| {
            String::from_utf8(output.stdout).expect("Output from llvm-config was not valid UTF-8")
        })
}

/// Get the LLVM version using llvm-config.
fn llvm_version<S: AsRef<OsStr>>(binary: S) -> io::Result<Version> {
    let version_str = llvm_config_ex(binary.as_ref(), "--version")?;

    // LLVM isn't really semver and uses version suffixes to build
    // version strings like '3.8.0svn', so limit what we try to parse
    // to only the numeric bits.
    let re = Regex::new(r"^(?P<major>\d+)\.(?P<minor>\d+)(?:\.(?P<patch>\d+))??").unwrap();
    let c = re
        .captures(&version_str)
        .expect("Could not determine LLVM version from llvm-config.");

    // some systems don't have a patch number but Version wants it so we just append .0 if it isn't
    // there
    let s = match c.name("patch") {
        None => format!("{}.0", &c[0]),
        Some(_) => c[0].to_string(),
    };
    Ok(Version::parse(&s).unwrap())
}

fn get_llvm_cxxflags() -> String {
    let output = llvm_config("--cxxflags");

    // llvm-config includes cflags from its own compilation with --cflags that
    // may not be relevant to us. In particularly annoying cases, these might
    // include flags that aren't understood by the default compiler we're
    // using. Unless requested otherwise, clean CFLAGS of options that are
    // known to be possibly-harmful.
    let no_clean = env::var_os(format!(
        "LLVM_SYS_{}_NO_CLEAN_CFLAGS",
        env!("CARGO_PKG_VERSION_MAJOR")
    ))
    .is_some();
    if no_clean || cfg!(target_env = "msvc") {
        // MSVC doesn't accept -W... options, so don't try to strip them and
        // possibly strip something that should be retained. Also do nothing if
        // the user requests it.
        return output;
    }

    output
        .split(&[' ', '\n'][..])
        .filter(|word| !word.starts_with("-W"))
        .filter(|word| word != &"-fno-exceptions")
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    std::env::set_var("CXXFLAGS", get_llvm_cxxflags());
    cc::Build::new()
        .cpp(true)
        .file("cpp/object_loader.cpp")
        .compile("llvm-backend");

    println!("cargo:rustc-link-lib=static=llvm-backend");
    println!("cargo:rerun-if-changed=build.rs");

    // Enable "nightly" cfg if the current compiler is nightly.
    if rustc_version::version_meta().unwrap().channel == rustc_version::Channel::Nightly {
        println!("cargo:rustc-cfg=nightly");
    }
}
