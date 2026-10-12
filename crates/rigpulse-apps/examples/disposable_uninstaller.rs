//! Isolated QA helper; never bundled or used by the production application.
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4
        || args[1] != "--remove"
        || !args[2].starts_with("RigPulseDisposableFixture-")
        || !args[2]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        std::process::exit(87);
    }
    let code = args[3].parse::<i32>().unwrap_or(87);
    if code == 1602 {
        std::process::exit(code);
    }
    use windows::{Win32::System::Registry::*, core::PCWSTR};
    let path = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        args[2]
    );
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr())) };
    if result.0 != 0 {
        std::process::exit(result.0 as i32);
    }
    std::process::exit(code);
}
