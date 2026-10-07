// cfg(windows) is the build host, not the target: other hosts (no resource
// compiler) embed nothing and never build embed-resource.
#[cfg(windows)]
fn embed_windows_icon() {
    println!("cargo::rerun-if-changed=../icons/deskpuck.rc");
    println!("cargo::rerun-if-changed=../icons/deskpuck.ico");
    let result = embed_resource::compile("../icons/deskpuck.rc", embed_resource::NONE);
    if let Err(problem) = result.manifest_required() {
        panic!("could not embed the Windows icon: {problem}");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {}
