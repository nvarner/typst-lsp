use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Result};
use lapce_plugin::VoltEnvironment;

use crate::TypstLspOptions;

pub fn get_server_path(typst_lsp_options: &TypstLspOptions) -> Result<String> {
    if let Some(path) = &typst_lsp_options.server_path {
        return Ok(path.to_owned());
    }

    let target_info = SupportedTargetInfo::from_volt()?;
    let expected_path = target_info.expected_server_filename();
    if !Path::new(&expected_path).exists() {
        download_server_binary(&target_info, &expected_path)?;
    }

    Ok(expected_path)
}

fn download_server_binary(target_info: &SupportedTargetInfo, path: &str) -> Result<()> {
    let mut file = File::create(path)?;

    let url = target_info.server_download_url();
    if let Err(err) = download_to(&url, &mut file) {
        let _ = std::fs::remove_file(path); // if this fails, download just isn't cleaned up, fine
        return Err(err);
    }

    Ok(())
}

fn download_to(url: &str, target: &mut impl Write) -> Result<()> {
    let mut response = lapce_plugin::Http::get(url)?;
    if !response.status_code.is_success() {
        bail!(
            "got unsuccessful status code downloading prebuilt binary from `{url}`: {}",
            response.status_code
        );
    }

    let mut buf = vec![0; 4096]; // size taken from `Response::body_read_all`
    loop {
        let bytes_read = response.body_read(&mut buf)?;

        if bytes_read == 0 {
            break;
        }

        target.write_all(&buf[0..bytes_read])?;
    }

    Ok(())
}

pub struct SupportedTargetInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub libc: String,
    pub ext: String,
}

impl SupportedTargetInfo {
    pub fn from_volt() -> Result<Self> {
        let version = env!("CARGO_PKG_VERSION");

        let (os, arch, libc) = (
            VoltEnvironment::operating_system(),
            VoltEnvironment::architecture(),
            VoltEnvironment::libc(),
        );

        let (os, arch, libc, ext) = match (os.as_deref(), arch.as_deref(), libc.as_deref()) {
            (Ok("macos"), Ok(arch @ ("aarch64" | "x86_64")), _) => ("apple", arch, "darwin", ""),
            (Ok("windows"), Ok(arch @ ("aarch64" | "x86_64")), _) => ("pc-windows", arch, "msvc", ".exe"),
            (Ok("linux"), Ok(arch @ ("aarch64" | "x86_64")), Ok("glibc")) => ("unknown-linux", arch, "gnu", ""),
            (Ok("linux"), Ok("x86_64"), Ok("musl")) => ("unknown-linux", "x86_64", "musl", ""),
            (Ok("linux"), Ok("arm"), Ok("gnu")) => ("unknown-linux", "x86_64", "gnueabihf", ""),

            (Ok("linux"), Ok(arch), Ok(libc)) => bail!("precompiled binaries are not available for OS `linux` with architecture `{arch}` and libc `{libc}`"),
            (Ok("linux"), Ok(_), Err(libc_err)) => bail!("could not get libc to find a precompiled binary: {libc_err}"),

            (Ok(os), Ok(arch), _) => bail!("precompiled binaries are not available for OS `{os}` with architecture `{arch}`"),
            (os, arch, _) => bail!("could not get all needed data to find a precompiled binary:\nOS: {os:?}\narchitecture: {arch:?}"),
        };

        Ok(Self {
            version: version.to_owned(),
            os: os.to_owned(),
            arch: arch.to_owned(),
            libc: libc.to_owned(),
            ext: ext.to_owned(),
        })
    }

    pub fn expected_server_filename(&self) -> String {
        format!(
            "typst-lsp-v{}-{}-{}-{}{}",
            self.version, self.arch, self.os, self.libc, self.ext
        )
    }

    pub fn server_download_url(&self) -> String {
        format!(
            "https://github.com/nvarner/typst-lsp/releases/download/v{}/typst-lsp-{}-{}-{}{}",
            self.version, self.arch, self.os, self.libc, self.ext
        )
    }
}
