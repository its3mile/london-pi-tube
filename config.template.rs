// IMPORTANT: Copy this file to config.rs (in the same directory as this file) and update with your real credentials
// DO NOT commit config.rs to Git - it should be (already) in .gitignore
use defmt::Format;

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

// Proxy configuration
#[derive(Clone, Copy, Format)]
pub struct ProxyConfig {
    pub http_proxy: &'static str,
}

impl ProxyConfig {
    pub fn new() -> Self {
        Self {
            http_proxy: HTTP_PROXY,
        }
    }
}

// TFL API request information
pub const API_PRIMARY_KEY: &str = "";
pub const LINE_ID: &str = "district";
pub const PLATFORM_NAME: &str = "Platform 1";
pub const STOPCODE: &str = "940GZZLUEPY";

// TFL API request configuration
#[derive(Clone, Copy, Format)]
pub struct TflApiRequestConfig {
    pub api_primary_key: &'static str,
    pub line_id: &'static str,
    pub platform_name: &'static str,
    pub stopcode: &'static str,
}

impl TflApiRequestConfig {
    pub fn new() -> Self {
        Self {
            api_primary_key: API_PRIMARY_KEY,
            line_id: LINE_ID,
            platform_name: PLATFORM_NAME,
            stopcode: STOPCODE,
        }
    }
}
