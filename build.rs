#[cfg(target_os = "windows")]
extern crate winres;

#[cfg(target_os = "windows")]
fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("archon.ico")
        .set("InternalName", "archon.exe")
        .set("ProductName", "Archon")
        .set("FileDescription", "Archon");
        res.compile().unwrap();
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}