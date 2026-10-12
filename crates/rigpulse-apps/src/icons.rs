use std::path::Path;
#[derive(Debug, Clone)]
pub struct Icon {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
/// Read resources through the shell; never launch the target executable.
pub fn load(path: &Path) -> Option<Icon> {
    use windows::{
        Win32::{
            Graphics::Gdi::*,
            UI::{Shell::*, WindowsAndMessaging::*},
        },
        core::PCWSTR,
    };
    crate::native::validate_local_path(path).ok()?;
    let _guards = crate::safe_fs::guard_path(path, false).ok()?;
    unsafe {
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            PCWSTR(crate::native::wide(path).as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        ) == 0
        {
            return None;
        }
        let mut icon = ICONINFO::default();
        let result = (|| {
            GetIconInfo(info.hIcon, &mut icon).ok()?;
            if icon.hbmColor.is_invalid() {
                return None;
            }
            let mut bitmap = BITMAP::default();
            if GetObjectW(
                icon.hbmColor.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some((&mut bitmap as *mut BITMAP).cast()),
            ) == 0
            {
                return None;
            }
            let width = bitmap.bmWidth;
            let height = bitmap.bmHeight.abs();
            if !(1..=128).contains(&width) || !(1..=128).contains(&height) {
                return None;
            }
            let mut dib = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut rgba = vec![0u8; (width * height * 4) as usize];
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return None;
            }
            let rows = GetDIBits(
                dc,
                icon.hbmColor,
                0,
                height as u32,
                Some(rgba.as_mut_ptr().cast()),
                &mut dib,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(dc);
            if rows == 0 {
                return None;
            }
            let alpha = rgba.chunks_exact(4).any(|p| p[3] != 0);
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                if !alpha {
                    pixel[3] = 255;
                }
            }
            Some(Icon {
                width: width as usize,
                height: height as usize,
                rgba,
            })
        })();
        if !icon.hbmColor.is_invalid() {
            let _ = DeleteObject(icon.hbmColor.into());
        }
        if !icon.hbmMask.is_invalid() {
            let _ = DeleteObject(icon.hbmMask.into());
        }
        let _ = DestroyIcon(info.hIcon);
        result
    }
}
