use crate::base::error::{Error, Result};
use crate::said;

use std::path::Path;

pub fn page(mod_root: &Path) -> Option<String> {
    let config = std::fs::read_to_string(mod_root.join(".git").join("config")).ok()?;
    let mut in_origin = false;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            in_origin = line == "[remote \"origin\"]";
        } else if let Some(("url", url)) = line.split_once('=').map(|(k, v)| (k.trim(), v.trim()))
            && in_origin
        {
            return web(url);
        }
    }
    None
}

fn web(remote: &str) -> Option<String> {
    let remote = remote.strip_suffix(".git").unwrap_or(remote);
    if remote.starts_with("https://") || remote.starts_with("http://") {
        return Some(remote.to_string());
    }
    let (host, path) = remote.strip_prefix("git@")?.split_once(':')?;
    Some(format!("https://{host}/{path}"))
}

/// An https link's first two path parts make the repository, whatever follows them.
/// Steam's own pages are never repositories.
pub fn links_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == '<' || c == '>') {
        let Some(rest) = word.strip_prefix("https://") else {
            continue;
        };
        let mut parts = rest.split(['/', '?', '#']).filter(|part| !part.is_empty());
        let (Some(host), Some(owner), Some(repo)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if host.contains("steam") || !host.contains('.') {
            continue;
        }
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        let link = format!("https://{host}/{owner}/{repo}");
        if !repo.is_empty() && !found.contains(&link) {
            found.push(link);
        }
    }
    found
}

/// A mod's `.git` may be a stranger's: these settings stop Git running programs it names.
/// Submodules' settings are not read; `in_checkout` refuses the rest.
fn git() -> std::process::Command {
    let nowhere = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut git = crate::base::program::command("git");
    git.args(["-c", "core.fsmonitor=false"])
        .args(["-c", &format!("core.hooksPath={nowhere}")])
        .args(["-c", "core.sshCommand=ssh"])
        .args(["-c", "protocol.ext.allow=never"])
        .args(["-c", "log.showSignature=false"])
        .args(["-c", "submodule.recurse=false"])
        .args(["-c", "fetch.recurseSubmodules=false"])
        .env("GIT_TERMINAL_PROMPT", "0");
    git
}

/// Settings a plain clone writes, as `section.key` or `section.*.key`. Any other may name
/// a program, so a checkout holding one is refused.
const PLAIN_CLONE: [&str; 27] = [
    "core.repositoryformatversion",
    "core.filemode",
    "core.bare",
    "core.logallrefupdates",
    "core.ignorecase",
    "core.precomposeunicode",
    "core.symlinks",
    "core.worktree",
    "core.autocrlf",
    "extensions.objectformat",
    "extensions.partialclone",
    "remote.*.url",
    "remote.*.fetch",
    "remote.*.promisor",
    "remote.*.partialclonefilter",
    "branch.*.remote",
    "branch.*.merge",
    "branch.*.rebase",
    "branch.*.vscode-merge-base",
    "submodule.active",
    "submodule.*.url",
    "submodule.*.active",
    "user.name",
    "user.email",
    "gui.wmstate",
    "gui.geometry",
    "lfs.repositoryformatversion",
];

/// Takes a key as `git config --name-only` prints it.
fn beyond_a_clone(key: &str) -> Option<String> {
    let key = key.trim().to_lowercase();
    let (section, rest) = key.split_once('.')?;
    let pattern = match rest.rsplit_once('.') {
        Some((_, name)) => format!("{section}.*.{name}"),
        None => key.clone(),
    };
    (!PLAIN_CLONE.contains(&pattern.as_str())).then_some(key)
}

/// Whatever `.git` is (folder, link or pointer file), the repository it reaches must lie
/// inside the mod: a stranger's could name another repository of the user's.
fn repository_of(mod_root: &Path) -> Result<std::path::PathBuf> {
    let dot_git = mod_root.join(".git");
    if dot_git.symlink_metadata().is_err() {
        return Ok(dot_git);
    }
    let named = match std::fs::read_to_string(&dot_git) {
        Ok(pointer) => match pointer.trim().strip_prefix("gitdir:") {
            Some(dir) => mod_root.join(dir.trim()),
            None => dot_git,
        },
        Err(_) => dot_git,
    };
    match (dunce::canonicalize(&named), dunce::canonicalize(mod_root)) {
        (Ok(repository), Ok(root)) if repository.starts_with(&root) => Ok(repository),
        _ => Err(Error::refused(said!(
            "Its .git file points outside the mod: RimStack does not run Git there"
        ))),
    }
}

