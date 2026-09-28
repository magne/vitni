//! The zip container of a `.vitni-backup`: writing members with their checksums, and reading them
//! back (ADR 0041 §1).
//!
//! Every member is deflated with a fixed timestamp and mode, so two backups of one log are
//! byte-identical: the golden fixtures are reproducible, and a diff in one means the data changed.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::backup::error::BackupError;

/// The `sha256:<lowercase hex>` form of a finished digest, the form media checksums use too.
pub(crate) fn checksum_text(hasher: Sha256) -> String {
    let mut text = String::from("sha256:");
    for byte in hasher.finalize() {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Maps an I/O failure on `what` to [`BackupError::Archive`].
pub(crate) fn io_error(what: &str, error: &impl std::fmt::Display) -> BackupError {
    BackupError::Archive(format!("{what}: {error}"))
}

/// The options every member is written with: deflated, dated 1980-01-01, mode 0644.
fn member_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

/// Writes members one at a time, recording each one's checksum for the manifest.
pub(crate) struct ArchiveWriter {
    zip: ZipWriter<BufWriter<File>>,
    members: BTreeMap<String, String>,
    current: Option<(String, Sha256)>,
}

impl ArchiveWriter {
    /// Starts an archive in `file`.
    pub(crate) fn new(file: File) -> Self {
        Self {
            zip: ZipWriter::new(BufWriter::new(file)),
            members: BTreeMap::new(),
            current: None,
        }
    }

    /// Starts member `name`, finishing the previous one.
    pub(crate) fn start(&mut self, name: &str) -> Result<(), BackupError> {
        self.finish_member();
        self.zip
            .start_file(name, member_options())
            .map_err(|e| io_error(name, &e))?;
        self.current = Some((name.to_owned(), Sha256::new()));
        Ok(())
    }

    /// Appends `bytes` to the current member.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), BackupError> {
        let Some((name, hasher)) = &mut self.current else {
            unreachable!("a member is started before it is written");
        };
        hasher.update(bytes);
        self.zip.write_all(bytes).map_err(|e| io_error(name, &e))
    }

    /// Writes the whole member `name` from `bytes`.
    pub(crate) fn member(&mut self, name: &str, bytes: &[u8]) -> Result<(), BackupError> {
        self.start(name)?;
        self.write(bytes)
    }

    /// Writes member `name` from the file at `path`, streamed.
    pub(crate) fn file(&mut self, name: &str, path: &Path) -> Result<(), BackupError> {
        let mut source = File::open(path).map_err(|e| io_error(&path.display().to_string(), &e))?;
        self.start(name)?;
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|e| io_error(&path.display().to_string(), &e))?;
            if read == 0 {
                return Ok(());
            }
            self.write(&buffer[..read])?;
        }
    }

    /// The checksums of every member written so far, finishing the current one.
    pub(crate) fn checksums(&mut self) -> &BTreeMap<String, String> {
        self.finish_member();
        &self.members
    }

    /// Writes `manifest` as the last member, unhashed, and closes the archive.
    pub(crate) fn finish(mut self, manifest_name: &str, manifest: &[u8]) -> Result<(), BackupError> {
        self.member(manifest_name, manifest)?;
        let writer = self.zip.finish().map_err(|e| io_error("closing the archive", &e))?;
        let file = writer
            .into_inner()
            .map_err(|e| io_error("flushing the archive", &e.error()))?;
        file.sync_all().map_err(|e| io_error("syncing the archive", &e))
    }

    /// Records the current member's checksum.
    fn finish_member(&mut self) {
        if let Some((name, hasher)) = self.current.take() {
            self.members.insert(name, checksum_text(hasher));
        }
    }
}

/// Reads members out of an archive.
pub(crate) struct ArchiveReader {
    zip: ZipArchive<File>,
}

impl ArchiveReader {
    /// Opens the archive at `path`.
    ///
    /// # Errors
    ///
    /// [`BackupError::Archive`] if the file cannot be opened, [`BackupError::NotABackup`] if it is
    /// not a zip.
    pub(crate) fn open(path: &Path) -> Result<Self, BackupError> {
        let file = File::open(path).map_err(|e| io_error(&path.display().to_string(), &e))?;
        let zip = ZipArchive::new(file).map_err(|e| BackupError::NotABackup(e.to_string()))?;
        Ok(Self { zip })
    }

    /// Every member name, in archive order.
    pub(crate) fn names(&self) -> Vec<String> {
        self.zip.file_names().map(str::to_owned).collect()
    }

    /// Whether the archive holds member `name`.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.zip.index_for_name(name).is_some()
    }

    /// Opens member `name` for streaming.
    pub(crate) fn open_member(&mut self, name: &str) -> Result<impl Read + '_, BackupError> {
        match self.zip.by_name(name) {
            Ok(file) => Ok(file),
            Err(zip::result::ZipError::FileNotFound) => Err(BackupError::MissingMember(name.to_owned())),
            Err(error) => Err(io_error(name, &error)),
        }
    }

    /// Reads the whole of member `name`.
    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>, BackupError> {
        let mut bytes = Vec::new();
        self.open_member(name)?
            .read_to_end(&mut bytes)
            .map_err(|e| io_error(name, &e))?;
        Ok(bytes)
    }

    /// The checksum of member `name`'s content, streamed.
    pub(crate) fn checksum(&mut self, name: &str) -> Result<String, BackupError> {
        let mut reader = HashingReader::new(self.open_member(name)?);
        io::copy(&mut reader, &mut io::sink()).map_err(|e| io_error(name, &e))?;
        Ok(reader.checksum())
    }
}

/// A reader that hashes what passes through it, so one pass both parses and verifies a member.
pub(crate) struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
}

impl<R: Read> HashingReader<R> {
    /// Wraps `inner`.
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    /// The checksum of everything read.
    pub(crate) fn checksum(self) -> String {
        checksum_text(self.hasher)
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..read]);
        Ok(read)
    }
}
