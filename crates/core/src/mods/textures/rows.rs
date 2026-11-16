use super::Summary;
use crate::base::translate::named;
use crate::mods::library::ModId;
use std::path::Path;

/// The English word is the key the interface translates into the chosen language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Skipped,
    OutOfDate,
    NotConverted,
    PartlyConverted,
    AlreadyHasDds,
    Converted,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Skipped => named("Skipped"),
            State::OutOfDate => named("Out of date"),
            State::NotConverted => named("Not converted"),
            State::PartlyConverted => named("Partly converted"),
            State::AlreadyHasDds => named("Already has DDS"),
            State::Converted => named("Converted"),
        }
    }
}

impl Summary {
    pub fn state(&self, skipped: bool) -> State {
        if skipped {
            State::Skipped
        } else if self.out_of_date > 0 {
            State::OutOfDate
        } else if self.not_converted() == self.textures {
            State::NotConverted
        } else if self.not_converted() > 0 {
            State::PartlyConverted
        } else if self.converted == 0 {
            State::AlreadyHasDds
        } else {
            State::Converted
        }
    }

    pub fn waits(&self, skipped: bool, converts: bool) -> bool {
        converts && !skipped && (self.out_of_date > 0 || self.not_converted() > 0)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: ModId,
    pub waits: bool,
    pub name: String,
    pub active: bool,
    pub textures: usize,
    pub converted: usize,
    pub out_of_date: usize,
    pub shipped: usize,
    pub not_converted: usize,
    pub state: &'static str,
    pub complete: bool,
    pub stale: bool,
    pub skipped: bool,
    pub png_mb: f64,
    pub dds_mb: f64,
    pub made_mb: f64,
    pub path: String,
}

impl Row {
    /// Nothing for a mod without textures.
    pub fn of(
        id: ModId,
        name: &str,
        path: &Path,
        counted: Summary,
        skipped: bool,
        active: bool,
        converts: bool,
    ) -> Option<Self> {
        let megabytes = |bytes: u64| bytes as f64 / 1_048_576.0;
        let state = counted.state(skipped);
        (counted.textures > 0).then(|| Self {
            id,
            waits: counted.waits(skipped, converts),
            name: name.to_string(),
            active,
            textures: counted.textures,
            converted: counted.converted,
            out_of_date: counted.out_of_date,
            shipped: counted.shipped,
            not_converted: counted.not_converted(),
            state: state.label(),
            complete: state == State::Converted,
            stale: state == State::OutOfDate,
            skipped,
            png_mb: megabytes(counted.png_bytes),
            dds_mb: megabytes(counted.dds_bytes),
            made_mb: megabytes(counted.made_bytes),
            path: path.to_string_lossy().into_owned(),
        })
    }
}

pub fn in_order(mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort_by_cached_key(|row| (!row.waits, row.name.to_lowercase()));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mod_s_state_is_the_first_thing_that_is_true_of_its_textures() {
        let counted = |textures, converted, out_of_date, shipped| Summary {
            textures,
            converted,
            out_of_date,
            shipped,
            ..Summary::default()
        };
        let state = |s: Summary| (s.state(false), s.waits(false, true));
        assert_eq!(state(counted(4, 0, 0, 0)), (State::NotConverted, true));
        assert_eq!(state(counted(4, 1, 0, 1)), (State::PartlyConverted, true));
        assert_eq!(state(counted(4, 3, 1, 0)), (State::OutOfDate, true));
        assert_eq!(state(counted(4, 0, 0, 4)), (State::AlreadyHasDds, false));
        assert_eq!(state(counted(4, 3, 0, 1)), (State::Converted, false));
        let skipped = counted(4, 0, 2, 0);
        assert_eq!(
            (skipped.state(true), skipped.waits(true, true)),
            (State::Skipped, false)
        );
        assert!(
            !counted(4, 0, 1, 0).waits(false, false),
            "nothing waits from 1.7"
        );
    }

    #[test]
    fn rows_put_what_waits_first_and_leave_out_mods_without_textures() {
        let counted = |textures, converted| Summary {
            textures,
            converted,
            png_bytes: 2_097_152,
            ..Summary::default()
        };
        let row = |at, name, s| {
            Row::of(
                ModId::new(0, at),
                name,
                Path::new("/mods/x"),
                s,
                false,
                at == 1,
                true,
            )
        };
        assert_eq!(row(0, "None", counted(0, 0)), None);
        let rows = in_order(
            [
                row(1, "zeta", counted(2, 2)),
                row(2, "Beta", counted(2, 2)),
                row(3, "omega", counted(2, 0)),
            ]
            .into_iter()
            .flatten()
            .collect(),
        );
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["omega", "Beta", "zeta"]);
        let said = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(said["state"], "Not converted");
        assert_eq!(
            (said["notConverted"].as_u64(), said["pngMb"].as_f64()),
            (Some(2), Some(2.0))
        );
        assert_eq!(said["active"], false);
    }
}
