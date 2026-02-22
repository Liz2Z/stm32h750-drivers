# 为什么这么多抽象层？

### **问题**

所以 embedded_graphics 其实就是提供一个更简单的方式去绘制图形？ 比如绘制一个矩形，使用抽象层就只要： `new Rect(x,y, size, color)`，但是直接使用 display 驱动就要

```

cs low
d/c command
mv x, y
set color
d/c data
send size
cs hight

```

这种？

### **答案**

完全正确！你理解得非常准确。

让我用代码对比一下：

**使用 embedded_graphics (高级抽象)**

```
// 绘制一个红色矩形
Rectangle::new(Point::new(10, 20), Size::new(50, 30))
    .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
    .draw(&mut display)?;
```

**直接使用 DisplaySpi (底层驱动)**

```
// 你需要手动控制整个过程
display.fill_rect(10, 20, 50, 30, Rgb565::RED);
```

而 `fill_rect` 内部实际做的是：

```
fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, color: Rgb565) {
// 1. CS 拉低，选中设备
self.cs.set_low();

    // 2. DC 拉低，准备发送命令
    self.dc.set_low();
    self.spi.write(&[CASET]);  // 设置 X 轴范围命令

    // 3. DC 拉高，发送参数
    self.dc.set_high();
    self.spi.write(&[x >> 8, x & 0xFF, (x+w-1) >> 8, (x+w-1) & 0xFF]);

    // 4. DC 拉低，设置 Y 轴
    self.dc.set_low();
    self.spi.write(&[PASET]);

    // 5. DC 拉高，发送 Y 参数
    self.dc.set_high();
    self.spi.write(&[y >> 8, y & 0xFF, (y+h-1) >> 8, (y+h-1) & 0xFF]);

    // 6. DC 拉低，发送写像素命令
    self.dc.set_low();
    self.spi.write(&[RAMWR]);

    // 7. DC 拉高，发送所有像素数据
    self.dc.set_high();
    for _ in 0..(w * h) {
        self.spi.write(&[color.byte_high(), color.byte_low()]);
    }

    // 8. CS 拉高，释放总线
    self.cs.set_high();

}
```

**embedded_graphics 带来的好处**

| 功能     | 用 embedded_graphics                          | 直接用驱动                  |
| -------- | --------------------------------------------- | --------------------------- |
| 画矩形   | Rectangle::new(...).draw(&mut display) ✅     | 需要 8 步 SPI 操作          |
| 画圆形   | Circle::new(...).draw(&mut display) ✅        | 需要自己实现 Bresenham 算法 |
| 画文字   | Text::new("Hello", ...).draw(&mut display) ✅ | 需要字体渲染引擎            |
| 画线条   | Line::new(...).draw(&mut display) ✅          | 需要 Bresenham 直线算法     |
| 组合图形 | 嵌套使用 ✅                                   | 需要手动管理坐标            |

```

// 直接调用底层驱动 - 快速但功能单一
display.fill_rect(x as u16, y as u16, box_size, box_size, Rgb565::BLACK);

// 使用 embedded_graphics - 功能强大
let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
let text = Text::new("Hello World!", Point::new(40, 150), text_style);
text.draw(&mut display).unwrap(); // 内部调用你的 draw_iter()
```

总结：embedded_graphics 就是一个"绘图工具库"，而你的 DisplaySpi 是"硬件驱动"。embedded_graphics 通过 `DrawTarget` trait 与你的驱动对接 — 你只需要实现基础的像素写入，embedded_graphics 就能提供各种高级绘图功能！

**抽象就是为了消除(划掉)**屏蔽**复杂度**
