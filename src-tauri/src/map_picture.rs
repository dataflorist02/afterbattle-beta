//! Sharp map pictures (E5-maps-hd): the loading screens of `map/screen/` are DDS with BC1
//! (DXT1) blocks, 2560×1600 on the author's client. Our own code reads the DDS header with every
//! limit checked before any allocation, crops like CSS `cover` and scales down with a box filter
//! to the size the window really shows. The BC1 blocks are decoded by `bcdec_rs` (MIT, pure Rust)
//! and the result is written as PNG with fast compression by `png` (already in the build through
//! Tauri). It never scales up: a request larger than the source gets the source's own pixels.
//! Nothing is written to disk.

/// A DDS file larger than this is refused before it is read (2560×1600 BC1 with all mipmaps is
/// about 2.7 MB).
pub(crate) const MAX_DDS_BYTES: u64 = 4 * 1024 * 1024;
/// Largest accepted width or height.
pub(crate) const MAX_DDS_SIDE: u32 = 4096;
/// Most mipmap levels a 4096-pixel side can have (4096 … 1).
pub(crate) const MAX_DDS_MIPMAPS: u32 = 13;
/// Largest picture the window may ask for, in device pixels per side.
pub(crate) const MAX_OUTPUT_SIDE: u32 = 4096;

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const DDS_HEADER_SIZE: u32 = 124;
const PIXEL_FORMAT_SIZE: u32 = 32;
const DDPF_FOURCC: u32 = 0x4;
const DATA_OFFSET: usize = 128;
const BC1_BLOCK_BYTES: usize = 8;

/// An RGB picture, rows top to bottom, 3 bytes per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rgb {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// Width and height of a DDS with BC1 blocks, or `None` for anything else or anything outside
/// the limits. Only the first (largest) mipmap level is needed; the data must hold all of it.
pub(crate) fn bc1_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < DATA_OFFSET || bytes.len() as u64 > MAX_DDS_BYTES {
        return None;
    }
    if &bytes[..4] != DDS_MAGIC
        || word(bytes, 4) != DDS_HEADER_SIZE
        || word(bytes, 76) != PIXEL_FORMAT_SIZE
        || word(bytes, 80) & DDPF_FOURCC == 0
        || &bytes[84..88] != b"DXT1"
    {
        return None;
    }
    let (height, width, mipmaps) = (word(bytes, 12), word(bytes, 16), word(bytes, 28));
    if !(1..=MAX_DDS_SIDE).contains(&width)
        || !(1..=MAX_DDS_SIDE).contains(&height)
        || mipmaps > MAX_DDS_MIPMAPS
    {
        return None;
    }
    let blocks = width.div_ceil(4) as usize * height.div_ceil(4) as usize;
    (bytes.len() - DATA_OFFSET >= blocks * BC1_BLOCK_BYTES).then_some((width, height))
}

/// Decodes the first mipmap level of a BC1 DDS (see [`bc1_dimensions`] for what is accepted).
/// The fourth color of a three-color block is transparent black; the loading screens are opaque,
/// so black is what shows.
pub(crate) fn decode_bc1(bytes: &[u8]) -> Option<Rgb> {
    let (width, height) = bc1_dimensions(bytes)?;
    let (w, h) = (width as usize, height as usize);
    let mut pixels = vec![0u8; w * h * 3];
    let blocks_wide = w.div_ceil(4);
    let mut block_rgba = [0u8; 4 * 4 * 4];
    for (index, block) in bytes[DATA_OFFSET..]
        .as_chunks::<BC1_BLOCK_BYTES>()
        .0
        .iter()
        .take(blocks_wide * h.div_ceil(4))
        .enumerate()
    {
        bcdec_rs::bc1(block, &mut block_rgba, 4 * 4);
        let (left, top) = ((index % blocks_wide) * 4, (index / blocks_wide) * 4);
        for pixel in 0..16 {
            let (x, y) = (left + pixel % 4, top + pixel / 4);
            if x < w && y < h {
                let at = (y * w + x) * 3;
                pixels[at..at + 3].copy_from_slice(&block_rgba[pixel * 4..pixel * 4 + 3]);
            }
        }
    }
    Some(Rgb {
        width,
        height,
        pixels,
    })
}

