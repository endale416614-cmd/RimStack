//! Nothing is executed. The layout is ECMA-335 partition II: sections 22,
//! 23.2, 23.3, 24 and 25.4.

mod calls;
mod harmony;
mod il;
mod tables;

pub use calls::{Base, Callee, How, Layout, MethodDef, Step, TypeDef};
pub use harmony::{Does, Patch};
use tables::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub name: String,
    pub extends: Option<String>,
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    /// Unity's immediate-mode GUI takes a `Rect`: a method given one draws.
    pub takes_rect: bool,
}

pub struct Assembly<'a> {
    dll: &'a [u8],
    strings: &'a [u8],
    blobs: &'a [u8],
    /// UTF-16 in the `#US` heap.
    user_strings: &'a [u8],
    rows: [usize; 64],
    starts: [usize; TABLES.len()],
    row_bytes: [usize; TABLES.len()],
    columns: [[(usize, usize); WIDEST]; TABLES.len()],
    sections: Vec<(usize, usize, usize)>,
    wide: (usize, usize, usize),
}

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// ECMA-335 II.23.2 compressed integer, as `(value, bytes used)`.
fn compressed(bytes: &[u8]) -> Option<(usize, usize)> {
    let first = *bytes.first()? as usize;
    if first & 0x80 == 0 {
        Some((first, 1))
    } else if first & 0xc0 == 0x80 {
        Some(((first & 0x3f) << 8 | *bytes.get(1)? as usize, 2))
    } else {
        let rest = bytes.get(1..4)?;
        let low = (rest[0] as usize) << 16 | (rest[1] as usize) << 8 | rest[2] as usize;
        Some(((first & 0x1f) << 24 | low, 4))
    }
}

fn in_file(sections: &[(usize, usize, usize)], rva: usize) -> Option<usize> {
    let inside = |(start, size, _): &&(usize, usize, usize)| rva >= *start && rva < start + size;
    sections.iter().find(inside).map(|s| rva - s.0 + s.2)
}

