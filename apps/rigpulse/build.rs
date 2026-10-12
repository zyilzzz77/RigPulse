fn main() {
    println!("cargo:rerun-if-changed=../../RigPulse Neon Monitoring Logo.png");
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let logo = image::open("../../RigPulse Neon Monitoring Logo.png")
            .expect("Application logo missing");
        let icon = logo
            .resize(256, 256, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let mut square = image::RgbaImage::from_pixel(256, 256, image::Rgba([5, 16, 40, 255]));
        image::imageops::overlay(
            &mut square,
            &icon,
            i64::from((256 - icon.width()) / 2),
            i64::from((256 - icon.height()) / 2),
        );
        let path =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("rigpulse.ico");
        square
            .save(&path)
            .expect("Application icon encoding failed");
        winresource::WindowsResource::new()
            .set_icon(path.to_str().unwrap())
            .set_manifest_file("app.manifest")
            .set("ProductName", "RigPulse")
            .set("FileDescription", "RigPulse Hardware Monitor")
            .set("LegalCopyright", "RigPulse contributors. MIT License.")
            .compile()
            .expect("Windows resources could not be compiled");
    }
}
