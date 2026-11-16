use super::types::FileTypes;
use crate::base::locks::Held;
use crate::base::stamp::FolderStamp;
use crate::base::walk;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FolderSettings {
    pub types: FileTypes,
    pub languages: bool,
    pub source: bool,
    pub textures: bool,
    pub git: bool,
}

impl Default for FolderSettings {
    fn default() -> Self {
        Self {
            types: FileTypes::default(),
            languages: true,
            source: true,
            textures: true,
            git: true,
        }
    }
}

impl FolderSettings {
    fn leaves_out(&self, name: &OsStr) -> bool {
        let named = |left: &str| name.eq_ignore_ascii_case(left);
        (self.languages && named("Languages"))
            || (self.source && named("Source"))
            || (self.textures && named("Textures"))
            || (self.git && named(".git"))
    }

    fn walk(&self, root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        walk::visit(
            root,
            |name| self.leaves_out(name),
            |seen| {
                if let walk::Seen::Item(path, kind) = seen
                    && (!kind.is_symlink() || path.is_file())
                {
                    found.push(path.to_path_buf());
                }
                ControlFlow::Continue(())
            },
        );
        found.sort_unstable();
        found.dedup();
        found
    }
}

type Kept = (FolderStamp, Arc<Vec<PathBuf>>);
type Key = (PathBuf, [bool; 4]);

#[derive(Default)]
pub struct FileLists {
    kept: Mutex<HashMap<Key, Kept>>,
}

impl FileLists {
    pub(super) fn all(&self, folder: &Path, settings: &FolderSettings) -> Arc<Vec<PathBuf>> {
        let stamp = FolderStamp::of(folder, |name| settings.leaves_out(name));
        let key = (
            folder.to_path_buf(),
            [
                settings.languages,
                settings.source,
                settings.textures,
                settings.git,
            ],
        );
        if let Some((kept, list)) = self.kept.held().get(&key)
            && *kept == stamp
        {
            return list.clone();
        }
        let list = Arc::new(settings.walk(folder));
        self.kept.held().insert(key, (stamp, list.clone()));
        list
    }

    pub fn get(&self, folder: &Path, settings: &FolderSettings) -> Vec<PathBuf> {
        let all = self.all(folder, settings);
        match &settings.types {
            FileTypes::Any => all.to_vec(),
            types => all.iter().filter(|f| types.takes(f)).cloned().collect(),
        }
    }

    /// Most common first; the same count by name.
    pub fn extensions(
        &self,
        folders: &[PathBuf],
        leave_out: &FolderSettings,
    ) -> Vec<(String, usize)> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for folder in folders {
            for file in self.all(folder, leave_out).iter() {
                let Some(x) = file.extension() else {
                    continue;
                };
                let name = x.to_string_lossy();
                match counts.get_mut(&*name) {
                    Some(n) => *n += 1,
                    None => *counts.entry(name.to_lowercase()).or_default() += 1,
                }
            }
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }
}
