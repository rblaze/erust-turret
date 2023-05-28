use std::mem::size_of;

use bytes::{BufMut, Bytes, BytesMut};
use simplefs::{DirEntry, FilesystemHeader};

#[derive(Debug)]
pub enum BuilderError {
    OutOfSpace,
    TooManyFiles,
    FileTooBig,
    IO(std::io::Error),
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuilderError::OutOfSpace => write!(f, "capacity exceeded"),
            BuilderError::TooManyFiles => write!(f, "too many files"),
            BuilderError::FileTooBig => write!(f, "file too big"),
            BuilderError::IO(ioerror) => write!(f, "{}", ioerror),
        }
    }
}
impl std::error::Error for BuilderError {}
impl From<std::io::Error> for BuilderError {
    fn from(ioerror: std::io::Error) -> Self {
        BuilderError::IO(ioerror)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileInfo {
    data: Vec<u8>,
}

pub struct SimpleFsBuilder {
    capacity: usize,
    files: Vec<FileInfo>,
}

impl SimpleFsBuilder {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, data: Vec<u8>) {
        self.files.push(FileInfo { data })
    }

    pub fn finalize(self) -> Result<Bytes, BuilderError> {
        let num_files = self
            .files
            .len()
            .try_into()
            .map_err(|_| BuilderError::TooManyFiles)?;

        let total_file_size: usize = self.files.iter().map(|file| file.data.len()).sum();
        let dir_size = self.files.len() * size_of::<DirEntry>();

        let mut writer =
            BytesMut::with_capacity(size_of::<FilesystemHeader>() + dir_size + total_file_size);

        FilesystemHeader {
            signature: simplefs::SIGNATURE,
            num_files,
        }
        .to_bytes(&mut writer);

        let mut current_offset = size_of::<FilesystemHeader>() + dir_size;

        for file in &self.files {
            let direntry = DirEntry {
                offset: current_offset
                    .try_into()
                    .map_err(|_| BuilderError::OutOfSpace)?,
                length: file
                    .data
                    .len()
                    .try_into()
                    .map_err(|_| BuilderError::FileTooBig)?,
            };

            current_offset += file.data.len();
            if current_offset > self.capacity {
                return Err(BuilderError::OutOfSpace);
            }

            direntry.to_bytes(&mut writer);
        }

        for file in &self.files {
            writer.put_slice(file.data.as_slice());
        }

        Ok(writer.freeze())
    }
}
