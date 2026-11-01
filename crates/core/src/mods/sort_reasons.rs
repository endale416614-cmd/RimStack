use crate::base::sentence::Sentence;
use crate::base::translate::named;
use crate::mods::list::Category;
use crate::mods::rules::{Kind, Rule};
use crate::say;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Why {
    pub group: &'static str,
    pub text: Sentence,
}

fn group(kind: Kind) -> &'static str {
    match kind {
        Kind::Requires => named("Required mods first"),
        Kind::LoadAfter | Kind::LoadBefore | Kind::ForceLoadAfter | Kind::ForceLoadBefore => {
            named("A mod's own rule")
        }
        Kind::CommunityAfter | Kind::CommunityBefore | Kind::CommunityBottom => {
            named("Community rules")
        }
        Kind::Yours => named("Your rules"),
        Kind::Inferred | Kind::Hint => named("Detected from files"),
    }
}

pub fn moved(at: usize, rules: &[Rule], names: &[&str], categories: &[Category]) -> Why {
    let own = rules.iter().filter(|r| r.first == at || r.then == at);
    let broken = own.filter(|r| r.broken() && r.kind != Kind::CommunityBottom);
    if let Some(rule) = broken.max_by_key(|r| r.kind.strength()) {
        let side = match rule.then == at {
            true => say!("after {}", names[rule.first]),
            false => say!("before {}", names[rule.then]),
        };
        let says = match (rule.kind, rule.declared_by()) {
            (Kind::Requires, Some(by)) if by == at => say!("which it requires"),
            (Kind::Requires, _) => say!("which requires it"),
            (Kind::Yours, _) => say!("by your rule"),
            (Kind::Inferred | Kind::Hint, _) => say!("as their files suggest"),
            (_, None) => say!("by a community rule"),
            (_, Some(by)) if by == at => say!("by its own rule"),
            _ => say!("by that mod's rule"),
        };
        return Why {
            group: group(rule.kind),
            text: Sentence::listed(vec![side, says]),
        };
    }
    let bottom = |r: &&Rule| r.kind == Kind::CommunityBottom && r.then == at;
    if rules.iter().find(bottom).is_some() {
        return Why {
            group: group(Kind::CommunityBottom),
            text: say!("near the bottom, by a community rule"),
        };
    }
    let mine = categories[at];
    let above = categories[..at].iter().any(|c| *c > mine);
    let below = categories[at + 1..].iter().any(|c| *c < mine);
    if above || below {
        return Why {
            group: named("Category order"),
            text: say!(
                "with the rest of {}",
                Sentence::named(mine.label()).inside()
            ),
        };
    }
    Why {
        group: named("Making room"),
        text: say!("to make room for a mod that has to move"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: [&str; 3] = ["Mod", "Harmony", "Other"];

    fn rule(first: usize, then: usize, kind: Kind) -> Rule {
        Rule { first, then, kind }
    }

    #[test]
    fn a_broken_rule_is_the_reason_and_the_strongest_one_wins() {
        let content = [Category::Content; 3];
        let rules = [
            rule(1, 0, Kind::Requires),
            rule(2, 0, Kind::Inferred),
            rule(0, 2, Kind::Yours),
        ];
        let why = moved(0, &rules, &NAMES, &content);
        assert_eq!(why.group, "Required mods first");
        assert_eq!(why.text.to_string(), "after Harmony, which it requires");
        let other = moved(1, &rules, &NAMES, &content);
        assert_eq!(other.text.to_string(), "before Mod, which requires it");
    }

    #[test]
    fn a_rule_that_holds_is_no_reason() {
        let content = [Category::Content; 3];
        let why = moved(1, &[rule(0, 1, Kind::Yours)], &NAMES, &content);
        assert_eq!(why.group, "Making room");
    }

    #[test]
    fn a_mod_among_another_category_moves_to_its_own() {
        let mixed = [Category::Content, Category::Libraries, Category::Content];
        let why = moved(1, &[], &NAMES, &mixed);
        assert_eq!(why.group, "Category order");
        assert_eq!(why.text.to_string(), "with the rest of Libraries");
    }

    #[test]
    fn a_mod_kept_at_the_bottom_says_so() {
        let content = [Category::Content; 3];
        let why = moved(0, &[rule(0, 0, Kind::CommunityBottom)], &NAMES, &content);
        assert_eq!(why.text.to_string(), "near the bottom, by a community rule");
    }
}
