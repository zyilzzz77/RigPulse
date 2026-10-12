pub mod battery;
pub mod devices;
pub mod event_log;
pub mod inventory;
pub mod shell;

pub trait HardwareInfoProvider {
    fn inventory(&self) -> anyhow::Result<rigpulse_core::Inventory>;
}
pub trait BatteryInfoProvider {
    fn battery(&self) -> anyhow::Result<rigpulse_core::BatteryReading>;
}
pub struct WindowsProvider;
impl HardwareInfoProvider for WindowsProvider {
    fn inventory(&self) -> anyhow::Result<rigpulse_core::Inventory> {
        inventory::collect()
    }
}
impl BatteryInfoProvider for WindowsProvider {
    fn battery(&self) -> anyhow::Result<rigpulse_core::BatteryReading> {
        battery::read()
    }
}
mod friendly;
pub mod gpu;
