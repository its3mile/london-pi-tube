// IMPORTANT: Copy this file to config.rs (in the same directory as this file) and update with your real credentials
// DO NOT commit config.rs to Git - it should be (already) in .gitignore

// WiFi credentials
pub const WIFI_SSID: &str = "your-ssid";
pub const WIFI_PASSWORD: &str = r"your-wifi-password";

// WiFi connection configuration
#[derive(Clone, Copy, Format)]
pub struct WifiConfig {
    pub ssid: &'static str,
    pub password: &'static str,
}

impl WifiConfig {
    pub fn new() -> Self {
        Self {
            ssid: WIFI_SSID,
            password: WIFI_PASSWORD,
        }
    }
}
