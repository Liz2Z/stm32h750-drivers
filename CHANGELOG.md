# 更新日志

本项目的所有重要更改都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 新增
- 待添加的新功能

### 变更
- 待变更的内容

### 修复
- 待修复的问题

## [0.1.0] - 2025-01-XX

### 新增
- ILI9341 显示驱动支持
- 硬件 SPI 加速实现（10x 性能提升）
- embedded-graphics 图形库集成
- 多种传感器驱动支持：
  - AHT20 温湿度传感器
  - BMP280 气压传感器
  - DHT11 温湿度传感器
  - NTC 热敏电阻 ADC 读取
- UI 组件系统：
  - Button 按钮
  - Card 卡片
  - Label 标签
  - Progress 进度条
  - History 历史图表
- 主题系统支持
- 串口调试输出
- OpenOCD 调试支持
- 完整的项目文档
  - 架构说明
  - 硬件连接指南
  - 故障排除文档

### 性能
- 硬件 SPI 相比软件 SPI 提升 10 倍速度
- 清屏操作从 ~500ms 优化到 ~50ms

### 技术细节
- 直接操作 SPI2 寄存器绕过 HAL 兼容性问题
- 支持帧缓冲区
- 模块化驱动架构

[Unreleased]: https://github.com/yourusername/stm32h750-drivers/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/stm32h750-drivers/releases/tag/v0.1.0
