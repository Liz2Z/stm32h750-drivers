# RC522 SPI 通信踩坑记录

## 问题现象

在对接 RC522 RFID 模块时，常见问题包括：

1. 初始化后读 `VersionReg` 一直是 `0x00` 或 `0xFF`。
2. `REQA` 无响应，始终超时。
3. 防冲突阶段经常报 `Collision` 或 `Protocol`。

## 问题分析与解决

### 1) SPI 寄存器地址格式错误

RC522 的 SPI 帧地址不是直接寄存器地址：

- 写：`((reg << 1) & 0x7E)`
- 读：`((reg << 1) & 0x7E) | 0x80`

如果把寄存器地址直接发出去，RC522 会忽略请求，表现为读值异常。

### 2) 忘记打开天线

初始化后必须设置 `TxControlReg` 的低两位（`0x03`），否则射频场未开启，卡片无法应答。

### 3) 位帧设置错误

执行 `REQA` 时，`BitFramingReg` 需要设置为 `0x07`（只发 7 bit）。

如果保持默认 `0x00`，会导致请求命令帧格式不正确，卡片可能不响应。

### 4) CRC 与 ACK 判断

MIFARE Read/Write 必须附带 CRC_A。

写块流程中，RC522 期望 4 bit ACK（`0x0A`），需要同时校验：

- `valid_bits == 4`
- `(ack & 0x0F) == 0x0A`

只看 `ack` 不看 `valid_bits`，容易误判。

## 本项目实现建议

- 上电后先读 `VersionReg` 验证硬件连通。
- 每次 `Transceive` 前清 FIFO 并清中断标志。
- 超时使用有限循环计数，避免死循环阻塞主线程。
- 认证成功后，操作完成要清 `Status2Reg.MFCrypto1On`。

## 快速排查清单

1. 硬件接线：`SCK/MOSI/MISO/CS/RST/GND/VCC`。
2. SPI 模式和频率：先用较低频率（如 1~4MHz）验证。
3. `VersionReg` 是否返回常见值（`0x91` / `0x92`）。
4. `REQA` 前是否设置了 `BitFramingReg = 0x07`。
5. 是否已调用天线开启逻辑。
