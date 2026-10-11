// 当前只验证显示和采集；避免未配置的 ESP 阻塞主循环。
pub(super) const ESP8266_ENABLED: bool = false;
pub const ESP8266_UART_BAUD: u32 = 115_200;
pub(super) const ESP8266_WIFI_SSID: &str = "quantum-wifi";
pub(super) const ESP8266_WIFI_PASSWORD: &str = "qwerzxc123!@#";
pub(super) const ESP8266_HTTP_HOST: &str = "";
pub(super) const ESP8266_HTTP_PORT: u16 = 80;
pub(super) const ESP8266_HTTP_PATH: &str = "/sensor";
