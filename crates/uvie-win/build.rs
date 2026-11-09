//! Embeds the application manifest (DPI awareness, common controls v6,
//! Windows 10/11 compatibility) and a VERSIONINFO block into the exe.
//! VERSIONINFO is generated from `CARGO_PKG_VERSION` so the binary always
//! reports the crate version — real file metadata matters for
//! SmartScreen/AV reputation on an unsigned or newly-signed binary.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");

    let version = env!("CARGO_PKG_VERSION");
    let mut nums = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let (maj, min, patch) = (
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
        nums.next().unwrap_or(0),
    );

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("uvie-version.rc");
    fs::write(
        &out,
        format!(
            "1 VERSIONINFO\n\
             FILEVERSION {maj},{min},{patch},0\n\
             PRODUCTVERSION {maj},{min},{patch},0\n\
             FILEFLAGSMASK 0x3fL\n\
             FILEFLAGS 0x0L\n\
             FILEOS 0x40004L\n\
             FILETYPE 0x1L\n\
             FILESUBTYPE 0x0L\n\
             BEGIN\n\
             \x20   BLOCK \"StringFileInfo\"\n\
             \x20   BEGIN\n\
             \x20       BLOCK \"040904b0\"\n\
             \x20       BEGIN\n\
             \x20           VALUE \"CompanyName\", \"uvie-project\"\n\
             \x20           VALUE \"FileDescription\", \"UVie for Windows - Vietnamese input method\"\n\
             \x20           VALUE \"FileVersion\", \"{version}\"\n\
             \x20           VALUE \"InternalName\", \"uvie-win.exe\"\n\
             \x20           VALUE \"LegalCopyright\", \"MIT OR Apache-2.0\"\n\
             \x20           VALUE \"OriginalFilename\", \"uvie-win.exe\"\n\
             \x20           VALUE \"ProductName\", \"UVie for Windows\"\n\
             \x20           VALUE \"ProductVersion\", \"{version}\"\n\
             \x20       END\n\
             \x20   END\n\
             \x20   BLOCK \"VarFileInfo\"\n\
             \x20   BEGIN\n\
             \x20       VALUE \"Translation\", 0x409, 1200\n\
             \x20   END\n\
             END\n",
        ),
    )
    .expect("failed to write version.rc");

    embed_resource::compile("app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed app.manifest");
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed version.rc");
}
