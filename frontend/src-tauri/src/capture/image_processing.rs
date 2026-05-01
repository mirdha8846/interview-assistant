use image::{DynamicImage, GrayImage, ImageBuffer, Luma};

/// Prepares an image for OCR by converting to grayscale and applying thresholding
pub fn preprocess_for_ocr(image: &DynamicImage) -> GrayImage {
    let gray = to_grayscale(image);
    // Apply thresholding to make text distinct (black/white)
    let processed = threshold(&gray, 150); 
    processed
}

/// Converts any image to grayscale (Luma8)
fn to_grayscale(image: &DynamicImage) -> GrayImage {
    image.to_luma8()
}

/// Applies a simple binary threshold
/// Pixels brighter than `threshold` become white, others become black
fn threshold(image: &GrayImage, threshold: u8) -> GrayImage {
    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);
    
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            let val = if pixel[0] > threshold { 255 } else { 0 };
            out.put_pixel(x, y, Luma([val]));
        }
    }
    out
}

/// Simple contrast enhancement (linear stretch)
pub fn enhance_contrast(image: &GrayImage) -> GrayImage {
    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);

    // Find min and max pixel values
    let mut min_val = 255u8;
    let mut max_val = 0u8;

    for pixel in image.pixels() {
        let val = pixel[0];
        if val < min_val { min_val = val; }
        if val > max_val { max_val = val; }
    }

    // Avoid division by zero
    if max_val == min_val {
        return image.clone();
    }

    // Apply linear stretch
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            let val = pixel[0];
            let new_val = ((val as f32 - min_val as f32) / (max_val as f32 - min_val as f32) * 255.0) as u8;
            out.put_pixel(x, y, Luma([new_val]));
        }
    }
    out
}
