//! Windows handle guards reject reparse points and deny rename while paths are validated.
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::{fs::File, path::Path};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub bytes: u64,
    pub volume: u32,
    pub index: u64,
    pub modified: u64,
    pub hash: [u8; 32],
}
pub(crate) fn open_directory(path: &Path) -> anyhow::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::*;
    Ok(std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?)
}
pub(crate) fn volume(file: &mut File) -> anyhow::Result<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)?;
    }
    Ok(info.dwVolumeSerialNumber)
}
pub(crate) fn open_file(path: &Path, modify: bool) -> anyhow::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows::Win32::Storage::FileSystem::*;
    let file = std::fs::OpenOptions::new()
        .access_mode(0x80000000 | if modify { DELETE.0 } else { 0 })
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0,
        "Hanya file biasa yang didukung"
    );
    Ok(file)
}
pub(crate) fn snapshot(file: &mut File) -> anyhow::Result<Snapshot> {
    use sha2::{Digest, Sha256};
    use std::{
        io::{Read, Seek, SeekFrom},
        os::windows::{fs::MetadataExt, io::AsRawHandle},
    };
    use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
    let metadata = file.metadata()?;
    ensure!(
        metadata.len() <= 64 * 1024 * 1024,
        "File melebihi batas 64 MiB"
    );
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)?;
    }
    ensure!(info.nNumberOfLinks == 1, "Hard link ditolak");
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    let mut total = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(total <= 64 * 1024 * 1024, "Batas baca tercapai");
        hash.update(&buf[..n]);
    }
    ensure!(
        total == metadata.len() && file.metadata()?.last_write_time() == metadata.last_write_time(),
        "File berubah saat dibaca"
    );
    Ok(Snapshot {
        bytes: total,
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        modified: metadata.last_write_time(),
        hash: hash.finalize().into(),
    })
}
pub(crate) fn rename(file: &File, destination: &Path) -> anyhow::Result<()> {
    rename_inner(file, destination, false)
}
pub(crate) fn rename_replace(file: &File, destination: &Path) -> anyhow::Result<()> {
    rename_inner(file, destination, true)
}
fn rename_inner(file: &File, destination: &Path, replace: bool) -> anyhow::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
    crate::native::validate_local_path(destination)?;
    let path = crate::native::wide(destination);
    let bytes = (path.len() - 1) * 2;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let size = offset + bytes + 2;
    let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*info).FileNameLength = bytes as u32;
        (*info).Anonymous.ReplaceIfExists = replace;
        std::ptr::copy_nonoverlapping(
            path.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            path.len(),
        );
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileRenameInfo,
            buffer.as_ptr().cast(),
            size as u32,
        )?;
    }
    Ok(())
}
pub(crate) fn delete(file: &File) -> anyhow::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )?;
    }
    Ok(())
}
#[cfg(windows)]
pub(crate) fn guard_path(path: &Path, allow_missing_leaf: bool) -> anyhow::Result<Vec<File>> {
    use std::os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    };
    use windows::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
    crate::native::validate_local_path(path)?;
    let mut guards = Vec::new();
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if current.parent().is_none() || current.to_string_lossy().len() <= 3 {
            continue;
        }
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES.0)
            .share_mode(FILE_SHARE_READ.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(&current);
        let file = match file {
            Ok(f) => f,
            Err(e)
                if allow_missing_leaf
                    && current == path
                    && e.kind() == std::io::ErrorKind::NotFound =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };
        ensure!(
            file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0,
            "Junction/symlink/reparse point ditolak"
        );
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe {
            GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)?;
        }
        ensure!(
            info.nNumberOfLinks <= 1 || file.metadata()?.is_dir(),
            "Hard link ditolak"
        );
        guards.push(file);
    }
    Ok(guards)
}
