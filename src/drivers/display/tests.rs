//! Tests for the same clipping, packing and command path used on the board.
use super::{
    pixel_pack::{clip_region, pack_rect_words},
    spi_transfer::write_bytes,
    Error,
};
use embedded_hal::spi::FullDuplex;
use std::{vec, vec::Vec};

#[derive(Default)]
struct CommandSpi {
    sent: Vec<u8>,
    stalled: bool,
    fault: bool,
}

impl FullDuplex<u8> for CommandSpi {
    type Error = ();

    fn send(&mut self, byte: u8) -> nb::Result<(), ()> {
        if self.fault {
            return Err(nb::Error::Other(()));
        }
        if self.stalled {
            return Err(nb::Error::WouldBlock);
        }
        self.sent.push(byte);
        Ok(())
    }

    fn read(&mut self) -> nb::Result<u8, ()> {
        panic!("Transmit-only commands must not wait for RX");
    }
}

#[test]
fn commands_send_all_bytes_without_waiting_for_rx() {
    let mut spi = CommandSpi::default();
    let data = [0x2a, 0x00, 0x01, 0x01, 0x3f, 0x2c];
    write_bytes(&mut spi, || 0, &data, 100).unwrap();
    assert_eq!(spi.sent, data);
    write_bytes(&mut spi, || 0, &[], 100).unwrap();
    assert_eq!(spi.sent, data);
}

#[test]
fn stalled_commands_timeout_across_counter_wrap() {
    let mut spi = CommandSpi {
        stalled: true,
        ..CommandSpi::default()
    };
    let mut now = u32::MAX - 50;
    assert_eq!(
        write_bytes(
            &mut spi,
            || {
                now = now.wrapping_add(10);
                now
            },
            &[1],
            100
        ),
        Err(Error::Timeout)
    );
}

#[test]
fn command_errors_are_propagated() {
    let mut spi = CommandSpi {
        fault: true,
        ..CommandSpi::default()
    };
    assert_eq!(write_bytes(&mut spi, || 0, &[1], 100), Err(Error::Spi(())));
}

#[test]
fn refresh_clips_empty_and_overflowing_regions() {
    assert_eq!(clip_region(5, 5, 0, 10, 320, 240), None);
    assert_eq!(clip_region(5, 5, 10, 0, 320, 240), None);
    assert_eq!(clip_region(320, 0, 1, 1, 320, 240), None);
    assert_eq!(
        clip_region(319, 239, u16::MAX, u16::MAX, 320, 240),
        Some((319, 239, 1, 1))
    );
}

#[test]
fn packed_rectangle_skips_row_padding_and_preserves_unused_tail() {
    let frame: Vec<u16> = (0x1200..0x1214).collect(); // 5 x 4 framebuffer
    let region = (1, 1, 3, 2);
    let mut first = [0; 4];
    let count = pack_rect_words(&frame, 5, region, 0, &mut first);
    assert_eq!(count, 4);
    assert_eq!(first, [0x1206, 0x1207, 0x1208, 0x120b]);
    let mut second = [0xaaaa; 4];
    let count = pack_rect_words(&frame, 5, region, 4, &mut second);
    assert_eq!(count, 2);
    assert_eq!(second, [0x120c, 0x120d, 0xaaaa, 0xaaaa]);
}

#[test]
fn native_word_chunks_match_wire_pixels_for_varied_rectangles_and_chunk_sizes() {
    let frame: Vec<u16> = (0..120).map(|i| 0x8100 + i * 17).collect(); // 12 x 10
    for region @ (x, y, w, h) in [(0, 0, 12, 10), (1, 2, 7, 5), (11, 9, 1, 1), (4, 1, 2, 8)] {
        let expected: Vec<u16> = (y..y + h)
            .flat_map(|row| {
                frame[row as usize * 12 + x as usize..row as usize * 12 + (x + w) as usize]
                    .iter()
                    .copied()
            })
            .collect();
        for capacity in [1, 3, 8, 4096] {
            let mut output = Vec::new();
            let mut chunk = vec![0xabcd; capacity];
            loop {
                chunk.fill(0xabcd);
                let count = pack_rect_words(&frame, 12, region, output.len(), &mut chunk);
                assert!(chunk[count..].iter().all(|&p| p == 0xabcd));
                if count == 0 {
                    break;
                }
                output.extend_from_slice(&chunk[..count]);
            }
            assert_eq!(output, expected);
            let wire: Vec<u8> = output.into_iter().flat_map(u16::to_be_bytes).collect();
            let reference: Vec<u8> = expected.iter().flat_map(|p| p.to_be_bytes()).collect();
            assert_eq!(wire, reference);
        }
    }
}

