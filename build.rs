// Versão gravada no programa (commit), para o log do Kindle dizer qual compilação está rodando.
fn main() {
    println!("cargo:rerun-if-env-changed=VERSO_GIT");
    println!("cargo:rerun-if-changed=.git/HEAD");
    let v = std::env::var("VERSO_GIT").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        std::process::Command::new("git")
            .args(["describe", "--always", "--dirty=+"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "dev".into())
    });
    println!("cargo:rustc-env=VERSO_GIT={v}");
}
