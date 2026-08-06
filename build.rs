//! Embed the Windows icon and version information into the executables.
//!
//! On Windows an icon is not a file beside the program, it is a resource inside
//! the `.exe`. Explorer, the taskbar, Alt-Tab and the SmartScreen prompt all read
//! it from there, so without this step `Open Samong.exe` shows the generic binary
//! icon no matter how many `.ico` files travel in the archive.
//!
//! Nothing here runs on any other platform: macOS reads its icon from the app
//! bundle's `Contents/Resources`, and Linux from the hicolor theme.
//!
//! ## Why a failure here only warns
//!
//! Embedding needs a resource compiler — `rc.exe` from the Windows SDK, or a
//! GNU `windres`. Most contributors on Windows will have one via Visual Studio,
//! but not all will, and refusing to build a knowledge base because a cosmetic
//! resource could not be compiled is out of proportion. So a failure prints a
//! warning and the build continues.
//!
//! That is a quiet fallback, which this project generally treats as a defect, so
//! it is paired with a loud check where it matters: `packaging/verify-stage.sh`
//! reads the icon back out of the built `.exe` during a release and fails if it
//! is not there. A contributor gets a warning; a release cannot ship without it.

fn main() {
    // Gated on the host rather than the target, to match where the
    // `winresource` build-dependency is declared. The release builds the Windows
    // target on a Windows runner, so the two are the same there.
    #[cfg(windows)]
    embed_windows_resources();
}

#[cfg(windows)]
fn embed_windows_resources() {
    // Without this, editing the icon does not rebuild the resource.
    println!("cargo:rerun-if-changed=assets/icon/samong.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let icon = std::path::Path::new("assets/icon/samong.ico");
    if !icon.exists() {
        // Not a warning to be ignored: the file is committed, so its absence
        // means the tree is incomplete rather than the toolchain lacking
        // something. Fail on that, because it is not the contributor's problem
        // to work around.
        panic!(
            "assets/icon/samong.ico is missing — run packaging/icons/make-icons.py, \
             or check that the file survived whatever produced this tree"
        );
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon/samong.ico");
    // Shown on the Properties → Details tab, and by SmartScreen when it warns
    // about an unsigned binary. "Unknown publisher" is unavoidable without a
    // certificate; an empty description on top of it is not.
    res.set("ProductName", "Samong");
    res.set("FileDescription", "Samong — local-first knowledge base");
    res.set("LegalCopyright", "Licensed under Apache-2.0");

    if let Err(err) = res.compile() {
        println!(
            "cargo:warning=could not embed the Windows icon ({err}). The build \
             continues without it; a resource compiler (rc.exe from the Windows \
             SDK, or windres) is what is missing."
        );
    }
}
