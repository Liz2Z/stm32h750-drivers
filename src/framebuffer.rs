/// Pack a clipped RGB565 rectangle into big-endian bytes for SPI. A chunk may
/// span several framebuffer rows; the unused pixels outside the window are skipped.
pub fn pack_rect_chunk(
    frame: &[u16],
    stride: usize,
    region: (u16, u16, u16, u16),
    offset: usize,
    destination: &mut [u8],
) -> usize {
    let (x, y, w, h) = region;
    let rect_width = w as usize;
    let total = rect_width * h as usize;
    let count = total.saturating_sub(offset).min(destination.len() / 2);
    let mut copied = 0;
    while copied < count {
        let pos = offset + copied;
        let row = pos / rect_width;
        let col = pos % rect_width;
        let run = (rect_width - col).min(count - copied);
        let start = (y as usize + row) * stride + x as usize + col;
        for (i, pixel) in frame[start..start + run].iter().enumerate() {
            let bytes = pixel.to_be_bytes();
            let dest = (copied + i) * 2;
            destination[dest..dest + 2].copy_from_slice(&bytes);
        }
        copied += run;
    }
    count
}
