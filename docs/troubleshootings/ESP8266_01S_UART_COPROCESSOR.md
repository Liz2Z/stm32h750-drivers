# ESP8266-01S 串口协处理器模式踩坑记录

## 问题现象

STM32H750 已经和 ESP-01S 正确连线，但数据始终发不出去，或者串口一直收不到预期的 `OK` 响应。

## 根本原因

这块 ESP-01S 跑的不是 AT 固件，而是自定义的 Arduino 草图 `ota.ino`。

这意味着：

- 不能发送 `AT+RST`、`AT+CWJAP`、`AT+CIPSEND` 之类的 AT 指令
- 必须按草图里定义的**文本行协议**通信
- ESP 上电后会先输出启动日志，STM32 需要忽略这段噪声

当前协议关键字：

```text
PING
INFO
WIFI?
SENSOR TEMP=25.0 HUM=60.0 PRES=1013.2
LED ON
LED OFF
REBOOT
```

典型返回：

```text
READY WIFI=CONNECTED IP=192.168.1.20
OK PONG
OK WIFI=CONNECTED IP=192.168.1.20
OK SENSOR=UPDATED
ERR UNKNOWN_CMD
```

## 正确做法

### 1. STM32 侧按行协议通信

- 波特率保持和 ESP 草图一致：`115200`
- 每条命令以 `\r\n` 结束
- 接收时忽略 `\r`
- 读取到 `\n` 才算一行结束

### 2. 上电先等待 READY，再发 PING

ESP8266 ROM 会在上电时输出启动日志，串口上可能先看到乱码或无关文本。

因此 STM32 初始化顺序应该是：

1. 清空接收缓冲区
2. 尝试等待 `READY ...`
3. 发送 `PING`
4. 再发送 `WIFI?`

### 3. 传感器数据不要走 AT 指令，直接走自定义命令

推荐格式：

```text
SENSOR TEMP=25.3 HUM=61.2 PRES=1008.6
```

ESP 收到后保存最新值，并通过 HTTP 接口暴露：

```text
GET /sensor
```

## 经验教训

⚠️ **先确认 ESP-01S 跑的到底是什么固件，再决定 STM32 侧协议。**

- 如果是官方 AT 固件，用 `AT+...`
- 如果是自定义 Arduino 草图，用草图里定义的文本命令

⚠️ **ESP-01S 上电启动日志会污染串口前几百毫秒，初始化阶段必须容忍噪声。**

⚠️ **`GPIO0/GPIO2/EN/RST` 的电平只保证 ESP 正常启动，不代表 STM32 一定能直接用 AT 指令通信。**
