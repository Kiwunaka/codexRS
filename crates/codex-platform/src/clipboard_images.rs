//! Windows supplies DIB data for bitmap-only clipboard owners as well.

#[cfg(any(windows, test))]
const MAX_CLIPBOARD_BITMAP_BYTES: usize = 128 * 1024 * 1024;
#[cfg(any(windows, test))]
const MAX_CLIPBOARD_PNG_BYTES: usize = 20 * 1024 * 1024;

pub fn read_clipboard_bitmap_png() -> Result<Option<Vec<u8>>, String> {
    #[cfg(windows)]
    {
        use clipboard_win::{Clipboard, formats::CF_DIB, raw};

        if !raw::is_format_avail(CF_DIB) {
            return Ok(None);
        }
        let clipboard = Clipboard::new_attempts(3)
            .map_err(|error| format!("Could not open the image clipboard: {error}"))?;
        let size = raw::size(CF_DIB)
            .ok_or_else(|| "Could not read the clipboard bitmap size.".to_owned())?
            .get();
        if size > MAX_CLIPBOARD_BITMAP_BYTES {
            return Err("The clipboard bitmap exceeds 128 MiB.".to_owned());
        }
        let mut bytes = vec![0; size];
        let read = raw::get(CF_DIB, &mut bytes)
            .map_err(|error| format!("Could not read the clipboard bitmap: {error}"))?;
        bytes.truncate(read);
        drop(clipboard);
        dib_to_png(&bytes).map(Some)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

#[cfg(any(windows, test))]
fn dib_to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Cursor;

    use image::ImageDecoder;

    if bytes.len() > MAX_CLIPBOARD_BITMAP_BYTES {
        return Err("The clipboard bitmap exceeds 128 MiB.".to_owned());
    }
    let decoder = image::codecs::bmp::BmpDecoder::new_without_file_header(Cursor::new(bytes))
        .map_err(|error| format!("Could not decode the clipboard bitmap: {error}"))?;
    if decoder.total_bytes() > MAX_CLIPBOARD_BITMAP_BYTES as u64 {
        return Err("The decoded clipboard bitmap exceeds 128 MiB.".to_owned());
    }
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Could not decode the clipboard bitmap: {error}"))?;
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| format!("Could not encode the clipboard image: {error}"))?;
    let bytes = output.into_inner();
    if bytes.len() > MAX_CLIPBOARD_PNG_BYTES {
        return Err("The clipboard PNG exceeds 20 MiB.".to_owned());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_clipboard_dib_becomes_a_png_attachment() {
        let mut dib = vec![0; 40];
        dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[4..8].copy_from_slice(&2_i32.to_le_bytes());
        dib[8..12].copy_from_slice(&1_i32.to_le_bytes());
        dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib[14..16].copy_from_slice(&24_u16.to_le_bytes());
        dib.extend_from_slice(&[0, 0, 255, 255, 0, 0, 0, 0]);
        let png = dib_to_png(&dib).unwrap_or_else(|error| panic!("DIB conversion failed: {error}"));
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap_or_else(|error| panic!("Clipboard PNG could not be read: {error}"))
            .to_rgb8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 255]);
    }

    #[test]
    fn invalid_dib_is_rejected_without_allocating_image_pixels() {
        assert!(dib_to_png(&[0; 8]).is_err());
        let mut dib = vec![0; 40];
        dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        dib[8..12].copy_from_slice(&i32::MAX.to_le_bytes());
        dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib[14..16].copy_from_slice(&24_u16.to_le_bytes());
        assert!(dib_to_png(&dib).is_err());
    }
}
