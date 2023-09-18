use std::ffi::OsStr;

/// Taken from https://gitlab.com/leonhard-llc/ops/-/tree/main/build-data
///
/// Executes `cmd` with `args` as parameters, waits for it to exit, and
/// returns its stdout, trimmed, and escaped with
/// [`escape_ascii`](#method.escape_ascii).
///
/// # Errors
/// Returns a descriptive error string if it fails to execute the command
/// or if the command exits with a non-zero status.
///
/// # Panics
/// Panics if the process writes non-UTF bytes to stdout.
pub fn exec(cmd: impl AsRef<OsStr>, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(cmd.as_ref())
        .args(args)
        .output()
        .map_err(|e| {
            format!(
                "error executing '{} {}': {e}",
                cmd.as_ref().to_string_lossy(),
                args.join(" "),
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "command '{} {}' failed: exit={} stdout='{}' stderr='{}'",
            cmd.as_ref().to_string_lossy(),
            args.join(" "),
            output
                .status
                .code()
                .map_or_else(|| String::from("signal"), |c| c.to_string()),
            escape_ascii(output.stdout),
            escape_ascii(output.stderr)
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout).map_err(|_| {
        format!(
            "command '{} {}' wrote non-utf8 bytes to stdout",
            cmd.as_ref().to_string_lossy(),
            args.join(" ")
        )
    })?;
    Ok(escape_ascii(stdout.trim()).replace('"', "\\"))
}

/// Taken from https://gitlab.com/leonhard-llc/ops/-/tree/main/build-data
///
/// Converts a byte slice into a string using
/// [`core::ascii::escape_default`](https://doc.rust-lang.org/core/ascii/fn.escape_default.html)
/// to escape each byte.
///
/// # Example
/// ```
/// use build_data::escape_ascii;
/// assert_eq!("abc", escape_ascii(b"abc"));
/// assert_eq!("abc\\n", escape_ascii(b"abc\n"));
/// assert_eq!(
///     "Euro sign: \\xe2\\x82\\xac",
///     escape_ascii("Euro sign: \u{20AC}".as_bytes())
/// );
/// assert_eq!("\\x01\\x02\\x03", escape_ascii(&[1, 2, 3]));
/// ```
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn escape_ascii(input: impl AsRef<[u8]>) -> String {
    let mut result = String::new();
    for byte in input.as_ref() {
        for ascii_byte in core::ascii::escape_default(*byte) {
            result.push_str(core::str::from_utf8(&[ascii_byte]).unwrap());
        }
    }
    result
}

fn main() {
    println!(
        "cargo:rustc-env=GIT_COMMIT={}",
        exec("git", &["rev-parse", "HEAD"]).unwrap()
    );

    let metadata = cargo_metadata::MetadataCommand::new().exec().unwrap();
    let typst = metadata
        .packages
        .iter()
        .find(|package| package.name == "typst")
        .expect("Typst should be a dependency");

    println!("cargo:rustc-env=TYPST_VERSION={}", typst.version);
}
