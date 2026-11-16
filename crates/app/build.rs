/// The commit this program is built from, for the nightly check. The two variables set
/// in the environment stand in where there is no `.git` (a source archive).
fn commit_of_build() {
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (out.status.success() && !text.is_empty()).then_some(text)
    };
    let asked = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let commit = asked("RIMSTACK_COMMIT").or_else(|| git(&["rev-parse", "--short=7", "HEAD"]));
    let made =
        asked("RIMSTACK_COMMIT_TIME").or_else(|| git(&["show", "-s", "--format=%ct", "HEAD"]));
    if let (Some(commit), Some(made)) = (commit, made) {
        println!("cargo::rustc-env=RIMSTACK_COMMIT={commit}");
        println!("cargo::rustc-env=RIMSTACK_COMMIT_TIME={made}");
    }
    println!("cargo::rerun-if-env-changed=RIMSTACK_COMMIT");
    println!("cargo::rerun-if-env-changed=RIMSTACK_COMMIT_TIME");
    println!("cargo::rerun-if-changed=../../.git/HEAD");
}

fn main() {
    commit_of_build();
    embed_resource::compile("assets/rimstack.rc", embed_resource::NONE)
        .manifest_required()
        .expect("the icon is compiled into the Windows program");
    println!("cargo::rerun-if-changed=assets/rimstack.rc");
    println!("cargo::rerun-if-changed=assets/rimstack.ico");
}
