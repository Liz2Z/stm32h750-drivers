//! 验证控件、主题、屏幕刷新和动画组合后的完整画面。
mod common;

use common::{screen, Panel};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use stm32h750_drivers::ui::FlowAnimation;

#[test]
fn widget_owned_inputs_preserve_original_card_and_footer_pixels() {
    use stm32h750_drivers::ui::{
        status::{draw_status, ConnectionLabels, StatusData, StatusLabel, StatusTone},
        PressureCardData, TempHumidCardData, ValueRange, ValueStatus,
    };
    // 改动前实现生成的 RGB565 小端像素 FNV-1a 快照，覆盖等待、失败、正常、旧值和恢复。
    // 控件测试只构造 UI 输入，不创建任何领域模型。
    let expected = [
        [0xf87b538557828dcb, 0x8878aeb1e1a8000f],
        [0xced7583177dcd7fb, 0x6000d3cb60bcf9ff],
        [0x23b4ba5ef6ac94fb, 0xb4de35f8df8cb6ff],
        [0x2594c85cdbc365eb, 0x97735c712a85daef],
        [0xbc7ef32c19d6b795, 0x58b1da95008da0a9],
    ];
    let range = |current, high, low| Some(ValueRange { current, high, low });
    for (phase, expected) in expected.into_iter().enumerate() {
        let status = match phase {
            0 => ValueStatus::Waiting,
            1 => ValueStatus::Error,
            3 => ValueStatus::Stale,
            _ => ValueStatus::Normal,
        };
        let (temp, humid, pressure) = if phase < 2 {
            (None, None, None)
        } else {
            (
                if phase == 4 {
                    range(0.0, 0.0, -12.3)
                } else {
                    range(-12.3, -12.3, -12.3)
                },
                range(55.0, 55.0, 55.0),
                range(1000.0, 1000.0, 1000.0),
            )
        };
        let mut view = screen();
        view.update_sensors(
            TempHumidCardData {
                values: temp,
                status,
            },
            TempHumidCardData {
                values: humid,
                status,
            },
            PressureCardData {
                values: pressure,
                status,
            },
        );
        let mut panel = Panel::new();
        view.draw_with_dma(&mut panel).unwrap();
        for (enabled, expected) in [false, true].into_iter().zip(expected) {
            draw_status(
                &mut panel,
                StatusData {
                    connection: enabled.then_some(ConnectionLabels {
                        link: StatusLabel {
                            text: "ESP: failed",
                            tone: StatusTone::Failure,
                        },
                        upload: StatusLabel {
                            text: "Upload: disabled",
                            tone: StatusTone::Muted,
                        },
                    }),
                    sample_count: 42,
                    refresh_centihz: Some(5920),
                },
            );
            let hash = panel
                .framebuffer
                .iter()
                .fold(0xcbf29ce484222325u64, |mut hash, pixel| {
                    for byte in pixel.into_storage().to_le_bytes() {
                        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                    }
                    hash
                });
            assert_eq!(hash, expected, "phase={phase}, connection={enabled}");
        }
    }
}

#[test]
fn partial_render_matches_full_frame_after_shorter_numbers_and_errors() {
    let mut screen = screen();
    let mut partial = Panel::new();
    screen.draw_with_dma(&mut partial).unwrap();
    use stm32h750_drivers::ui::{PressureCardData, TempHumidCardData, ValueRange, ValueStatus};
    for value in [-12.3, 0.0, 5.0, 100.0] {
        let status = if value == 0.0 {
            ValueStatus::Stale
        } else {
            ValueStatus::Normal
        };
        screen.update_sensors(
            TempHumidCardData {
                values: Some(ValueRange {
                    current: value,
                    high: value.max(-12.3),
                    low: -12.3,
                }),
                status,
            },
            TempHumidCardData {
                values: Some(ValueRange {
                    current: 50.0,
                    high: 50.0,
                    low: 50.0,
                }),
                status: ValueStatus::Normal,
            },
            PressureCardData {
                values: Some(ValueRange {
                    current: 1000.0,
                    high: 1000.0,
                    low: 1000.0,
                }),
                status,
            },
        );
        screen.draw_sensor_updates(&mut partial).unwrap();
        let mut full = Panel::new();
        screen.draw_with_dma(&mut full).unwrap();
        assert_eq!(partial.visible, full.visible);
    }
}

#[test]
fn animation_only_changes_its_strip_and_skips_late_frames_across_wrap() {
    let mut panel = Panel::new();
    let mut animation = FlowAnimation::new(u32::MAX - 49_999);
    animation.refresh(&mut panel, 0).unwrap();
    let first = panel.visible.clone();
    animation.refresh(&mut panel, 750_000).unwrap();
    assert_ne!(first, panel.visible);
    assert!(panel
        .windows
        .iter()
        .all(|&window| window == (5, 3, 310, 14)));
    for y in 0..240 {
        for x in 0..320 {
            if !(5..315).contains(&x) || !(3..17).contains(&y) {
                assert_eq!(panel.framebuffer[y * 320 + x], Rgb565::BLACK);
            }
        }
    }
    let mut reference = Panel::new();
    let mut from_zero = FlowAnimation::new(0);
    from_zero.refresh(&mut reference, 800_000).unwrap();
    assert_eq!(panel.visible, reference.visible);
    animation.refresh(&mut panel, 6_750_000).unwrap();
    assert_eq!(panel.visible, reference.visible);
}
