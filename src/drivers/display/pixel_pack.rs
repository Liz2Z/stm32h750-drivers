//! Build contiguous DMA pixel chunks from a clipped framebuffer rectangle.

/// Clip a refresh window without overflowing or sending empty regions.
pub(super) fn clip_region(
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    width: u16,
    height: u16,
) -> Option<(u16, u16, u16, u16)> {
    if w == 0 || h == 0 || x >= width || y >= height {
        return None;
    }
    Some((x, y, w.min(width - x), h.min(height - y)))
}

/// Pack native RGB565 half-words; SPI's 16-bit MSB-first mode supplies wire order.
pub(super) fn pack_rect_words(
    frame: &[u16],
    stride: usize,
    region: (u16, u16, u16, u16),
    offset: usize,
    destination: &mut [u16],
) -> usize {
    let (x, y, w, h) = region;
    let width = w as usize;
    let count = (width * h as usize)
        .saturating_sub(offset)
        .min(destination.len());
    let mut copied = 0;
    while copied < count {
        let pos = offset + copied;
        let row = pos / width;
        let col = pos % width;
        let run = (width - col).min(count - copied);
        let start = (y as usize + row) * stride + x as usize + col;
        destination[copied..copied + run].copy_from_slice(&frame[start..start + run]);
        copied += run;
    }
    count
}
