// Included by the build scripts of the crates that build a Windows .exe.

/// Puts the Deskpuck logo into a Windows .exe, so Explorer and shortcuts
/// show it. A build on Windows must embed it; checking the Windows target
/// from macOS or Linux has no resource compiler and goes without.
fn embed_windows_icon() {
    println!("cargo::rerun-if-changed=../icons/deskpuck.rc");
    println!("cargo::rerun-if-changed=../icons/deskpuck.ico");
    let result = embed_resource::compile("../icons/deskpuck.rc", embed_resource::NONE);
    let checked = if cfg!(windows) { result.manifest_required() } else { result.manifest_optional() };
    if let Err(problem) = checked {
        panic!("could not embed the Windows icon: {problem}");
    }
}
