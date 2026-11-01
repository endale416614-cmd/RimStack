use crate::mods::list::{Category, Mod};

const FIRST: [&str; 12] = [
    "Vanilla", "Rim", "Alpha", "Advanced", "Simple", "Better", "Expanded", "Tribal", "Royal",
    "Outland", "Glitter", "Medieval",
];
const SECOND: [&str; 12] = [
    "Furniture",
    "Weapons",
    "Animals",
    "Hygiene",
    "Textures",
    "Factions",
    "Armour",
    "Cooking",
    "Storage",
    "Power",
    "Traits",
    "Biomes",
];
const AUTHORS: [&str; 7] = [
    "Oskar Potocki",
    "Dubwise",
    "Sarg Bjornson",
    "Jaxe",
    "Fluffy",
    "Mlie",
    "Taranchuk",
];

const ENDS: [(Category, usize); 7] = [
    (Category::Loaders, 1),
    (Category::Game, 2),
    (Category::Libraries, 10),
    (Category::Content, 70),
    (Category::Patches, 85),
    (Category::Textures, 95),
    (Category::Interface, 100),
];

pub fn generate(count: usize) -> Vec<Mod> {
    (0..count)
        .map(|i| {
            let percent = i * 100 / count;
            let category = ENDS
                .iter()
                .find(|(_, end)| percent < *end)
                .map_or(Category::Interface, |(c, _)| *c);
            Mod {
                id: String::new(),
                tags: Vec::new(),
                local: false,
                workshop: false,
                git: false,
                code: false,
                patched: Default::default(),
                sub: String::new(),
                load_micros: None,
                filed: category,
                chosen: false,
                held_by: String::new(),
                name: format!(
                    "{} {} {}",
                    FIRST[i % FIRST.len()],
                    SECOND[(i / FIRST.len()) % SECOND.len()],
                    i + 1
                ),
                author: AUTHORS[i % AUTHORS.len()].to_string(),
                version: format!("1.{}.{}", i % 7, i % 13),
                category,
                installed: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::list::{Category, GroupedList, Row};

    #[test]
    fn categories_are_contiguous_and_in_order() {
        let mods = generate(2000);
        assert!(mods.windows(2).all(|w| w[0].category <= w[1].category));
        let list = GroupedList::new(mods);
        let headers = list
            .rows()
            .iter()
            .filter(|r| matches!(r, Row::Header { .. }))
            .count();
        assert_eq!(headers, Category::ALL.len() - 2);
    }

    #[test]
    fn names_are_distinct() {
        let mods = generate(2000);
        let mut names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 2000);
    }
}
