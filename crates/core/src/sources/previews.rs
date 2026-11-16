use crate::base::error::Error;
use crate::said;
use std::path::{Path, PathBuf};

const LARGEST: u64 = 4 << 20;

pub const CACHE_CAP: u64 = 200 << 20;

/// `https` only, on Steam's image hosts, with no login, port or odd characters in the host.
pub fn is_steam_image(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let plain = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    let shared = host.starts_with("shared.")
        && host.ends_with(".steamstatic.com")
        && host.len() > "shared..steamstatic.com".len();
    plain
        && (host == "images.steamusercontent.com"
            || host == "steamuserimages-a.akamaihd.net"
            || shared)
}

const LARGEST_IN_TEXT: u64 = 8 << 20;

/// `https` on any named host: no login, port, address in numbers or odd characters.
pub fn is_https_image(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let named = host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && host.chars().any(|c| c.is_ascii_alphabetic())
        && !host.ends_with(".local")
        && !host.ends_with(".internal");
    named && !host.starts_with(['.', '-']) && !host.ends_with(['.', '-'])
}

pub fn file_name(url: &str) -> String {
    let hash = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}.img")
}

/// An existing file is returned as it is. Blocks on the network otherwise.
pub fn fetch(url: &str, folder: &Path) -> Result<PathBuf, Error> {
    if !is_steam_image(url) {
        return Err(Error::refused(said!("previews are only loaded from Steam")));
    }
    get(url, folder, LARGEST)
}

pub fn fetch_in_text(url: &str, folder: &Path) -> Result<PathBuf, Error> {
    if !is_https_image(url) {
        return Err(Error::refused(said!("pictures are only loaded over https")));
    }
    get(url, folder, LARGEST_IN_TEXT)
}

fn get(url: &str, folder: &Path, largest: u64) -> Result<PathBuf, Error> {
    let path = folder.join(file_name(url));
    if path.is_file() {
        if let Ok(file) = std::fs::File::options().write(true).open(&path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        return Ok(path);
    }
    let failed = crate::base::web::steam_failed;
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    let mut response = AGENT
        .get_or_init(|| crate::base::web::agent(20))
        .get(url)
        .call()
        .map_err(crate::base::web::failed)?;
    let bytes = crate::base::web::read_bytes(&mut response, largest).map_err(failed)?;
    crate::base::atomic_file::write(&path, bytes)?;
    Ok(path)
}

/// For the fixture, which never reaches the network: a plain 16 by 9 picture in the colour the
/// address gives, written as a binary PPM.
pub fn made_up(url: &str, folder: &Path) -> Result<PathBuf, Error> {
    if !is_steam_image(url) {
        return Err(Error::refused(said!("previews are only loaded from Steam")));
    }
    painted(url, folder)
}

pub fn made_up_in_text(url: &str, folder: &Path) -> Result<PathBuf, Error> {
    if !is_https_image(url) {
        return Err(Error::refused(said!("pictures are only loaded over https")));
    }
    painted(url, folder)
}

fn painted(url: &str, folder: &Path) -> Result<PathBuf, Error> {
    let path = folder.join(file_name(url));
    if !path.is_file() {
        let hash = file_name(url).into_bytes();
        let colour = [hash[0], hash[1], hash[2]].map(|b| 80 + b % 120);
        let mut bytes = b"P6\n16 9\n255\n".to_vec();
        for _ in 0..16 * 9 {
            bytes.extend(colour);
        }
        crate::base::atomic_file::write(&path, bytes)?;
    }
    Ok(path)
}

pub fn trim(folder: &Path, cap: u64) {
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    files.sort();
    for (_, size, path) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_of_a_description_may_be_on_any_named_https_host() {
        assert!(is_https_image("https://i.imgur.com/a.png"));
        assert!(is_https_image(
            "https://images.steamusercontent.com/ugc/1/2/"
        ));
        for no in [
            "http://i.imgur.com/a.png",
            "https://127.0.0.1/a.png",
            "https://localhost/a.png",
            "https://[::1]/a.png",
            "https://user@i.imgur.com/a.png",
            "https://i.imgur.com:8080/a.png",
            "https://printer.local/a.png",
            "https://a b.example/a.png",
        ] {
            assert!(!is_https_image(no), "{no}");
        }
    }

    #[test]
    fn only_https_urls_on_steams_image_hosts_are_taken() {
        let yes = [
            "https://images.steamusercontent.com/ugc/123/ABCDEF/",
            "https://images.steamusercontent.com/ugc/1/2/?imw=268&ima=fit",
            "https://steamuserimages-a.akamaihd.net/ugc/1/2/",
            "https://shared.fastly.steamstatic.com/store_item_assets/a.jpg",
        ];
        let no = [
            "http://images.steamusercontent.com/ugc/1/2/",
            "https://images.steamusercontent.com.evil.example/ugc/1/",
            "https://evil.example/images.steamusercontent.com/",
            "https://images.steamusercontent.com@evil.example/",
            "https://images.steamusercontent.com:8080/",
            "https://shared.steamstatic.com/",
            "https://other.steamstatic.com/a.jpg",
            "file:///etc/passwd",
            "https://",
            "",
        ];
        assert!(yes.iter().all(|url| is_steam_image(url)));
        assert!(no.iter().all(|url| !is_steam_image(url)));
    }

    #[test]
    fn a_name_is_stable_safe_and_different_per_url() {
        let a = file_name("https://images.steamusercontent.com/ugc/1/2/");
        assert_eq!(a, file_name("https://images.steamusercontent.com/ugc/1/2/"));
        assert_ne!(a, file_name("https://images.steamusercontent.com/ugc/1/3/"));
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'));
    }

    #[test]
    fn a_folder_over_its_cap_loses_the_files_least_recently_used_first() {
        let dir = tempfile::tempdir().unwrap();
        let at = |name: &str, age: u64| {
            let path = dir.path().join(name);
            std::fs::write(&path, [0u8; 10]).unwrap();
            let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
            let file = std::fs::File::options().write(true).open(&path).unwrap();
            file.set_modified(when).unwrap();
        };
        at("old", 300);
        at("middle", 200);
        at("new", 100);
        trim(dir.path(), 30);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
        trim(dir.path(), 15);
        let left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["new"]);
    }

    #[test]
    fn a_file_reused_is_the_most_recently_used() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://images.steamusercontent.com/ugc/1/2/";
        let path = dir.path().join(file_name(url));
        std::fs::write(&path, b"x").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(old).unwrap();
        fetch(url, dir.path()).unwrap();
        assert!(std::fs::metadata(&path).unwrap().modified().unwrap() > old);
    }

    #[test]
    fn a_file_already_there_is_returned_and_a_foreign_url_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://images.steamusercontent.com/ugc/1/2/";
        let path = dir.path().join(file_name(url));
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(fetch(url, dir.path()).unwrap(), path);
        assert!(fetch("https://evil.example/a.png", dir.path()).is_err());
    }
}
