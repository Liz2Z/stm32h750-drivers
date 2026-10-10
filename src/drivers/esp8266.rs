//! ESP8266 AT 固件串口驱动
//!
//! 默认假设 ESP-01S 已刷回官方 AT 固件，通过 UART 使用阻塞式 AT 命令通信。

use core::fmt::Write as FmtWrite;

use embedded_hal::serial::{Read as SerialRead, Write as SerialWrite};
use heapless::String;

/// 当前一次采样数据
#[derive(Clone, Copy, Debug)]
pub struct SensorPayload {
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
}

/// Wi-Fi 连接状态
#[derive(Clone, Copy, Debug)]
pub struct WifiStatus {
    pub connected: bool,
}

/// HTTP 上报配置
#[derive(Clone, Copy, Debug)]
pub struct HttpEndpoint<'a> {
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
}

/// ESP8266 通信错误
#[derive(Clone, Copy, Debug)]
pub enum Esp8266Error {
    SerialWrite,
    SerialRead,
    Timeout,
    ResponseOverflow,
    UnexpectedResponse,
    InvalidConfig,
}

/// 简单的阻塞式 AT 客户端
pub struct Esp8266<TX, RX> {
    tx: TX,
    rx: RX,
}

impl<TX, RX, TXE, RXE> Esp8266<TX, RX>
where
    TX: SerialWrite<u8, Error = TXE>,
    RX: SerialRead<u8, Error = RXE>,
{
    pub fn new(tx: TX, rx: RX) -> Self {
        Self { tx, rx }
    }

    /// 初始化串口链路并连接 Wi-Fi。
    pub fn initialize<F>(
        &mut self,
        delay_ms: &mut F,
        ssid: &str,
        password: &str,
    ) -> Result<WifiStatus, Esp8266Error>
    where
        F: FnMut(u32),
    {
        if ssid.is_empty() || password.is_empty() {
            return Err(Esp8266Error::InvalidConfig);
        }

        delay_ms(1500);
        self.flush_input(delay_ms, 300);
        self.test_at_with_retry(delay_ms, 6, 500)?;
        self.send_basic(delay_ms, "ATE0", 1000)?;
        self.send_basic(delay_ms, "AT+CWMODE=1", 1000)?;
        self.join_ap(delay_ms, ssid, password)?;
        self.query_wifi(delay_ms)
    }

    pub fn test_at<F>(&mut self, delay_ms: &mut F) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        self.send_basic(delay_ms, "AT", 800)
    }

    pub fn test_at_with_retry<F>(
        &mut self,
        delay_ms: &mut F,
        retries: u32,
        retry_gap_ms: u32,
    ) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        let mut last_err = Esp8266Error::Timeout;

        for _ in 0..retries {
            match self.test_at(delay_ms) {
                Ok(_) => return Ok(()),
                Err(err) => {
                    last_err = err;
                    delay_ms(retry_gap_ms);
                    self.flush_input(delay_ms, 100);
                },
            }
        }

        Err(last_err)
    }

    pub fn query_wifi<F>(&mut self, delay_ms: &mut F) -> Result<WifiStatus, Esp8266Error>
    where
        F: FnMut(u32),
    {
        let response = self.send_command(delay_ms, "AT+CIPSTA?", 1500)?;
        Ok(WifiStatus {
            connected: response.contains("+CIPSTA:ip:"),
        })
    }

    pub fn send_sensor_data<F>(
        &mut self,
        delay_ms: &mut F,
        endpoint: HttpEndpoint<'_>,
        payload: SensorPayload,
    ) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        if endpoint.host.is_empty() || endpoint.path.is_empty() {
            return Err(Esp8266Error::InvalidConfig);
        }

        self.close_connection(delay_ms);

        let mut request: String<256> = String::new();
        write!(
            request,
            "GET {}?temp={:.1}&hum={:.1}&pres={:.1} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            endpoint.path, payload.temperature, payload.humidity, payload.pressure, endpoint.host
        )
        .map_err(|_| Esp8266Error::ResponseOverflow)?;

        let mut connect_cmd: String<96> = String::new();
        write!(
            connect_cmd,
            "AT+CIPSTART=\"TCP\",\"{}\",{}",
            endpoint.host, endpoint.port
        )
        .map_err(|_| Esp8266Error::ResponseOverflow)?;
        self.send_basic(delay_ms, &connect_cmd, 8000)?;

        let mut send_cmd: String<48> = String::new();
        write!(send_cmd, "AT+CIPSEND={}", request.len())
            .map_err(|_| Esp8266Error::ResponseOverflow)?;

        self.write_line(&send_cmd)?;
        self.wait_for_prompt(delay_ms, 4000)?;
        self.write_bytes(request.as_bytes())?;
        self.wait_for_send_ok(delay_ms, 8000)?;
        self.close_connection(delay_ms);

        Ok(())
    }

    fn join_ap<F>(
        &mut self,
        delay_ms: &mut F,
        ssid: &str,
        password: &str,
    ) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        let mut command: String<128> = String::new();
        write!(command, "AT+CWJAP=\"{}\",\"{}\"", ssid, password)
            .map_err(|_| Esp8266Error::ResponseOverflow)?;
        self.send_basic(delay_ms, &command, 30000)
    }

    fn send_basic<F>(
        &mut self,
        delay_ms: &mut F,
        command: &str,
        timeout_ms: u32,
    ) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        let _ = self.send_command(delay_ms, command, timeout_ms)?;
        Ok(())
    }

    fn send_command<F>(
        &mut self,
        delay_ms: &mut F,
        command: &str,
        timeout_ms: u32,
    ) -> Result<String<256>, Esp8266Error>
    where
        F: FnMut(u32),
    {
        self.flush_input(delay_ms, 20);
        self.write_line(command)?;

        let mut line: String<256> = String::new();
        let mut aggregate: String<256> = String::new();
        let mut waited_ms = 0;

        while waited_ms < timeout_ms {
            if self.read_line(delay_ms, 10, &mut line)? {
                if !aggregate.is_empty() && aggregate.push('\n').is_err() {
                    return Err(Esp8266Error::ResponseOverflow);
                }
                if aggregate.push_str(line.as_str()).is_err() {
                    return Err(Esp8266Error::ResponseOverflow);
                }

                if line == "OK" || line.ends_with("OK") {
                    return Ok(aggregate);
                }

                if line == "ERROR" || line.ends_with("ERROR") {
                    return Err(Esp8266Error::UnexpectedResponse);
                }

                if line == "busy p..." || line == "busy s..." {
                    return Err(Esp8266Error::UnexpectedResponse);
                }
            }

            waited_ms += 10;
        }

        Err(Esp8266Error::Timeout)
    }

    fn wait_for_prompt<F>(&mut self, delay_ms: &mut F, timeout_ms: u32) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        let mut waited_ms = 0;

        while waited_ms < timeout_ms {
            match self.rx.read() {
                Ok(b'>') => return Ok(()),
                Ok(_) => {},
                Err(nb::Error::WouldBlock) => {
                    delay_ms(1);
                    waited_ms += 1;
                },
                Err(nb::Error::Other(_)) => return Err(Esp8266Error::SerialRead),
            }
        }

        Err(Esp8266Error::Timeout)
    }

    fn wait_for_send_ok<F>(&mut self, delay_ms: &mut F, timeout_ms: u32) -> Result<(), Esp8266Error>
    where
        F: FnMut(u32),
    {
        let mut line: String<256> = String::new();
        let mut waited_ms = 0;
        let mut got_send_ok = false;

        while waited_ms < timeout_ms {
            if self.read_line(delay_ms, 10, &mut line)? {
                if line.contains("SEND OK") {
                    got_send_ok = true;
                }

                if got_send_ok && line == "OK" {
                    return Ok(());
                }

                if line == "ERROR" || line.contains("FAIL") {
                    return Err(Esp8266Error::UnexpectedResponse);
                }
            }

            waited_ms += 10;
        }

        Err(Esp8266Error::Timeout)
    }

    fn close_connection<F>(&mut self, delay_ms: &mut F)
    where
        F: FnMut(u32),
    {
        let _ = self.send_basic(delay_ms, "AT+CIPCLOSE", 1000);
    }

    fn flush_input<F>(&mut self, delay_ms: &mut F, quiet_ms: u32)
    where
        F: FnMut(u32),
    {
        let mut quiet = 0;

        while quiet < quiet_ms {
            match self.rx.read() {
                Ok(_) => quiet = 0,
                Err(nb::Error::WouldBlock) => {
                    delay_ms(1);
                    quiet += 1;
                },
                Err(nb::Error::Other(_)) => {
                    delay_ms(1);
                    quiet += 1;
                },
            }
        }
    }

    fn read_line<F>(
        &mut self,
        delay_ms: &mut F,
        timeout_ms: u32,
        line: &mut String<256>,
    ) -> Result<bool, Esp8266Error>
    where
        F: FnMut(u32),
    {
        line.clear();
        let mut waited_ms = 0;

        while waited_ms < timeout_ms {
            match self.rx.read() {
                Ok(byte) => {
                    if byte == b'\r' {
                        continue;
                    }

                    if byte == b'\n' {
                        if !line.is_empty() {
                            return Ok(true);
                        }
                        continue;
                    }

                    if line.push(byte as char).is_err() {
                        return Err(Esp8266Error::ResponseOverflow);
                    }
                },
                Err(nb::Error::WouldBlock) => {
                    delay_ms(1);
                    waited_ms += 1;
                },
                Err(nb::Error::Other(_)) => return Err(Esp8266Error::SerialRead),
            }
        }

        Ok(false)
    }

    fn write_line(&mut self, s: &str) -> Result<(), Esp8266Error> {
        self.write_bytes(s.as_bytes())?;
        self.write_bytes(b"\r\n")
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Esp8266Error> {
        for &byte in bytes {
            loop {
                match self.tx.write(byte) {
                    Ok(()) => break,
                    Err(nb::Error::WouldBlock) => {},
                    Err(nb::Error::Other(_)) => return Err(Esp8266Error::SerialWrite),
                }
            }
        }

        Ok(())
    }
}
