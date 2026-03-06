# 贡献指南

感谢你对 STM32H750 Display 项目的兴趣！本文档将帮助你了解如何为项目做出贡献。

## 行为准则

- 尊重所有贡献者
- 接受建设性批评
- 关注对社区最有利的事情

## 如何贡献

### 报告 Bug

如果你发现了 bug，请创建一个 Issue 并包含：

1. **清晰的标题**：简短描述问题
2. **复现步骤**：详细说明如何复现问题
3. **预期行为**：你期望发生什么
4. **实际行为**：实际发生了什么
5. **环境信息**：
   - Rust 版本 (`rustc --version`)
   - 目标平台 (`thumbv7em-none-eabihf`)
   - 硬件配置
6. **日志输出**：如果有相关的调试输出

### 提出新功能

1. 先创建 Issue 讨论你的想法
2. 说明为什么这个功能对项目有用
3. 等待维护者反馈后再开始实现

### 提交代码

#### 开发环境设置

```bash
# 克隆仓库
git clone https://github.com/yourusername/stm32h750-display.git
cd stm32h750-display

# 安装 Rust 目标
rustup target add thumbv7em-none-eabihf

# 安装调试工具 (macOS/Linux)
# Ubuntu: sudo apt install gdb-multiarch openocd
# macOS: brew install arm-none-eabi-gdb openocd
```

#### 代码风格

- 运行 `cargo fmt` 确保代码格式正确
- 运行 `cargo clippy` 检查代码质量
- 为新功能添加文档注释
- 保持函数简短和单一职责

#### 提交规范

使用清晰的提交信息：

```
<type>: <subject>

<body>
```

类型包括：
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式调整
- `refactor`: 代码重构
- `test`: 测试相关
- `chore`: 构建/工具相关

示例：
```
feat: 添加 BMP280 温度传感器支持

- 实现 BMP280 驱动
- 添加温度和气压读取功能
- 更新文档
```

#### Pull Request 流程

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

#### PR 检查清单

- [ ] 代码通过 `cargo build` 编译
- [ ] 代码通过 `cargo fmt --check` 检查
- [ ] 代码通过 `cargo clippy` 检查
- [ ] 更新了相关文档
- [ ] 添加了必要的注释
- [ ] 提交信息清晰明确

## 测试

由于这是嵌入式项目，测试主要依赖硬件验证：

1. 确保代码能在目标硬件上编译
2. 使用 ST-Link 和 OpenOCD 进行调试
3. 验证硬件功能正常

```bash
# 编译检查
make build

# 烧录测试
make flash
```

## 文档贡献

文档改进同样重要！你可以：

- 修正拼写或语法错误
- 改进文档结构
- 添加示例代码
- 翻译文档

## 问题？

如果你有任何问题，可以：

- 创建 Issue
- 查阅 [故障排除文档](./docs/troubleshootings/)

## 许可证

通过贡献代码，你同意你的代码将以 MIT 许可证发布。

---

再次感谢你的贡献！🎉
