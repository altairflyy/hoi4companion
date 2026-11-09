//! Bounded read-only texture decoding. BCn uses image_dds/bcdec; no image
//! interpreter or external process remains resident for sprite conversion.
use image::{DynamicImage, ImageFormat, ImageReader, RgbaImage};
use std::io::Cursor;
type Result<T> = std::result::Result<T, String>;
fn u32_at(raw: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        raw.get(offset..offset + 4)
            .ok_or("Truncated DDS header")?
            .try_into()
            .unwrap(),
    ))
}
fn dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 8_000_000 {
        Err("Texture dimensions exceed limit".into())
    } else {
        Ok(())
    }
}
pub fn decode(raw: &[u8]) -> Result<RgbaImage> {
    if raw.len() > 32_000_000 {
        return Err("Texture exceeds limit".into());
    }
    if raw.starts_with(b"DDS ") {
        let height = u32_at(raw, 12)?;
        let width = u32_at(raw, 16)?;
        dimensions(width, height)?;
        if raw.get(84..88) == Some(b"DX10")
            && [87, 91].contains(&u32_at(raw, 128)?)
            && u32_at(raw, 132)? == 3
            && u32_at(raw, 140)? == 1
            && u32_at(raw, 136)? & 4 == 0
        {
            let stride = u32_at(raw, 20)?.max(width.checked_mul(4).ok_or("Texture width overflow")?)
                as usize;
            let size = stride
                .checked_mul(height as usize)
                .ok_or("Texture pitch overflow")?;
            let data = raw
                .get(148..)
                .filter(|data| data.len() >= size)
                .ok_or("Truncated DDS texture")?;
            let mut image = RgbaImage::new(width, height);
            for y in 0..height {
                for x in 0..width {
                    let i = y as usize * stride + x as usize * 4;
                    image.put_pixel(
                        x,
                        y,
                        image::Rgba([data[i + 2], data[i + 1], data[i], data[i + 3]]),
                    );
                }
            }
            return Ok(image);
        }
        let dds =
            image_dds::ddsfile::Dds::read(&mut Cursor::new(raw)).map_err(|e| e.to_string())?;
        let mut surface = image_dds::Surface::from_dds(&dds).map_err(|e| e.to_string())?;
        // Like the existing image decoder, an icon selects the first base-level
        // face/slice. Do not allocate or decode other array layers or mip levels.
        surface.depth = 1;
        return surface
            .decode_layers_mipmaps_rgba8(0..1, 0..1)
            .map_err(|e| e.to_string())?
            .into_image()
            .map_err(|e| e.to_string());
    }
    let reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let format = reader
        .format()
        .or_else(|| {
            (raw.len() >= 18
                && [1, 2, 3, 9, 10, 11].contains(&raw[2])
                && raw[1] <= 1
                && [8, 15, 16, 24, 32].contains(&raw[16]))
            .then_some(ImageFormat::Tga)
        })
        .ok_or("Unknown texture format")?;
    if ![ImageFormat::Png, ImageFormat::Tga, ImageFormat::Bmp].contains(&format) {
        return Err("Unsupported texture format".into());
    }
    let (width, height) = ImageReader::with_format(Cursor::new(raw), format)
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    dimensions(width, height)?;
    let mut reader = ImageReader::with_format(Cursor::new(raw), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64_000_000);
    reader.limits(limits);
    reader
        .decode()
        .map(|image| image.into_rgba8())
        .map_err(|e| e.to_string())
}
pub fn png(raw: &[u8], frames: u32, frame: u32) -> Result<Vec<u8>> {
    let image = decode(raw)?;
    if !(1..=256).contains(&frames)
        || !(1..=frames).contains(&frame)
        || !image.width().is_multiple_of(frames)
    {
        return Err("Unsupported texture frame geometry".into());
    }
    let width = image.width() / frames;
    let crop =
        image::imageops::crop_imm(&image, (frame - 1) * width, 0, width, image.height()).to_image();
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(crop)
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(output.into_inner())
}