/// The repository is named, so Git never searches a folder above the mod for one.
fn in_checkout(mod_root: &Path) -> Result<std::process::Command> {
    let repository = repository_of(mod_root)?;
    if repository.join("commondir").exists() {
        return Err(Error::refused(said!(
            "It is a linked Git worktree: RimStack does not run Git there"
        )));
    }
    let listed = crate::base::program::run(
        git().arg("--git-dir").arg(&repository).args([
            "config",
            "--local",
            "--no-includes",
            "--name-only",
            "--list",
        ]),
        crate::base::program::LOCAL,
    )
    .map_err(not_run)?;
    if !listed.status.success() {
        return Err(Error::refused(said!(
            "{} is not a Git checkout",
            mod_root.display()
        )));
    }
    let mut named: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(beyond_a_clone)
        .collect();
    named.dedup();
    if !named.is_empty() {
        return Err(Error::refused(said!(
            "Its Git settings go beyond a plain clone's ({}): RimStack does not run Git there",
            named.join(", ")
        )));
    }
    let mut git = git();
    git.arg("--git-dir")
        .arg(&repository)
        .arg("--work-tree")
        .arg(mod_root);
    Ok(git)
}

fn not_run(e: std::io::Error) -> Error {
    Error::program(said!("git could not be run: {}", e))
}

pub fn folder_for(link: &str, mods: &Path) -> std::path::PathBuf {
    mods.join(link.rsplit('/').next().unwrap_or_default())
}

/// Gives its output, or on failure the last line it wrote to stderr.
fn ran(git: &mut std::process::Command, limit: std::time::Duration) -> Result<String> {
    let ran = crate::base::program::run(git, limit).map_err(not_run)?;
    if ran.status.success() {
        return Ok(String::from_utf8_lossy(&ran.stdout).trim().to_string());
    }
    let said = String::from_utf8_lossy(&ran.stderr);
    match said.lines().last() {
        Some(line) => Err(Error::program(line.trim().to_string())),
        None => Err(Error::program(said!("git failed"))),
    }
}

/// Refuses a folder that is there already. Blocks on the network.
pub fn clone(link: &str, mods: &Path) -> Result<std::path::PathBuf> {
    let into = folder_for(link, mods);
    if into.exists() {
        return Err(Error::refused(said!("{} is there already", into.display())));
    }
    // `--` ends the options: a link starting with `-` must not be read as one.
    let mut cloning = git();
    cloning.args(["clone", "--"]).arg(link).arg(&into);
    ran(&mut cloning, crate::base::program::DOWNLOAD)?;
    Ok(into)
}

/// None where Git cannot be run.
pub fn installed() -> Option<String> {
    let said = ran(git().arg("--version"), crate::base::program::LOCAL).ok()?;
    Some(said.trim_start_matches("git version ").to_string())
}

/// Fetches from the remote first: blocks on the network.
pub fn behind(mod_root: &Path) -> Result<usize> {
    let network = crate::base::program::NETWORK;
    let fetch = ["fetch", "--quiet", "--no-recurse-submodules"];
    ran(in_checkout(mod_root)?.args(fetch), network)?;
    let count = ran(
        in_checkout(mod_root)?.args(["rev-list", "--count", "HEAD..@{upstream}"]),
        network,
    )?;
    count
        .parse()
        .map_err(|_| Error::format(said!("git said {}", count)))
}

/// Newest first, at most thirty; `since` is seconds since 1970.
pub fn commits_since(mod_root: &Path, since: u64) -> Result<Vec<(u64, String)>> {
    let mut git = in_checkout(mod_root)?;
    git.args(["log", "-n", "30", "--pretty=%ct%x09%s"]);
    // Git reads `--since=@0` as now.
    if since > 0 {
        git.arg(format!("--since=@{since}"));
    }
    let said = ran(&mut git, crate::base::program::LOCAL)?;
    let commit = |line: &str| {
        let (time, text) = line.split_once('\t')?;
        Some((time.parse().ok()?, text.to_string()))
    };
    Ok(said.lines().filter_map(commit).collect())
}