/// The size `cover` would show for a `width`×`height` box: the box itself when the source is
/// large enough, otherwise the same proportions at the source's own scale (never scaled up).
pub(crate) fn cover_size(source: (u32, u32), width: u32, height: u32) -> (u32, u32) {
    let scale =
        (f64::from(width) / f64::from(source.0)).max(f64::from(height) / f64::from(source.1));
    if scale <= 1.0 {
        return (width, height);
    }
    (
        ((f64::from(width) / scale).round() as u32).clamp(1, source.0),
        ((f64::from(height) / scale).round() as u32).clamp(1, source.1),
    )
}

/// Crops the middle of `source` to the proportions of `width`×`height` (like CSS `cover` with
/// `center`) and scales it down with a box filter; never up (see [`cover_size`]).
pub(crate) fn cover(source: &Rgb, width: u32, height: u32) -> Rgb {
    let (out_w, out_h) = cover_size((source.width, source.height), width, height);
    let scale = (f64::from(out_w) / f64::from(source.width))
        .max(f64::from(out_h) / f64::from(source.height));
    let crop_w = f64::from(out_w) / scale;
    let crop_h = f64::from(out_h) / scale;
    let left = (f64::from(source.width) - crop_w) / 2.0;
    let top = (f64::from(source.height) - crop_h) / 2.0;
    // Source columns (rows) covered by each output column (row): [start, end).
    let spans = |count: u32, offset: f64, crop: f64, limit: u32| -> Vec<(usize, usize)> {
        (0..count)
            .map(|index| {
                let start = offset + crop * f64::from(index) / f64::from(count);
                let end = offset + crop * f64::from(index + 1) / f64::from(count);
                let first = (start.floor() as u32).min(limit - 1);
                let last = (end.ceil() as u32).clamp(first + 1, limit);
                (first as usize, last as usize)
            })
            .collect()
    };
    let columns = spans(out_w, left, crop_w, source.width);
    let rows = spans(out_h, top, crop_h, source.height);
    let stride = source.width as usize * 3;
    let mut pixels = Vec::with_capacity(out_w as usize * out_h as usize * 3);
    for &(row_start, row_end) in &rows {
        for &(column_start, column_end) in &columns {
            let mut sum = [0u64; 3];
            for row in row_start..row_end {
                let line = &source.pixels[row * stride..(row + 1) * stride];
                for column in column_start..column_end {
                    for channel in 0..3 {
                        sum[channel] += u64::from(line[column * 3 + channel]);
                    }
                }
            }
            let count = ((row_end - row_start) * (column_end - column_start)) as u64;
            for total in sum {
                pixels.push(((total + count / 2) / count) as u8);
            }
        }
    }
    Rgb {
        width: out_w,
        height: out_h,
        pixels,
    }
}

/// An RGB PNG with fast compression.
pub(crate) fn encode_png(picture: &Rgb) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, picture.width, picture.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&picture.pixels).ok()?;
    writer.finish().ok()?;
    Some(bytes)
}

