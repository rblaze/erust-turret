use simplefs::DirEntry;
use simplefs::FilesystemHeader;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]

pub enum BuilderError {
    OutOfSpace,
    TooManyFiles,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileInfo {
    name: String,
    data: Vec<u8>,
}

pub struct SimpleFsBuilder {
    capacity: usize,
    write_block: usize,
    files: Vec<FileInfo>,
}

impl SimpleFsBuilder {
    pub fn new(capacity: usize, write_block: usize) -> Self {
        Self {
            capacity,
            write_block,
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, name: String, data: Vec<u8>) {
        self.files.push(FileInfo { name, data })
    }

    pub fn finalize(self) -> Result<Vec<u8>, BuilderError> {
        let num_files = self
            .files
            .len()
            .try_into()
            .map_err(|_| BuilderError::TooManyFiles)?;

        let header = FilesystemHeader {
            signature: simplefs::SIGNATURE,
            num_files,
        };

        // TODO write directory
        // TODO write files

        Ok(Self::header_as_bytes(&header))
    }

    fn header_as_bytes(header: &FilesystemHeader) -> Vec<u8> {
        header
            .signature
            .to_be_bytes()
            .iter()
            .chain(header.num_files.to_be_bytes().iter())
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::*;

    const CAPACITY: usize = 4096 * 128;
    const WRITE_BLOCK: usize = 16;

    fn header_from_bytes(bytes: &[u8]) -> Option<FilesystemHeader> {
        let signature = u64::from_be_bytes(bytes.get(0..8)?.try_into().ok()?);
        let num_files = u16::from_be_bytes(bytes.get(8..10)?.try_into().ok()?);

        Some(FilesystemHeader {
            signature,
            num_files,
        })
    }

    #[test]
    fn test_empty_fs_build() {
        let builder: SimpleFsBuilder = SimpleFsBuilder::new(CAPACITY, WRITE_BLOCK);

        let image_bytes = builder.finalize().expect("empty fs image");
        assert_eq!(image_bytes.len(), size_of::<FilesystemHeader>());

        let header = header_from_bytes(&image_bytes).expect("parsing fs header");
        let signature = header.signature;
        let num_files = header.num_files;
        assert_eq!(signature, simplefs::SIGNATURE);
        assert_eq!(num_files, 0);
    }
}
