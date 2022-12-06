use std::{env, path::PathBuf};

fn main() {
    let project_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cplus_header = project_dir.join("c++/interface.h");

    // we also need to install emp-toolkit
    let eiffel_install_prefix = cmake::Config::new("c++").build();
    println!(
        "cargo:rustc-link-search={}",
        eiffel_install_prefix.display()
    );

    println!(
        "cargo:rustc-link-search={}/lib",
        eiffel_install_prefix.display()
    );

    println!("cargo:rustc-link-lib=static=eiffelcpp");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=dylib=c++");
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    println!("cargo:rustc-link-lib=ssl");
    println!("cargo:rustc-link-lib=crypto");

    println!("cargo:rerun-if-changed={}", "c++");

    let bindings = bindgen::Builder::default()
        .header(format!("{}", cplus_header.to_str().unwrap()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("unable to generate bindings");

    bindings
        .write_to_file(project_dir.join("src/bindings.rs"))
        .expect("Coundn't write bindings!");
}
