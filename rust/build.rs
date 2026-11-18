//! Embeds the app icon and version details into the Windows executables.

fn main() {
    println!("cargo:rerun-if-changed=../assets/app_icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../assets/app_icon.ico")
        .set("ProductName", "BBB StudyForge")
        .set("FileDescription", "BBB StudyForge - BigBlueButton lecture archiver")
        .set("CompanyName", "THE NIHILIST")
        .set("LegalCopyright", "MIT License");
    // A missing resource compiler should not break the build; the exe just keeps the default icon.
    if let Err(e) = res.compile() {
        println!("cargo:warning=Could not embed the app icon: {e}");
    }
}
