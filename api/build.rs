//! Bakes the git revision into the binary as `SESLOGIN_GIT_REV`.
//!
//! The deploy workflows run `cargo lambda deploy` with no `--env-var`, so the
//! Lambda's environment map is owned entirely by Terraform. Passing a SHA that
//! way would need a `terraform apply` per deploy, so the rev is compiled in
//! instead. This mirrors `VITE_CLIENT_VERSION` on the web side.

use std::process::Command;

fn main() {
    // `GITHUB_SHA` changes on every commit, which is what keeps the baked value
    // fresh under `Swatinem/rust-cache` in CI.
    println!("cargo:rerun-if-env-changed=GIT_REV");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-changed=../.git/HEAD");

    let rev = std::env::var("GIT_REV")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .ok()
        .filter(|rev| !rev.trim().is_empty())
        .or_else(git_rev_parse_head)
        // Local builds outside a git checkout (and vendored source trees) still
        // have to compile, so fall back rather than failing the build.
        .unwrap_or_else(|| "dev".to_string());

    println!("cargo:rustc-env=SESLOGIN_GIT_REV={}", rev.trim());
}

fn git_rev_parse_head() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let rev = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!rev.is_empty()).then_some(rev)
}
