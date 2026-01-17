use image::{Rgba, RgbaImage};
use nalgebra::Matrix3;

use crate::detection::find_sprite_bounds;
use crate::geometry::{compute_output_bounds, transform_point};

/// Premultiply alpha: RGB values are multiplied by alpha
fn premultiply_alpha(img: &RgbaImage) -> Vec<[f64; 4]> {
    let (width, height) = img.dimensions();
    let mut result = Vec::with_capacity((width * height) as usize);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            let alpha = pixel[3] as f64 / 255.0;
            result.push([
                pixel[0] as f64 * alpha,
                pixel[1] as f64 * alpha,
                pixel[2] as f64 * alpha,
                pixel[3] as f64,
            ]);
        }
    }

    result
}

/// Unpremultiply alpha: divide RGB by alpha
fn unpremultiply_alpha(premultiplied: [f64; 4]) -> Rgba<u8> {
    let alpha = premultiplied[3];
    if alpha < 1.0 {
        return Rgba([0, 0, 0, 0]);
    }

    let alpha_norm = alpha / 255.0;
    let r = (premultiplied[0] / alpha_norm).clamp(0.0, 255.0) as u8;
    let g = (premultiplied[1] / alpha_norm).clamp(0.0, 255.0) as u8;
    let b = (premultiplied[2] / alpha_norm).clamp(0.0, 255.0) as u8;
    let a = alpha.clamp(0.0, 255.0) as u8;

    Rgba([r, g, b, a])
}

/// Cubic interpolation kernel (Catmull-Rom)
fn cubic_weight(t: f64) -> [f64; 4] {
    let t2 = t * t;
    let t3 = t2 * t;

    [
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    ]
}

/// Bicubic interpolation at a given position
fn bicubic_interpolate(
    premultiplied: &[[f64; 4]],
    width: u32,
    height: u32,
    x: f64,
    y: f64,
) -> [f64; 4] {
    let x_floor = x.floor() as i32;
    let y_floor = y.floor() as i32;
    let x_frac = x - x.floor();
    let y_frac = y - y.floor();

    let wx = cubic_weight(x_frac);
    let wy = cubic_weight(y_frac);

    let mut result = [0.0; 4];

    for j in 0..4 {
        for i in 0..4 {
            let px = (x_floor + i as i32 - 1).clamp(0, width as i32 - 1) as u32;
            let py = (y_floor + j as i32 - 1).clamp(0, height as i32 - 1) as u32;
            let idx = (py * width + px) as usize;

            let weight = wx[i] * wy[j];
            for c in 0..4 {
                result[c] += premultiplied[idx][c] * weight;
            }
        }
    }

    result
}

/// Bilinear interpolation (faster, available for edge cleaning)
#[allow(dead_code)]
fn bilinear_interpolate(
    premultiplied: &[[f64; 4]],
    width: u32,
    height: u32,
    x: f64,
    y: f64,
) -> [f64; 4] {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let x_frac = x - x.floor();
    let y_frac = y - y.floor();

    let get_pixel = |px: i32, py: i32| -> [f64; 4] {
        let px = px.clamp(0, width as i32 - 1) as u32;
        let py = py.clamp(0, height as i32 - 1) as u32;
        premultiplied[(py * width + px) as usize]
    };

    let p00 = get_pixel(x0, y0);
    let p10 = get_pixel(x1, y0);
    let p01 = get_pixel(x0, y1);
    let p11 = get_pixel(x1, y1);

    let mut result = [0.0; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - x_frac) + p10[c] * x_frac;
        let bottom = p01[c] * (1.0 - x_frac) + p11[c] * x_frac;
        result[c] = top * (1.0 - y_frac) + bottom * y_frac;
    }

    result
}

