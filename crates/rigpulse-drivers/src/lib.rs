#[cfg(windows)]
mod async_wua;
pub mod install;
pub mod privilege;
pub mod wua;
pub trait DriverUpdateProvider {
    fn search(&self) -> anyhow::Result<rigpulse_core::UpdateSearch>;
}
pub struct WindowsUpdateAgent;
impl DriverUpdateProvider for WindowsUpdateAgent {
    fn search(&self) -> anyhow::Result<rigpulse_core::UpdateSearch> {
        wua::search()
    }
}