impl<'a> Assembly<'a> {
    /// `None` for a file that is not a .NET assembly, or is damaged.
    pub fn read(dll: &'a [u8]) -> Option<Self> {
        let pe = u32_at(dll, 0x3c)?;
        if dll.get(pe..pe + 4)? != b"PE\0\0" {
            return None;
        }
        let sections = u16_at(dll, pe + 6)?;
        let optional = pe + 24;
        let directories = match u16_at(dll, optional)? {
            0x10b => optional + 96,
            0x20b => optional + 112,
            _ => return None,
        };
        let section_table = optional + u16_at(dll, pe + 20)?;
        let sections: Vec<(usize, usize, usize)> = (0..sections)
            .filter_map(|i| {
                let s = section_table + i * 40;
                let size = u32_at(dll, s + 8)?.max(1);
                Some((u32_at(dll, s + 12)?, size, u32_at(dll, s + 20)?))
            })
            .collect();
        let to_file = |rva: usize| in_file(&sections, rva);
        // The .NET header is the fifteenth directory; it points at the metadata.
        let cli = to_file(u32_at(dll, directories + 14 * 8)?)?;
        let metadata = to_file(u32_at(dll, cli + 8)?)?;
        if u32_at(dll, metadata)? != 0x424a_5342 {
            return None;
        }
        let mut at = metadata + 16 + u32_at(dll, metadata + 12)?;
        let streams = u16_at(dll, at + 2)?;
        at += 4;
        let (mut tables, mut strings, mut blobs, mut user_strings) = (None, None, None, None);
        for _ in 0..streams {
            let (offset, size) = (u32_at(dll, at)?, u32_at(dll, at + 4)?);
            let name_end = dll.get(at + 8..)?.iter().position(|b| *b == 0)?;
            let stream = || dll.get(metadata + offset..metadata + offset + size);
            match dll.get(at + 8..at + 8 + name_end)? {
                b"#~" | b"#-" => tables = Some(metadata + offset),
                b"#Strings" => strings = stream(),
                b"#Blob" => blobs = stream(),
                b"#US" => user_strings = stream(),
                _ => {}
            }
            // Names are padded to a multiple of four bytes, with their ending zero.
            at += 8 + (name_end + 4) / 4 * 4;
        }
        let tables = tables?;

        let heap_sizes = *dll.get(tables + 6)?;
        let present = u64::from_le_bytes(dll.get(tables + 8..tables + 16)?.try_into().ok()?);
        let mut rows = [0usize; 64];
        let mut at = tables + 24;
        for (table, count) in rows.iter_mut().enumerate() {
            if present >> table & 1 == 1 {
                *count = u32_at(dll, at)?;
                at += 4;
            }
        }
        let wide = (
            if heap_sizes & 1 != 0 { 4 } else { 2 },
            if heap_sizes & 2 != 0 { 4 } else { 2 },
            if heap_sizes & 4 != 0 { 4 } else { 2 },
        );
        let mut assembly = Self {
            dll,
            strings: strings?,
            // An assembly with no blobs has no such heap.
            blobs: blobs.unwrap_or_default(),
            user_strings: user_strings.unwrap_or_default(),
            rows,
            starts: [0; TABLES.len()],
            row_bytes: [0; TABLES.len()],
            columns: [[(0, 0); WIDEST]; TABLES.len()],
            sections,
            wide,
        };
        for (table, columns) in TABLES.iter().enumerate() {
            let mut bytes = 0;
            for (i, column) in columns.iter().enumerate() {
                let width = assembly.width(*column);
                assembly.columns[table][i] = (bytes, width);
                bytes += width;
            }
            assembly.row_bytes[table] = bytes;
            assembly.starts[table] = at;
            at += bytes * rows[table];
        }
        dll.get(..at)?;
        // A type's methods run up to the next type's first, so the firsts must never
        // go back: a file where they do would make each reading quadratic.
        let mut last = 0;
        for row in 1..=rows[TYPE_DEF] {
            let first = assembly.cell(TYPE_DEF, row, 5)?;
            if first < last {
                return None;
            }
            last = first;
        }
        Some(assembly)
    }

    fn to_file(&self, rva: usize) -> Option<usize> {
        in_file(&self.sections, rva)
    }

    fn width(&self, col: Col) -> usize {
        let index = |most: usize, bits: u32| if most < 1 << (16 - bits) { 2 } else { 4 };
        match col {
            Col::Fixed(bytes) => bytes,
            Col::Str => self.wide.0,
            Col::Guid => self.wide.1,
            Col::Blob => self.wide.2,
            Col::Row(table) => index(self.rows[table], 0),
            Col::Coded(tables, bits) => index(
                tables.iter().map(|t| self.rows[*t]).max().unwrap_or(0),
                bits,
            ),
        }
    }

    /// Rows count from 1. Columns of more than four bytes read as their first four.
    fn cell(&self, table: usize, row: usize, column: usize) -> Option<usize> {
        if row == 0 || row > self.rows[table] {
            return None;
        }
        let (before, width) = self.columns[table][column];
        let at = self.starts[table] + (row - 1) * self.row_bytes[table] + before;
        match width {
            2 => u16_at(self.dll, at),
            _ => u32_at(self.dll, at),
        }
    }

    fn coded(&self, table: usize, row: usize, column: usize) -> Option<(usize, usize)> {
        let Col::Coded(tables, bits) = TABLES[table][column] else {
            return None;
        };
        let value = self.cell(table, row, column)?;
        let tag = value & ((1 << bits) - 1);
        // CustomAttributeType numbers its two tables 2 and 3.
        let tag = match (table, column) {
            (CUSTOM_ATTRIBUTE, 1) => tag.checked_sub(2)?,
            _ => tag,
        };
        Some((*tables.get(tag)?, value >> bits))
    }

