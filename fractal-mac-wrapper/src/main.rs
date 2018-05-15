extern crate core_foundation;
extern crate libc;

use core_foundation::bundle::CFBundle;
use libc::execv;
use std::ffi::CString;
use std::ptr;

fn main() {
    // Get the URL of the executable inside the bundle and build the other paths relative to this.
    //
    // Ideally, we would use CFBundleCopyResourcesDirectoryURL and
    // CFBundleCopyAuxiliaryExecutableURL to get the paths to the Resources directory and the
    // fractal binary, but core_foundation_sys doesn't expose them for now.
    let bundle = CFBundle::main_bundle();
    let url = bundle.executable_url().unwrap();

    let mut path = url.to_path().unwrap();
    path.pop();

    // Construct the argument for execv
    let program = CString::new(format!("{}/fractal", path.to_str().unwrap())).unwrap();
    let args = vec![program.as_ptr(), ptr::null()];

    // Inject the needed environment variables
    path.pop();
    let env = vec![
        ("GDK_PIXBUF_MODULE_FILE", "Resources/gdk-loaders.cache"),
        ("GTK_IM_MODULE_FILE", "Resources/gtk.immodules"),
        ("XDG_DATA_DIRS", "Resources"),
        ("XDG_CONFIG_DIRS", "Resources"),
    ];

    for (key, val) in env {
        let val = format!("{}/{}", path.to_str().unwrap(), val);
        std::env::set_var(key, val);
    }

    // Execute the real `fractal`
    unsafe {
        execv(program.as_ptr(), args.as_ptr());
    };
}
