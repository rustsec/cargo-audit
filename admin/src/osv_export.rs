//! Backend for the `osv` subcommand.

use std::path::{Path, PathBuf};

use rustsec::{
    fs,
    osv::OsvAdvisory,
    repository::git::{GitModificationTimes, GitPath, Repository},
    Advisory, Collection,
};

use crate::{
    error::{Error, ErrorKind},
    prelude::*,
};

/// Lists all versions for a crate and prints info on which ones are affected
pub struct OsvExporter {
    /// Loaded git repository
    repository: Repository,

    /// Loaded modification times for files in Git
    mod_times: GitModificationTimes,
}

impl OsvExporter {
    /// Load the the database at the given path
    pub fn new(repo_path: impl Into<PathBuf>) -> Result<Self, Error> {
        let repo_path = repo_path.into();
        let repository = Repository::open(&repo_path)?;
        let mod_times = GitModificationTimes::new(&repository)?;
        Ok(Self {
            repository,
            mod_times,
        })
    }

    /// Exports all advisories to OSV JSON format to the specified directory.
    /// The destination directory must exist.
    pub fn export_all(&self, destination_folder: &Path) -> Result<(), Error> {
        let repo_path = self.repository.path();
        let collection_path = repo_path.join(Collection::Crates.as_str());

        if let Ok(collection_entry) = fs::read_dir(&collection_path) {
            for dir_entry in collection_entry {
                for advisory_entry in fs::read_dir(dir_entry?.path())? {
                    // TODO: fail if this isn't reached at least once

                    // Load the RustSec advisory
                    let advisory_path = advisory_entry?.path();
                    let advisory = Advisory::load_file(&advisory_path)?;
                    let id = advisory.id().clone();

                    // Transform the advisory to OSV format
                    // We've been simply pushing things to the end of the path, so in theory
                    // it *should* reverse cleanly, hence the `.unwrap()`
                    let relative_path = advisory_path.strip_prefix(repo_path).unwrap();
                    let gitpath = GitPath::new(&self.repository, relative_path)?;
                    let osv = OsvAdvisory::from_rustsec(advisory, &self.mod_times, gitpath);

                    // Serialize the OSV advisory to JSON and write it to file
                    let mut output_path: PathBuf = destination_folder.join(id.to_string());
                    output_path.set_extension("json");
                    let output_file = fs::File::create(output_path)?;
                    let writer = std::io::BufWriter::new(output_file);
                    serde_json::to_writer_pretty(writer, &osv)
                        .map_err(|err| format_err!(ErrorKind::Io, "{}", err))?
                }
            }
        }
        Ok(())
    }
}