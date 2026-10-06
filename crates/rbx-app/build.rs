//! Embeds the icon and version info in Rusticean.exe. Only for MSVC builds (CI and
//! normal Windows installs), since other toolchains may lack a resource compiler.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/rusticean.ico");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/rusticean.ico")
        .set("ProductName", "Rusticean")
        .set("FileDescription", "Rusticean")
        .set("LegalCopyright", "MIT licence");
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed the icon: {e}");
    }
}
