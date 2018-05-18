use std::process::Command;
use std::env;
use std::fs::File;
use std::path::Path;
use std::io::Write;

fn main() {
    // Compile Gresource
    Command::new("glib-compile-resources")
        .args(&["--generate", "resources.xml"])
        .current_dir("res")
        .status()
        .unwrap();

    // Generating build globals
    let out_dir = env::var("OUT_DIR").unwrap();
    let localedir = env::var("FRACTAL_LOCALEDIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_globals.rs");
    let mut f = File::create(&dest_path).unwrap();

    let globals = format!("
pub static LOCALEDIR: &'static str = \"{}\";
",
        localedir);

    f.write_all(&globals.into_bytes()[..]).unwrap();
}