#[derive(Default)]
struct TransferLog {
    commands: Vec<u8>,
    parameters: Vec<Vec<u8>>,
    pixels: Vec<u16>,
    chunks: Vec<usize>,
    active: bool,
    finishes: usize,
    fail_write: bool,
}

struct RecordingInterface(std::rc::Rc<std::cell::RefCell<TransferLog>>);
impl super::DisplayInterface for RecordingInterface {
    type Error = Error<()>;
    fn write_command(&mut self, command: u8) -> Result<(), Self::Error> {
        self.0.borrow_mut().commands.push(command);
        Ok(())
    }
    fn write_data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.0.borrow_mut().parameters.push(data.to_vec());
        Ok(())
    }
    fn write_cmd_data(&mut self, command: u8, data: u8) -> Result<(), Self::Error> {
        self.write_command(command)?;
        self.write_data(&[data])
    }
    fn begin_pixels(&mut self, command: u8) -> Result<(), Self::Error> {
        let mut log = self.0.borrow_mut();
        assert!(!log.active);
        log.commands.push(command);
        log.active = true;
        Ok(())
    }
    fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), Self::Error> {
        let mut log = self.0.borrow_mut();
        assert!(log.active);
        if log.fail_write {
            return Err(Error::Dma);
        }
        log.pixels.extend_from_slice(pixels);
        log.chunks.push(pixels.len());
        Ok(())
    }
    fn finish_pixels(&mut self, result: Result<(), Self::Error>) -> Result<(), Self::Error> {
        let mut log = self.0.borrow_mut();
        assert!(log.active);
        log.active = false;
        log.finishes += 1;
        result
    }
}

type TestDisplay = super::DisplayDriver<RecordingInterface>;
fn recording_display() -> (TestDisplay, std::rc::Rc<std::cell::RefCell<TransferLog>>) {
    use std::{boxed::Box, cell::RefCell, rc::Rc};
    let frame = Box::leak(Box::new([0; super::MAX_FRAME_BUFFER_SIZE]));
    for (i, pixel) in frame.iter_mut().enumerate() {
        *pixel = i as u16;
    }
    let transfer = Box::leak(Box::new([0; super::DMA_BUF_WORDS]));
    let log = Rc::new(RefCell::new(TransferLog::default()));
    (
        super::DisplayDriver::new(
            RecordingInterface(log.clone()),
            frame,
            transfer,
            super::DisplayOrientation::Landscape,
        ),
        log,
    )
}

#[test]
fn driver_sets_window_and_sends_only_rectangle_pixels() {
    let (mut display, log) = recording_display();
    display.flush_rect(1, 2, 3, 2).unwrap();
    let log = log.borrow();
    // ILI9341 CASET (2Ah), PASET (2Bh), RAMWR (2Ch).
    assert_eq!(log.commands, [0x2a, 0x2b, 0x2c]);
    assert_eq!(log.parameters, [vec![0, 1, 0, 3], vec![0, 2, 0, 3]]);
    assert_eq!(log.pixels, [641, 642, 643, 961, 962, 963]);
    assert_eq!(log.finishes, 1);
    assert!(!log.active);
}

#[test]
fn driver_splits_full_frame_at_dma_limit_without_losing_pixels() {
    let (mut display, log) = recording_display();
    display.flush().unwrap();
    let log = log.borrow();
    assert_eq!(log.chunks, [65535, 11265]);
    assert_eq!(
        log.pixels,
        (0..super::MAX_FRAME_BUFFER_SIZE)
            .map(|i| i as u16)
            .collect::<Vec<_>>()
    );
    assert_eq!(log.finishes, 1);
    assert!(!log.active);
}

#[test]
fn driver_finishes_failed_pixel_session_and_can_retry() {
    let (mut display, log) = recording_display();
    log.borrow_mut().fail_write = true;
    assert_eq!(display.flush_rect(1, 2, 3, 2), Err(Error::Dma));
    assert!(!log.borrow().active);
    assert_eq!(log.borrow().finishes, 1);
    log.borrow_mut().fail_write = false;
    display.flush_rect(1, 2, 3, 2).unwrap();
    assert_eq!(log.borrow().finishes, 2);
    assert_eq!(log.borrow().pixels, [641, 642, 643, 961, 962, 963]);
    display.flush_rect(320, 0, 1, 1).unwrap();
    assert_eq!(log.borrow().finishes, 2);
}