/// A BC1 DDS cropped and scaled to `width`×`height` device pixels (never up) as PNG, or `None`
/// when the DDS is not accepted or the size is outside 1..=4096.
pub(crate) fn dds_to_png(bytes: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    if !(1..=MAX_OUTPUT_SIDE).contains(&width) || !(1..=MAX_OUTPUT_SIDE).contains(&height) {
        return None;
    }
    encode_png(&cover(&decode_bc1(bytes)?, width, height))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic BC1 DDS: `blocks` gives (c0, c1, codes) per 4×4 block, row by row.
    pub(crate) fn dds(
        width: u32,
        height: u32,
        mipmaps: u32,
        blocks: &[(u16, u16, u32)],
    ) -> Vec<u8> {
        let mut bytes = vec![0u8; DATA_OFFSET];
        bytes[..4].copy_from_slice(DDS_MAGIC);
        bytes[4..8].copy_from_slice(&DDS_HEADER_SIZE.to_le_bytes());
        bytes[8..12].copy_from_slice(&0x0008_1007u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&height.to_le_bytes());
        bytes[16..20].copy_from_slice(&width.to_le_bytes());
        bytes[28..32].copy_from_slice(&mipmaps.to_le_bytes());
        bytes[76..80].copy_from_slice(&PIXEL_FORMAT_SIZE.to_le_bytes());
        bytes[80..84].copy_from_slice(&DDPF_FOURCC.to_le_bytes());
        bytes[84..88].copy_from_slice(b"DXT1");
        for (c0, c1, codes) in blocks {
            bytes.extend_from_slice(&c0.to_le_bytes());
            bytes.extend_from_slice(&c1.to_le_bytes());
            bytes.extend_from_slice(&codes.to_le_bytes());
        }
        bytes
    }

    /// One flat color per block.
    pub(crate) fn flat_dds(width: u32, height: u32, color: u16) -> Vec<u8> {
        let blocks = (width.div_ceil(4) * height.div_ceil(4)) as usize;
        dds(width, height, 1, &vec![(color, color, 0); blocks])
    }

    const RED: u16 = 0xf800;
    const GREEN: u16 = 0x07e0;
    const BLUE: u16 = 0x001f;
    const WHITE: u16 = 0xffff;

    fn pixel(picture: &Rgb, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * picture.width + x) * 3) as usize;
        picture.pixels[at..at + 3].try_into().unwrap()
    }

    #[test]
    fn blocks_decode_in_row_order_with_two_bit_codes_least_significant_first() {
        // Codes: pixel 0 → 0 (c0), pixel 1 → 1 (c1), pixel 2 → 2, pixel 3 → 3, row 2 all 1.
        let codes = 0b11_10_01_00 | (0x55 << 8);
        let bytes = dds(8, 4, 1, &[(WHITE, 0, codes), (RED, RED, 0)]);
        let picture = decode_bc1(&bytes).unwrap();
        assert_eq!((picture.width, picture.height), (8, 4));
        assert_eq!(pixel(&picture, 0, 0), [255, 255, 255]);
        assert_eq!(pixel(&picture, 1, 0), [0, 0, 0]);
        assert_eq!(pixel(&picture, 2, 0), [170, 170, 170]);
        assert_eq!(pixel(&picture, 3, 0), [85, 85, 85]);
        assert_eq!(pixel(&picture, 0, 1), [0, 0, 0]);
        assert_eq!(pixel(&picture, 3, 1), [0, 0, 0]);
        assert_eq!(pixel(&picture, 0, 2), [255, 255, 255]);
        assert_eq!(pixel(&picture, 4, 0), [255, 0, 0]);
        assert_eq!(pixel(&picture, 7, 3), [255, 0, 0]);
    }

    #[test]
    fn sides_not_divisible_by_four_use_partial_blocks() {
        // 6×5: 2×2 blocks; the bottom-right block holds the pixel (5, 4).
        let bytes = dds(
            6,
            5,
            1,
            &[
                (RED, RED, 0),
                (GREEN, GREEN, 0),
                (BLUE, BLUE, 0),
                (WHITE, WHITE, 0),
            ],
        );
        let picture = decode_bc1(&bytes).unwrap();
        assert_eq!(picture.pixels.len(), 6 * 5 * 3);
        assert_eq!(pixel(&picture, 3, 3), [255, 0, 0]);
        assert_eq!(pixel(&picture, 4, 0), [0, 255, 0]);
        assert_eq!(pixel(&picture, 0, 4), [0, 0, 255]);
        assert_eq!(pixel(&picture, 5, 4), [255, 255, 255]);
    }

    #[test]
    fn header_fields_and_limits_are_checked_before_allocation() {
        let good = flat_dds(8, 8, RED);
        assert_eq!(bc1_dimensions(&good), Some((8, 8)));
        let with = |at: usize, value: &[u8]| {
            let mut bytes = good.clone();
            bytes[at..at + value.len()].copy_from_slice(value);
            bytes
        };
        for (at, value, why) in [
            (0, &b"DDs "[..], "magic"),
            (4, &125u32.to_le_bytes()[..], "header size"),
            (76, &31u32.to_le_bytes()[..], "pixel format size"),
            (80, &0x40u32.to_le_bytes()[..], "no FourCC flag"),
            (84, &b"DXT5"[..], "other format"),
            (84, &b"DX10"[..], "DX10 header"),
            (16, &0u32.to_le_bytes()[..], "zero width"),
            (12, &0u32.to_le_bytes()[..], "zero height"),
            (16, &(MAX_DDS_SIDE + 1).to_le_bytes()[..], "too wide"),
            (12, &(MAX_DDS_SIDE + 1).to_le_bytes()[..], "too high"),
            (
                28,
                &(MAX_DDS_MIPMAPS + 1).to_le_bytes()[..],
                "too many mipmaps",
            ),
        ] {
            assert_eq!(bc1_dimensions(&with(at, value)), None, "{why}");
        }
        // Limits themselves are accepted.
        assert!(bc1_dimensions(&with(28, &MAX_DDS_MIPMAPS.to_le_bytes())).is_some());
        assert_eq!(
            bc1_dimensions(&flat_dds(MAX_DDS_SIDE, 4, RED)),
            Some((MAX_DDS_SIDE, 4))
        );
        assert_eq!(
            bc1_dimensions(&flat_dds(4, MAX_DDS_SIDE, RED)),
            Some((4, MAX_DDS_SIDE))
        );
        // A huge declared size with little data is refused without allocating it.
        let huge = with(16, &MAX_DDS_SIDE.to_le_bytes());
        let huge = {
            let mut bytes = huge;
            bytes[12..16].copy_from_slice(&MAX_DDS_SIDE.to_le_bytes());
            bytes
        };
        assert_eq!(decode_bc1(&huge), None);
        // The file size limit.
        let mut too_big = flat_dds(8, 8, RED);
        too_big.resize(usize::try_from(MAX_DDS_BYTES).unwrap() + 1, 0);
        assert_eq!(bc1_dimensions(&too_big), None);
        too_big.truncate(usize::try_from(MAX_DDS_BYTES).unwrap());
        assert_eq!(bc1_dimensions(&too_big), Some((8, 8)));
    }

    #[test]
    fn truncated_files_at_every_length_are_refused() {
        let full = flat_dds(8, 8, GREEN);
        for length in 0..full.len() {
            assert_eq!(decode_bc1(&full[..length]), None, "length {length}");
        }
        assert!(decode_bc1(&full).is_some());
    }

    #[test]
    fn deterministic_mutations_never_panic_and_results_are_deterministic() {
        let fixture = dds(
            6,
            5,
            1,
            &[
                (RED, GREEN, 0x1234_5678),
                (GREEN, RED, 0x9abc_def0),
                (BLUE, WHITE, 0xffff_0000),
                (WHITE, BLUE, 0x0f0f_0f0f),
            ],
        );
        let mut inputs = vec![Vec::new(), vec![0; 256], fixture.clone()];
        for index in 0..fixture.len() {
            let mut mutated = fixture.clone();
            mutated[index] ^= (index as u8).wrapping_mul(31).wrapping_add(1);
            inputs.push(mutated);
        }
        let mut state = 0xC0FF_EE12_u32;
        for _ in 0..256 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let mut random = fixture[..DATA_OFFSET].to_vec();
            // Random small sizes and random block data.
            random[12..16].copy_from_slice(&(state % 64).to_le_bytes());
            random[16..20].copy_from_slice(&((state >> 8) % 64).to_le_bytes());
            let length = (state as usize >> 16) % 2_048;
            for _ in 0..length {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                random.push((state >> 24) as u8);
            }
            inputs.push(random);
        }
        for input in inputs {
            let first = std::panic::catch_unwind(|| {
                (
                    decode_bc1(&input),
                    dds_to_png(&input, 7, 3),
                    dds_to_png(&input, 300, 2),
                )
            });
            assert!(first.is_ok());
            let first = first.unwrap();
            assert_eq!(first.0, decode_bc1(&input));
            assert_eq!(first.1, dds_to_png(&input, 7, 3));
        }
    }

    fn stripes(width: u32, height: u32) -> Rgb {
        // Column x has the gray value x; handy for checking crops and averages.
        let mut pixels = Vec::new();
        for _ in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&[x as u8, 0, (255 - x) as u8]);
            }
        }
        Rgb {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn cover_size_never_scales_up() {
        assert_eq!(cover_size((2560, 1600), 1280, 400), (1280, 400));
        assert_eq!(cover_size((2560, 1600), 2560, 1600), (2560, 1600));
        // A 4000×100 strip from a 2560-wide source: the source's own pixels, same proportions.
        assert_eq!(cover_size((2560, 1600), 4000, 100), (2560, 64));
        // Too high for the source: limited by height.
        assert_eq!(cover_size((500, 235), 400, 470), (200, 235));
        assert_eq!(cover_size((500, 235), 1, 1), (1, 1));
    }

    #[test]
    fn limits_have_their_documented_values() {
        assert_eq!(MAX_DDS_BYTES, 4_194_304);
        assert_eq!(
            (MAX_DDS_SIDE, MAX_DDS_MIPMAPS, MAX_OUTPUT_SIDE),
            (4096, 13, 4096)
        );
    }

    #[test]
    fn cover_crops_rows_from_the_vertical_middle_when_scaling_down() {
        // 8×8, green = row²: a 2×1 box takes rows 2..6 (scale 1/4) and averages them.
        let mut source = stripes(8, 8);
        for (index, value) in source.pixels.iter_mut().enumerate() {
            if index % 3 == 1 {
                let row = (index / 24) as u8;
                *value = row * row;
            }
        }
        let picture = cover(&source, 2, 1);
        assert_eq!((picture.width, picture.height), (2, 1));
        // (4 + 9 + 16 + 25) / 4 = 13.5, rounded up.
        assert_eq!(pixel(&picture, 0, 0)[1], 14);
        assert_eq!(pixel(&picture, 1, 0)[1], 14);
    }

    #[test]
    fn cover_crops_the_middle_and_averages_boxes() {
        let source = stripes(8, 4);
        // Half the width, full height scale 1: the middle four columns.
        let middle = cover(&source, 4, 4);
        assert_eq!((middle.width, middle.height), (4, 4));
        assert_eq!(
            (0..4).map(|x| pixel(&middle, x, 0)[0]).collect::<Vec<_>>(),
            [2, 3, 4, 5]
        );
        // Scaled to half: pairs of columns averaged (0,1) → 1 (rounded), (2,3) → 3 …
        let half = cover(&source, 4, 2);
        assert_eq!((half.width, half.height), (4, 2));
        assert_eq!(
            (0..4).map(|x| pixel(&half, x, 1)[0]).collect::<Vec<_>>(),
            [1, 3, 5, 7]
        );
        assert_eq!(pixel(&half, 0, 0), [1, 0, 255]);
        // A wide strip from the vertical middle.
        let mut rows = stripes(4, 4);
        for (index, value) in rows.pixels.iter_mut().enumerate() {
            if index % 3 == 1 {
                *value = (index / 12) as u8 * 10; // green = 10 × row
            }
        }
        let strip = cover(&rows, 4, 2);
        assert_eq!(
            (0..2).map(|y| pixel(&strip, 0, y)[1]).collect::<Vec<_>>(),
            [10, 20]
        );
        // One pixel from everything: the average.
        assert_eq!(pixel(&cover(&source, 1, 1), 0, 0)[0], 4);
    }

    /// Size and RGB pixels of a PNG, read back with the same crate.
    pub(crate) fn read_png(bytes: &[u8]) -> Rgb {
        let decoder = png::Decoder::new(bytes);
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgb);
        pixels.truncate(info.buffer_size());
        Rgb {
            width: info.width,
            height: info.height,
            pixels,
        }
    }

    #[test]
    fn png_round_trips_the_pixels() {
        let picture = Rgb {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        };
        let bytes = encode_png(&picture).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
        assert_eq!(read_png(&bytes), picture);
    }

    #[test]
    fn dds_to_png_checks_the_requested_size() {
        let bytes = flat_dds(16, 8, RED);
        let png = read_png(&dds_to_png(&bytes, 8, 4).unwrap());
        assert_eq!((png.width, png.height), (8, 4));
        assert_eq!(&png.pixels[..3], &[255, 0, 0]);
        // Never larger than the source.
        let png = read_png(&dds_to_png(&bytes, 32, 16).unwrap());
        assert_eq!((png.width, png.height), (16, 8));
        for (width, height) in [
            (0, 4),
            (4, 0),
            (MAX_OUTPUT_SIDE + 1, 4),
            (4, MAX_OUTPUT_SIDE + 1),
        ] {
            assert_eq!(dds_to_png(&bytes, width, height), None, "{width}×{height}");
        }
        assert!(dds_to_png(&bytes, MAX_OUTPUT_SIDE, MAX_OUTPUT_SIDE).is_some());
        assert_eq!(dds_to_png(b"not a dds", 4, 4), None);
    }
}
