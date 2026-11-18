//! A trail of what the list of records told and of the row each record was taken for: a text
//! file beside the database (`list-trail.log`), to find out afterwards why a row was told as
//! it was. Rows, battles and players are written as their numbers: no name goes into it.

use std::{fs::File, io::Write, path::Path};

/// The trail is started anew once it has grown this large.
const FULL: u64 = 512 * 1024;

pub struct Trail {
    file: Option<File>,
    last: String,
}

impl Trail {
    /// The trail in the folder `dir`. One that cannot be written is left unwritten: it is
    /// only an aid.
    pub fn open(dir: &Path) -> Self {
        let path = dir.join("list-trail.log");
        let full = std::fs::metadata(&path).is_ok_and(|file| file.len() > FULL);
        let file = if full {
            File::create(path)
        } else {
            File::options().create(true).append(true).open(path)
        };
        Self {
            file: file.ok(),
            last: String::new(),
        }
    }

    /// Writes what happened at `clock` (seconds since 1970), unless the line before said
    /// the same.
    pub fn note(&mut self, clock: i64, what: String) {
        if what != self.last
            && let Some(file) = &mut self.file
        {
            let _ = writeln!(file, "{clock} {what}");
            self.last = what;
        }
    }
}