/// Fast-forwards only: local changes or commits make it fail. Blocks on the network.
pub fn update(mod_root: &Path) -> Result<String> {
    let pull = ["pull", "--ff-only", "--quiet", "--no-recurse-submodules"];
    ran(
        in_checkout(mod_root)?.args(pull),
        crate::base::program::NETWORK,
    )?;
    let head = ["log", "-1", "--format=%h %s"];
    ran(
        in_checkout(mod_root)?.args(head),
        crate::base::program::LOCAL,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn repositories_are_found_among_other_links() {
        let text = "see https://github.com/Owner/Repo.git and https://gitlab.com/a/b/-/tree/main,\nhttps://steamcommunity.com/sharedfiles/filedetails/?id=1 https://github.com/Owner/Repo";
        assert_eq!(
            super::links_in(text),
            ["https://github.com/Owner/Repo", "https://gitlab.com/a/b"]
        );
        assert!(super::links_in("https://github.com/owner git@github.com:a/b.git").is_empty());
        let into = super::folder_for("https://github.com/Owner/Repo", std::path::Path::new("/m"));
        assert_eq!(into, std::path::Path::new("/m/Repo"));
    }

    use super::*;

    fn checkout(config: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let repository = tmp.path().join(".git");
        for dir in ["objects", "refs"] {
            std::fs::create_dir_all(repository.join(dir)).unwrap();
        }
        std::fs::write(repository.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(repository.join("config"), config).unwrap();
        tmp
    }

    #[test]
    fn the_origin_s_page_is_read_from_the_checkout() {
        let https = checkout(
            "[core]\n\tbare = false\n[remote \"upstream\"]\n\turl = https://example.org/up.git\n[remote \"origin\"]\n\turl = https://github.com/me/mod.git\n",
        );
        assert_eq!(
            page(https.path()).as_deref(),
            Some("https://github.com/me/mod")
        );
        let ssh = checkout("[remote \"origin\"]\n\turl = git@gitgud.io:me/mod.git\n");
        assert_eq!(
            page(ssh.path()).as_deref(),
            Some("https://gitgud.io/me/mod")
        );
    }

    #[test]
    fn a_folder_that_is_no_checkout_or_has_no_origin_has_no_page() {
        assert_eq!(page(tempfile::tempdir().unwrap().path()), None);
        assert_eq!(page(checkout("[core]\n\tbare = false\n").path()), None);
        assert_eq!(
            page(checkout("[remote \"origin\"]\n\turl = /srv/local\n").path()),
            None
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        let identity = ["-c", "user.name=t", "-c", "user.email=t@example.org"];
        let ran = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(identity)
            .args(args)
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }

    #[test]
    fn a_checkout_moves_forward_to_its_remote_and_not_over_local_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let (origin, clone) = (tmp.path().join("origin"), tmp.path().join("clone"));
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "--quiet", "-b", "main"]);
        std::fs::write(origin.join("a"), "1").unwrap();
        git(&origin, &["add", "a"]);
        git(&origin, &["commit", "--quiet", "-m", "first"]);
        git(tmp.path(), &["clone", "--quiet", "origin", "clone"]);
        std::fs::write(origin.join("a"), "2").unwrap();
        git(&origin, &["commit", "--quiet", "-am", "second"]);
        assert_eq!(behind(&clone), Ok(1));
        assert!(update(&clone).unwrap().ends_with("second"));
        let commits = commits_since(&clone, 0).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].1, "second");
        assert!(commits_since(tmp.path(), 0).is_err());
        assert_eq!(behind(&clone), Ok(0));
        assert!(behind(tmp.path()).is_err());
        assert_eq!(std::fs::read_to_string(clone.join("a")).unwrap(), "2");
        std::fs::write(clone.join("b"), "x").unwrap();
        git(&clone, &["add", "b"]);
        git(&clone, &["commit", "--quiet", "-m", "local"]);
        std::fs::write(origin.join("a"), "3").unwrap();
        git(&origin, &["commit", "--quiet", "-am", "third"]);
        assert!(update(&clone).is_err());
        assert!(update(tmp.path()).is_err());
    }

    #[test]
    fn settings_a_plain_clone_does_not_write_are_found_whatever_their_case_or_subsection() {
        let found: Vec<String> = [
            "core.bare",
            "core.fsmonitor",
            "remote.origin.url",
            "remote.my.fork.uploadpack",
            "filter.lfs.Smudge",
            "includeif.gitdir:/x.path",
            "gpg.ssh.program",
            "user.name",
            "branch.main.vscode-merge-base",
            "mergetool.vscode.cmd",
            "remote.origin.vcs",
        ]
        .into_iter()
        .filter_map(beyond_a_clone)
        .collect();
        assert_eq!(
            found,
            [
                "core.fsmonitor",
                "remote.my.fork.uploadpack",
                "filter.lfs.smudge",
                "includeif.gitdir:/x.path",
                "gpg.ssh.program",
                "mergetool.vscode.cmd",
                "remote.origin.vcs"
            ]
        );
    }

    /// Git also reads settings written on their section's own line.
    #[test]
    fn a_checkout_naming_a_program_however_it_is_written_is_refused() {
        for config in [
            "[credential]\n\thelper = store\n",
            "[credential] helper = store\n",
            "[remote \"origin\"] uploadpack = git-upload-pack\n\turl = /nowhere\n",
            "[include] path = elsewhere.cfg\n",
            "[gpg] program = touch\n[log] showSignature = true\n",
        ] {
            let tmp = checkout(config);
            let refused = behind(tmp.path()).unwrap_err();
            assert_eq!(
                refused.kind(),
                crate::base::error::Kind::Refused,
                "{config:?}"
            );
        }
    }

    #[test]
    fn git_is_never_run_on_a_repository_above_the_mod() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "--quiet", "-b", "main"]);
        let inside = tmp.path().join("Mods/plain");
        std::fs::create_dir_all(&inside).unwrap();
        let asked = behind(&inside).unwrap_err();
        assert_eq!(asked.kind(), crate::base::error::Kind::Refused);
        assert!(update(&inside).is_err());
    }

    #[test]
    fn a_checkout_naming_a_program_is_neither_asked_nor_updated() {
        let tmp = checkout("[core]\n\tsshCommand = touch /tmp/owned\n");
        let asked = behind(tmp.path()).unwrap_err();
        assert_eq!(asked.kind(), crate::base::error::Kind::Refused);
        assert!(asked.to_string().contains("core.sshcommand"));
        assert_eq!(
            update(tmp.path()).unwrap_err().kind(),
            crate::base::error::Kind::Refused
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_s_own_hooks_are_not_run() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (origin, clone) = (tmp.path().join("origin"), tmp.path().join("clone"));
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "--quiet", "-b", "main"]);
        std::fs::write(origin.join("a"), "1").unwrap();
        git(&origin, &["add", "a"]);
        git(&origin, &["commit", "--quiet", "-m", "first"]);
        git(tmp.path(), &["clone", "--quiet", "origin", "clone"]);
        std::fs::write(origin.join("a"), "2").unwrap();
        git(&origin, &["commit", "--quiet", "-am", "second"]);

        let ran = tmp.path().join("ran");
        let hook = clone.join(".git/hooks/post-merge");
        std::fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", ran.display())).unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(update(&clone).unwrap().ends_with("second"));
        assert!(!ran.exists());
    }

    /// The submodule's own settings are not read by `in_checkout`, and a fetch into the
    /// submodule would run its `uploadpack`.
    #[cfg(unix)]
    fn clone_with_a_hostile_submodule(tmp: &Path, program: &Path) -> std::path::PathBuf {
        let local = ["-c", "protocol.file.allow=always"];
        let (sub, upstream) = (tmp.join("sub"), tmp.join("upstream"));
        for dir in [&sub, &upstream] {
            std::fs::create_dir(dir).unwrap();
            git(dir, &["init", "--quiet", "-b", "main"]);
            git(dir, &["commit", "--quiet", "--allow-empty", "-m", "first"]);
        }
        let add = ["submodule", "add", "-q", sub.to_str().unwrap(), "sub"];
        git(&upstream, &[&local[..], &add].concat());
        git(&upstream, &["commit", "--quiet", "-m", "with sub"]);
        let clone = tmp.join("clone");
        let from = [upstream.to_str().unwrap(), clone.to_str().unwrap()];
        git(
            tmp,
            &[&local[..], &["clone", "-q", "--recurse-submodules"], &from].concat(),
        );
        let in_sub = clone.join("sub");
        git(
            &in_sub,
            &[
                "config",
                "remote.origin.uploadpack",
                program.to_str().unwrap(),
            ],
        );
        git(&in_sub, &["config", "protocol.file.allow", "always"]);
        git(
            &sub,
            &["commit", "--quiet", "--allow-empty", "-m", "second"],
        );
        git(
            &upstream.join("sub"),
            &[&local[..], &["pull", "-q", "origin", "main"]].concat(),
        );
        git(&upstream, &["commit", "--quiet", "-am", "sub moved"]);
        clone
    }

    #[cfg(unix)]
    #[test]
    fn a_program_named_in_a_submodule_s_settings_is_never_run() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (program, ran) = (tmp.path().join("program"), tmp.path().join("ran"));
        let script = format!(
            "#!/bin/sh\ntouch '{}'\nexec git-upload-pack \"$@\"\n",
            ran.display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let clone = clone_with_a_hostile_submodule(tmp.path(), &program);
        assert_eq!(behind(&clone), Ok(1));
        assert!(update(&clone).unwrap().ends_with("sub moved"));
        assert!(!ran.exists());

        git(
            &clone,
            &[
                "config",
                "remote.origin.uploadpack",
                program.to_str().unwrap(),
            ],
        );
        let refused = behind(&clone).unwrap_err();
        assert_eq!(refused.kind(), crate::base::error::Kind::Refused);
        assert!(!ran.exists());
    }

    fn behind_by_one(tmp: &Path) -> std::path::PathBuf {
        let (origin, clone) = (tmp.join("origin"), tmp.join("clone"));
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "--quiet", "-b", "main"]);
        git(
            &origin,
            &["commit", "--quiet", "--allow-empty", "-m", "first"],
        );
        git(tmp, &["clone", "--quiet", "origin", "clone"]);
        git(
            &origin,
            &["commit", "--quiet", "--allow-empty", "-m", "second"],
        );
        clone
    }

    #[test]
    fn settings_reached_through_commondir_are_never_used() {
        let tmp = tempfile::tempdir().unwrap();
        let clone = behind_by_one(tmp.path());
        let (repository, common) = (clone.join(".git"), clone.join(".common"));
        std::fs::rename(&repository, &common).unwrap();
        std::fs::create_dir(&repository).unwrap();
        std::fs::copy(common.join("HEAD"), repository.join("HEAD")).unwrap();
        std::fs::copy(common.join("config"), repository.join("config")).unwrap();
        std::fs::write(repository.join("commondir"), "../.common\n").unwrap();
        let ran = tmp.path().join("ran");
        let program = format!("touch '{}'; git-upload-pack", ran.display());
        git(
            &clone,
            &[
                "config",
                "--file",
                ".common/config",
                "remote.origin.uploadpack",
                &program,
            ],
        );

        let refused = behind(&clone).unwrap_err();
        assert_eq!(refused.kind(), crate::base::error::Kind::Refused);
        assert!(update(&clone).is_err());
        assert!(!ran.exists());

        std::fs::remove_file(repository.join("commondir")).unwrap();
        std::fs::remove_dir_all(&repository).unwrap();
        std::fs::rename(&common, &repository).unwrap();
        let refused = behind(&clone).unwrap_err();
        assert!(refused.to_string().contains("remote.origin.uploadpack"));
        assert!(!ran.exists());
    }

    #[test]
    fn a_git_file_pointing_outside_the_mod_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = behind_by_one(tmp.path());
        let mod_root = tmp.path().join("Mods/stranger");
        std::fs::create_dir_all(&mod_root).unwrap();
        let pointer = format!("gitdir: {}\n", elsewhere.join(".git").display());
        std::fs::write(mod_root.join(".git"), pointer).unwrap();
        assert_eq!(
            behind(&mod_root).unwrap_err().kind(),
            crate::base::error::Kind::Refused
        );
        std::fs::write(mod_root.join(".git"), "gitdir: ../../clone/.git\n").unwrap();
        assert!(behind(&mod_root).is_err());
        assert!(update(&mod_root).is_err());

        let inside = tmp.path().join("inside");
        std::fs::rename(&elsewhere, &inside).unwrap();
        std::fs::rename(inside.join(".git"), inside.join("repository")).unwrap();
        std::fs::write(inside.join(".git"), "gitdir: repository\n").unwrap();
        assert_eq!(behind(&inside), Ok(1));
    }

    #[cfg(unix)]
    #[test]
    fn a_git_link_is_followed_only_to_a_folder_inside_the_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        git(&elsewhere, &["init", "--quiet", "-b", "main"]);
        let mod_root = tmp.path().join("Mods/stranger");
        std::fs::create_dir_all(&mod_root).unwrap();
        std::os::unix::fs::symlink(elsewhere.join(".git"), mod_root.join(".git")).unwrap();
        assert_eq!(
            in_checkout(&mod_root).unwrap_err().kind(),
            crate::base::error::Kind::Refused
        );

        let inside = tmp.path().join("Mods/own");
        std::fs::create_dir_all(&inside).unwrap();
        git(&inside, &["init", "--quiet", "-b", "main"]);
        std::fs::rename(inside.join(".git"), inside.join("repository")).unwrap();
        std::os::unix::fs::symlink("repository", inside.join(".git")).unwrap();
        assert!(in_checkout(&inside).is_ok());
    }

    #[test]
    fn a_plain_git_folder_and_a_mod_folder_that_is_itself_a_link_are_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let checkout = tmp.path().join("checkout");
        std::fs::create_dir(&checkout).unwrap();
        git(&checkout, &["init", "--quiet", "-b", "main"]);
        assert!(in_checkout(&checkout).is_ok());
        #[cfg(unix)]
        {
            let linked = tmp.path().join("linked");
            std::os::unix::fs::symlink(&checkout, &linked).unwrap();
            assert!(in_checkout(&linked).is_ok());
        }
    }

    #[test]
    #[ignore = "needs the network, git and RIMSTACK_SCRATCH: an empty folder"]
    fn a_real_repository_is_cloned_asked_and_updated() {
        let mods = std::path::PathBuf::from(std::env::var_os("RIMSTACK_SCRATCH").unwrap());
        let link = links_in("see https://github.com/pardeike/HarmonyRimWorld.git").remove(0);
        let into = clone(&link, &mods).unwrap();
        assert!(into.join("About/About.xml").is_file());
        assert_eq!(page(&into).as_deref(), Some(link.as_str()));
        assert_eq!(
            clone(&link, &mods).unwrap_err().kind(),
            crate::base::error::Kind::Refused
        );
        assert_eq!(behind(&into), Ok(0));
        let now_at = update(&into).unwrap();
        println!("cloned {} at {now_at}", into.display());
        assert!(!now_at.is_empty());
    }

    #[test]
    fn every_commit_is_listed_when_none_is_too_old() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "--quiet"]);
        std::fs::write(tmp.path().join("a"), "a").unwrap();
        git(tmp.path(), &["add", "."]);
        let identity = ["-c", "user.name=t", "-c", "user.email=t@example.org"];
        let made = std::process::Command::new("git")
            .arg("-C")
            .arg(tmp.path())
            .args(identity)
            .args(["commit", "--quiet", "-m", "Old"])
            .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
            .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
            .status()
            .unwrap();
        assert!(made.success());
        let commits = commits_since(tmp.path(), 0).unwrap();
        assert_eq!(commits, [(1_577_836_800, "Old".to_string())]);
        assert!(commits_since(tmp.path(), 1_577_836_801).unwrap().is_empty());
    }
}
