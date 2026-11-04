//! Embeds the application manifest (DPI awareness, common controls v6,
//! Windows 10/11 compatibility) into the exe.

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    embed_resource::compile("app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed app.manifest");
}
