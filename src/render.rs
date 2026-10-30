//! Raster output: Graphviz `dot` -> PNG, then macOS `sips` (or ImageMagick) -> JPG.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::emit;
use crate::model::Sheet;

fn have(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Lay the sheet out left-to-right (a wide org stack becomes a readable
/// portrait page) and render it to `png` with Graphviz.
pub fn png(sheet: &Sheet, png: &Path) -> Result<()> {
    if !have("dot") {
        bail!("Graphviz 'dot' not found (install it, e.g. `brew install graphviz`)");
    }
    let src = emit::dot(std::slice::from_ref(sheet), "LR");
    let mut child = Command::new("dot")
        .args(["-Gdpi=144", "-Tpng", "-o"])
        .arg(png)
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to run dot")?;
    child.stdin.take().unwrap().write_all(src.as_bytes())?;
    if !child.wait()?.success() {
        bail!("dot failed rendering sheet '{}'", sheet.title);
    }
    Ok(())
}

/// Convert an existing PNG to JPEG (quality 92).
pub fn jpg(png: &Path, jpg: &Path) -> Result<()> {
    let status = if have("sips") {
        Command::new("sips")
            .args(["-s", "format", "jpeg", "-s", "formatOptions", "92"])
            .arg(png)
            .arg("--out")
            .arg(jpg)
            .stdout(Stdio::null())
            .status()
    } else if have("magick") {
        Command::new("magick").arg(png).args(["-quality", "92"]).arg(jpg).status()
    } else {
        bail!("no JPEG converter found (needs macOS `sips` or ImageMagick `magick`)");
    };
    if !status.context("failed to run JPEG converter")?.success() {
        bail!("JPEG conversion failed for {}", png.display());
    }
    Ok(())
}
