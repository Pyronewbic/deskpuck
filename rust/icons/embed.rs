// Included by the build scripts of the crates that build a Windows .exe.

/// Puts the Deskpuck logo into a Windows .exe, so Explorer and shortcuts
/// show it. A build on Windows must embed it. cfg here is the build host,
/// as is the cfg(windows) that makes embed-resource a build dependency, so
/// other hosts (checking the Windows target from macOS, say, which has no
/// resource compiler) embed nothing and never build it.
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