/// Apply an affine transformation to an image using inverse mapping
pub fn apply_affine_transform(
    img: &RgbaImage,
    forward_matrix: &Matrix3<f64>,
    verbose: bool,
) -> RgbaImage {
    let (src_width, src_height) = img.dimensions();

    // Compute output dimensions
    let (new_width, new_height, offset_x, offset_y) =
        compute_output_bounds(forward_matrix, src_width, src_height);

    // Ensure reasonable dimensions
    let new_width = new_width.max(1).min(src_width * 3);
    let new_height = new_height.max(1).min(src_height * 3);

    if verbose {
        eprintln!(
            "Transform: {}x{} -> {}x{} (offset: {:.1}, {:.1})",
            src_width, src_height, new_width, new_height, offset_x, offset_y
        );
    }

    // Compute inverse matrix for backward mapping
    let inverse_matrix = match forward_matrix.try_inverse() {
        Some(inv) => inv,
        None => {
            eprintln!("Warning: Could not invert transform matrix, returning original image");
            return img.clone();
        }
    };

    // Pre-multiply alpha for correct interpolation
    let premultiplied = premultiply_alpha(img);

    // Create output image
    let mut output = RgbaImage::new(new_width, new_height);

    // Apply inverse mapping with bicubic interpolation
    for out_y in 0..new_height {
        for out_x in 0..new_width {
            // Map output pixel to source coordinates
            let dst_x = out_x as f64 + offset_x;
            let dst_y = out_y as f64 + offset_y;
            let (src_x, src_y) = transform_point(&inverse_matrix, dst_x, dst_y);

            // Check if source is within bounds (with some margin for interpolation)
            if src_x >= -1.0
                && src_x <= src_width as f64
                && src_y >= -1.0
                && src_y <= src_height as f64
            {
                let interpolated =
                    bicubic_interpolate(&premultiplied, src_width, src_height, src_x, src_y);
                let pixel = unpremultiply_alpha(interpolated);
                output.put_pixel(out_x, out_y, pixel);
            } else {
                output.put_pixel(out_x, out_y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    // Clean up edge artifacts
    clean_edges(&mut output)
}

/// Remove edge artifacts by cleaning up semi-transparent edge pixels
fn clean_edges(img: &mut RgbaImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut result = img.clone();

    // Simple artifact removal: if a pixel has very low alpha but neighbors are transparent,
    // make it fully transparent
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let pixel = img.get_pixel(x, y);

            // Check for semi-transparent edge pixels
            if pixel[3] > 0 && pixel[3] < 32 {
                // Count transparent neighbors
                let neighbors = [
                    img.get_pixel(x - 1, y),
                    img.get_pixel(x + 1, y),
                    img.get_pixel(x, y - 1),
                    img.get_pixel(x, y + 1),
                ];

                let transparent_count = neighbors.iter().filter(|p| p[3] == 0).count();

                // If mostly surrounded by transparent pixels, make this transparent too
                if transparent_count >= 3 {
                    result.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }
    }

    result
}

/// Crop image to its non-transparent content (removes padding)
pub fn crop_to_content(img: &RgbaImage) -> RgbaImage {
    let bounds = match find_sprite_bounds(img, 10) {
        Some(b) => b,
        None => return img.clone(), // Return original if fully transparent
    };

    let (min_x, min_y, width, height) = bounds;
    let mut cropped = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(min_x + x, min_y + y);
            cropped.put_pixel(x, y, *pixel);
        }
    }

    cropped
}

/// Resize image so that the longest side equals target_size
/// Uses bicubic interpolation for quality
pub fn resize_to_fit(img: &RgbaImage, target_size: u32) -> RgbaImage {
    let (width, height) = img.dimensions();

    if width == 0 || height == 0 {
        return img.clone();
    }

    let scale = target_size as f64 / width.max(height) as f64;
    let new_width = ((width as f64 * scale).round() as u32).max(1);
    let new_height = ((height as f64 * scale).round() as u32).max(1);

    // Premultiply alpha for correct interpolation
    let premultiplied = premultiply_alpha(img);

    let mut output = RgbaImage::new(new_width, new_height);

    for out_y in 0..new_height {
        for out_x in 0..new_width {
            // Map output coordinates to source coordinates
            let src_x = (out_x as f64 + 0.5) / scale - 0.5;
            let src_y = (out_y as f64 + 0.5) / scale - 0.5;

            let interpolated = bicubic_interpolate(&premultiplied, width, height, src_x, src_y);
            let pixel = unpremultiply_alpha(interpolated);
            output.put_pixel(out_x, out_y, pixel);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_premultiply_unpremultiply() {
        let pixel = Rgba([200, 100, 50, 128]);
        let img = RgbaImage::from_pixel(1, 1, pixel);
        let premul = premultiply_alpha(&img);

        let unpremul = unpremultiply_alpha(premul[0]);
        // Should be close to original (some rounding error expected)
        assert!((unpremul[0] as i32 - pixel[0] as i32).abs() <= 1);
        assert!((unpremul[1] as i32 - pixel[1] as i32).abs() <= 1);
        assert!((unpremul[2] as i32 - pixel[2] as i32).abs() <= 1);
        assert_eq!(unpremul[3], pixel[3]);
    }

    #[test]
    fn test_identity_transform() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let identity = Matrix3::identity();
        let result = apply_affine_transform(&img, &identity, false);

        // Should preserve dimensions and colors
        assert_eq!(result.dimensions(), (10, 10));
        let center = result.get_pixel(5, 5);
        assert_eq!(center[0], 255);
        assert_eq!(center[3], 255);
    }
}