    fn string(&self, index: usize) -> Option<&'a str> {
        let text = self.strings.get(index..)?;
        let end = text.iter().position(|c| *c == 0)?;
        std::str::from_utf8(&text[..end]).ok()
    }

    fn name(&self, table: usize, row: usize, column: usize) -> Option<&'a str> {
        self.string(self.cell(table, row, column)?)
    }

    fn blob(&self, table: usize, row: usize, column: usize) -> Option<&'a [u8]> {
        let bytes = self.blobs.get(self.cell(table, row, column)?..)?;
        let (length, used) = compressed(bytes)?;
        bytes.get(used..used + length)
    }

    fn full_name(&self, table: usize, row: usize, name_column: usize) -> Option<String> {
        let name = self.name(table, row, name_column)?;
        let namespace = self.name(table, row, name_column + 1)?;
        Some(match namespace {
            "" => name.to_string(),
            _ => format!("{namespace}.{name}"),
        })
    }

    fn enclosing(&self, type_row: usize) -> Option<usize> {
        // Sorted by the nested type, so this is a binary search.
        let (mut low, mut high) = (1, self.rows[NESTED_CLASS] + 1);
        while low < high {
            let middle = (low + high) / 2;
            match self.cell(NESTED_CLASS, middle, 0)?.cmp(&type_row) {
                std::cmp::Ordering::Equal => return self.cell(NESTED_CLASS, middle, 1),
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
            }
        }
        None
    }

    /// Nested types as `Namespace.Outer+Inner`; `None` for a constructed type.
    fn type_name(&self, of: (usize, usize)) -> Option<String> {
        let outer = |(table, row): (usize, usize)| match table {
            TYPE_DEF => self.enclosing(row).map(|outer| (TYPE_DEF, outer)),
            _ => self.coded(TYPE_REF, row, 0).filter(|to| to.0 == TYPE_REF),
        };
        if !matches!(of.0, TYPE_DEF | TYPE_REF) {
            return None;
        }
        let mut name = self.full_name(of.0, of.1, 1)?;
        // A damaged file may make nesting loop, so the walk is bounded.
        let mut inner = of;
        for _ in 0..16 {
            let Some(around) = outer(inner) else { break };
            name = format!("{}+{name}", self.full_name(around.0, around.1, 1)?);
            inner = around;
        }
        Some(name)
    }

    pub fn references(&self) -> Vec<String> {
        (1..=self.rows[ASSEMBLY_REF])
            .filter_map(|row| self.name(ASSEMBLY_REF, row, 3))
            .map(String::from)
            .collect()
    }

    fn methods_of(&self, type_row: usize) -> std::ops::Range<usize> {
        // A damaged list starting at row 0 is taken from row 1.
        let first = self.cell(TYPE_DEF, type_row, 5).unwrap_or(1).max(1);
        let end = match type_row < self.rows[TYPE_DEF] {
            true => self.cell(TYPE_DEF, type_row + 1, 5).unwrap_or(first),
            false => self.rows[METHOD_DEF] + 1,
        };
        first..end.max(first).min(self.rows[METHOD_DEF] + 1)
    }

    /// ELEMENT_TYPE_VALUETYPE, then the coded `TypeRef` of `UnityEngine.Rect`.
    fn rect_marker(&self) -> Option<Vec<u8>> {
        let row = (1..=self.rows[TYPE_REF])
            .find(|&row| self.full_name(TYPE_REF, row, 1).as_deref() == Some("UnityEngine.Rect"))?;
        let coded = row << 2 | 1;
        let mut marker = vec![0x11];
        match coded {
            0..=0x7f => marker.push(coded as u8),
            0x80..=0x3fff => marker.extend([(coded >> 8) as u8 | 0x80, coded as u8]),
            _ => marker.extend((coded as u32 | 0xc000_0000).to_be_bytes()),
        }
        Some(marker)
    }

    /// Names with `<` (compiler-made types) are left out.
    pub fn type_names(&self) -> Vec<String> {
        (1..=self.rows[TYPE_DEF])
            .filter_map(|row| self.type_name((TYPE_DEF, row)))
            .filter(|name| !name.contains('<'))
            .collect()
    }

    /// Names with `<` (compiler-made types) are left out.
    pub fn types(&self) -> Vec<Type> {
        let rect = self.rect_marker();
        let takes_rect = |row: usize| {
            let (Some(rect), Some(signature)) = (&rect, self.blob(METHOD_DEF, row, 4)) else {
                return false;
            };
            signature.windows(rect.len()).any(|w| w == rect.as_slice())
        };
        (1..=self.rows[TYPE_DEF])
            .filter_map(|row| {
                let name = self.type_name((TYPE_DEF, row))?;
                if name.contains('<') {
                    return None;
                }
                let extends = self
                    .coded(TYPE_DEF, row, 3)
                    .and_then(|to| self.type_name(to));
                let methods = self
                    .methods_of(row)
                    .filter_map(|m| {
                        Some(Method {
                            name: self.name(METHOD_DEF, m, 3)?.to_string(),
                            takes_rect: takes_rect(m),
                        })
                    })
                    .collect();
                Some(Type {
                    name,
                    extends,
                    methods,
                })
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod build;

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod tests {
    use super::build::Builder;
    use super::*;

    #[test]
    fn a_file_that_is_not_an_assembly_is_refused() {
        assert!(Assembly::read(b"MZ not really").is_none());
        assert!(Assembly::read(&[0u8; 4096]).is_none());
        assert!(Assembly::read(&[]).is_none());
    }

    #[test]
    fn an_empty_assembly_has_nothing_in_it() {
        let dll = Builder::new().finish();
        let assembly = Assembly::read(&dll).unwrap();
        assert!(assembly.references().is_empty());
        assert!(assembly.types().is_empty());
        assert!(assembly.patches().is_empty());
        assert!(assembly.looked_up().is_empty());
    }

    #[test]
    fn method_lists_that_go_back_are_a_damaged_file() {
        let mut b = Builder::new();
        b.type_def("My", "First", 0);
        b.method("One", &[0x00, 0, 0x01]);
        b.method("Two", &[0x00, 0, 0x01]);
        b.type_def("My", "Second", 0);
        let name = b.string("Third");
        b.row(TYPE_DEF, &[0, name, 0, 0, 1, 1]);
        assert!(Assembly::read(&b.finish()).is_none());
    }

    #[test]
    fn references_are_listed_by_name() {
        let mut b = Builder::new();
        b.reference("Assembly-CSharp");
        b.reference("0Harmony");
        let dll = b.finish();
        assert_eq!(
            Assembly::read(&dll).unwrap().references(),
            ["Assembly-CSharp", "0Harmony"]
        );
    }

    #[test]
    fn types_come_with_what_they_extend_and_their_methods() {
        let mut b = Builder::new();
        let window = b.type_ref("Verse", "Window");
        let rect = b.type_ref("UnityEngine", "Rect");
        b.type_def("", "<Module>", 0);
        b.type_def("My", "Dialog", window);
        // void DoWindowContents(Rect): instance, one parameter, a value type.
        b.method(
            "DoWindowContents",
            &[0x20, 1, 0x01, 0x11, (rect << 2 | 1) as u8],
        );
        b.method("Close", &[0x20, 0, 0x01]);
        b.type_def("My", "Helper", 0);
        b.method("Count", &[0x00, 1, 0x08, 0x08]);
        let dll = b.finish();
        let types = Assembly::read(&dll).unwrap().types();
        assert_eq!(
            Assembly::read(&dll).unwrap().type_names(),
            ["My.Dialog", "My.Helper"]
        );
        assert_eq!(types[0].name, "My.Dialog");
        assert_eq!(types[0].extends.as_deref(), Some("Verse.Window"));
        let drawn: Vec<(&str, bool)> = types[0]
            .methods
            .iter()
            .map(|m| (m.name.as_str(), m.takes_rect))
            .collect();
        assert_eq!(drawn, [("DoWindowContents", true), ("Close", false)]);
        assert_eq!(types[1].extends, None);
        assert_eq!(types[1].methods.len(), 1);
        assert!(!types[1].methods[0].takes_rect);
    }
}
