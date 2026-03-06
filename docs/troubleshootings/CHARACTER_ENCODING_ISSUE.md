# 嵌入式系统中的字符编码问题

## 问题现象

在 STM32H750 显示屏上，温度单位 `°C` 显示为 `?C`。

## 根本原因

### 字体限制

在 [card.rs](file:///Users/lishuang/workDir/stm32h750-display/src/ui/widgets/card.rs) 和 [sensor.rs](file:///Users/lishuang/workDir/stm32h750-display/src/ui/sensor.rs) 中使用的字体：

```rust
use embedded_graphics::mono_font::ascii::FONT_10X20;
```

**关键问题**：`FONT_10X20` 是 ASCII 字体，只支持 0-127 的字符。

### 字符编码

- **°符号** 的 Unicode 编码：`U+00B0`（十进制 176）
- ASCII 字符集范围：0-127
- 结果：°符号不在 ASCII 范围内，显示为 `?`

## 解决方案

### 方案 1：使用 ASCII 兼容表示（推荐）

最简单的方法是使用纯 ASCII 字符：

```rust
// 修改前
write!(s, "{:.1}°C", temp);  // ❌ °符号不在 ASCII 中

// 修改后
write!(s, "{:.1}C", temp);   // ✅ 纯 ASCII 字符
```

**优点**：
- 简单直接
- 无需额外资源
- 兼容性好

**缺点**：
- 不够美观
- 不符合习惯表示

### 方案 2：使用自定义字符

如果确实需要显示 °符号，可以：

#### 2.1 使用支持 Unicode 的字体

`embedded-graphics` 提供了一些支持扩展字符的字体：

```rust
use embedded_graphics::mono_font::iso_8859_1::FONT_10X20;

// ISO 8859-1 支持 0-255 的字符，包含 °符号（176）
```

**注意**：需要检查 `embedded-graphics` 是否提供 ISO 8859-1 字体。

#### 2.2 自定义字体

创建自定义字体文件，包含需要的特殊字符：

```rust
// 定义自定义字体
const CUSTOM_FONT: MonoFont = MonoFont {
    // 字体数据，包含 °符号
    // ...
};

// 使用自定义字体
let style = MonoTextStyle::new(&CUSTOM_FONT, Rgb565::BLACK);
```

#### 2.3 使用图标

用图标代替文字：

```rust
// 绘制温度图标
icon.draw(display, x, y, scale, color)?;

// 显示数值
write!(s, "{:.1}", temp);
```

### 方案 3：使用上标字符

某些字体支持上标字符：

```rust
// 使用上标 o 代替 °
write!(s, "{:.1}\u{00B0}C", temp);  // 需要 Unicode 字体支持
```

## 实际应用

### 当前解决方案

本项目采用 **方案 1**，将所有 `°C` 改为 `C`：

**修改文件**：
- [src/ui/widgets/card.rs](file:///Users/lishuang/workDir/stm32h750-display/src/ui/widgets/card.rs#L139)
- [src/ui/sensor.rs](file:///Users/lishuang/workDir/stm32h750-display/src/ui/sensor.rs#L81)

**修改内容**：
```rust
// card.rs
let _ = write!(s, "hi:{:.0}C", self.sensor.temp_high);
let _ = write!(s, "lo:{:.0}C", self.sensor.temp_low);

// sensor.rs
if write!(s, "{:.1}C", self.temp_current).is_ok() {
    s
} else {
    heapless::String::from("--.-C")
}
```

### 显示效果

```
修改前：hi:28°C  → 显示为 hi:28?C
修改后：hi:28C   → 显示为 hi:28C  ✅
```

## 扩展知识

### 常见字符编码问题

| 字符 | Unicode | ASCII 支持 | 常见问题 |
|------|---------|-----------|---------|
| ° (度) | U+00B0 | ❌ | 显示为 ? |
| ± (正负) | U+00B1 | ❌ | 显示为 ? |
| μ (微) | U+03BC | ❌ | 显示为 ? |
| Ω (欧姆) | U+03A9 | ❌ | 显示为 ? |
| % (百分号) | U+0025 | ✅ | 正常显示 |

### embedded-graphics 字体类型

```rust
// ASCII 字体（0-127）
use embedded_graphics::mono_font::ascii::FONT_6X10;

// ISO 8859-1 字体（0-255）
// 注意：需要检查是否可用
use embedded_graphics::mono_font::iso_8859_1::FONT_6X10;

// 自定义字体
use embedded_graphics::mono_font::MonoFont;
```

### 字体资源消耗

| 字体类型 | 字符范围 | Flash 占用 | 适用场景 |
|---------|---------|-----------|---------|
| ASCII | 0-127 | ~1KB | 基本文本显示 |
| ISO 8859-1 | 0-255 | ~2KB | 西欧语言 |
| Unicode 子集 | 自定义 | 取决于字符数 | 特殊符号 |

## 最佳实践

### 1. 优先使用 ASCII 字符

在资源受限的嵌入式系统中：
- ✅ 使用 `C` 代替 `°C`
- ✅ 使用 `deg` 代替 `°`
- ✅ 使用 `u` 代替 `μ`
- ✅ 使用 `E` 代替 `±`

### 2. 必要时使用图标

对于常用的特殊符号：
- 创建图标资源
- 使用图标绘制 API
- 更美观、更灵活

### 3. 字体选择策略

```
基本需求 → ASCII 字体（FONT_6X10, FONT_10X20）
特殊字符 → ISO 8859-1 字体（如果可用）
自定义需求 → 自定义字体或图标
```

### 4. 测试字符显示

在开发阶段测试所有字符：

```rust
// 测试函数
fn test_font_display<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    
    // 测试 ASCII 字符
    Text::new("Test: 25C", Point::new(10, 10), style).draw(display)?;
    
    // 测试特殊字符（如果支持）
    // Text::new("Test: 25°C", Point::new(10, 30), style).draw(display)?;
    
    Ok(())
}
```

## 相关资源

- [embedded-graphics 文档](https://docs.rs/embedded-graphics/)
- [Unicode 字符表](https://unicode-table.com/)
- [ASCII 字符表](https://ascii.cl/)
- [ISO 8859-1 字符集](https://en.wikipedia.org/wiki/ISO/IEC_8859-1)

## 总结

在嵌入式系统中显示特殊字符时：

1. **检查字体支持**：确认使用的字体是否包含需要的字符
2. **优先 ASCII**：使用 ASCII 兼容的替代方案
3. **考虑资源**：权衡字体大小和功能需求
4. **测试验证**：在实际硬件上测试显示效果

通过使用 ASCII 兼容的字符表示，可以避免字符编码问题，确保在资源受限的嵌入式系统中正常显示。
