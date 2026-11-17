//! Puts the app's icon in the Windows exe.

fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        println!("cargo:rerun-if-changed=bluearchive-rio.rc");
        println!("cargo:rerun-if-changed=icons/icon.ico");
        embed_resource::compile("bluearchive-rio.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("the exe's icon is embedded");
    }
}
