fn main() {
    let output = std::process::Command::new("which")
        .arg("nix-prefetch-git")
        .output()
        .expect("failed to run `which`");

    assert!(
        output.status.success(),
        "nix-prefetch-git not found in PATH"
    );

    let path = String::from_utf8(output.stdout).expect("valid UTF-8");
    println!("cargo:rustc-env=NIX_PREFETCH_GIT={}", path.trim());
}
