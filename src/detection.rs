use anyhow::{Context, Result};
use image::{DynamicImage, GrayImage, RgbaImage};
use imageproc::edges::canny;
use imageproc::hough::{detect_lines, LineDetectionOptions, PolarLine};

use crate::geometry::DetectedAngles;

/// Result of the detection pipeline
#[derive(Debug)]
pub struct DetectedGeometry {
    /// Detected isometric angles
    pub angles: DetectedAngles,
    /// Bounding box of the sprite (x, y, width, height)
    pub bounds: (u32, u32, u32, u32),
    /// Center point of the sprite
    pub center: (f64, f64),
    /// Number of lines detected
    pub line_count: usize,
}

/// A detected line with its properties
#[derive(Debug, Clone)]
struct DetectedLine {
    angle_degrees: f64,
    length: f64,
}

/// Find the non-transparent bounding box of a sprite
pub fn find_sprite_bounds(img: &RgbaImage, alpha_threshold: u8) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = img.dimensions();
    let mut min_x = width;
    let mut max_x = 0;
    let mut min_y = height;
    let mut max_y = 0;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel[3] >= alpha_threshold {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }

    if min_x <= max_x && min_y <= max_y {
        Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
    } else {
        None
    }
}

/// Convert RGBA image to grayscale, using alpha to mask out transparent pixels
fn to_grayscale_masked(img: &RgbaImage, alpha_threshold: u8) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut gray = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if pixel[3] >= alpha_threshold {
                // Standard luminance conversion
                let luma = (0.299 * pixel[0] as f64
                    + 0.587 * pixel[1] as f64
                    + 0.114 * pixel[2] as f64) as u8;
                gray.put_pixel(x, y, image::Luma([luma]));
            } else {
                // Transparent pixels become white (background)
                gray.put_pixel(x, y, image::Luma([255]));
            }
        }
    }

    gray
}

/// Apply Canny edge detection
fn detect_edges(gray: &GrayImage, low_threshold: f32, high_threshold: f32) -> GrayImage {
    canny(gray, low_threshold, high_threshold)
}

/// Convert polar line representation to angle in degrees
fn polar_to_angle_degrees(line: &PolarLine) -> f64 {
    // In Hough space, angle is perpendicular to the line direction
    // We need to convert to the actual line angle
    let theta_rad = line.angle_in_degrees as f64 * std::f64::consts::PI / 180.0;
    // The line direction is perpendicular to the normal
    let line_angle = theta_rad - std::f64::consts::FRAC_PI_2;
    let mut degrees = line_angle.to_degrees();

    // Normalize to -90 to +90 range
    while degrees > 90.0 {
        degrees -= 180.0;
    }
    while degrees < -90.0 {
        degrees += 180.0;
    }

    degrees
}

/// Estimate line length based on edge image and line parameters
fn estimate_line_length(edges: &GrayImage, line: &PolarLine) -> f64 {
    let (width, height) = edges.dimensions();
    let theta = (line.angle_in_degrees as f64).to_radians();
    let r = line.r as f64;

    let cos_t = theta.cos();
    let sin_t = theta.sin();

    let mut count = 0;

    // Sample points along the line
    if sin_t.abs() > cos_t.abs() {
        // More horizontal line - iterate over x
        for x in 0..width {
            let y = ((r - x as f64 * cos_t) / sin_t) as i32;
            if y >= 0 && y < height as i32 {
                let pixel = edges.get_pixel(x, y as u32);
                if pixel[0] > 0 {
                    count += 1;
                }
            }
        }
    } else {
        // More vertical line - iterate over y
        for y in 0..height {
            let x = ((r - y as f64 * sin_t) / cos_t) as i32;
            if x >= 0 && x < width as i32 {
                let pixel = edges.get_pixel(x as u32, y);
                if pixel[0] > 0 {
                    count += 1;
                }
            }
        }
    }

    count as f64
}

/// Classify lines into left-sloping and right-sloping groups
fn classify_lines(lines: &[DetectedLine]) -> (Vec<&DetectedLine>, Vec<&DetectedLine>) {
    let mut left_sloping = Vec::new(); // Negative angles (-60° to -15°)
    let mut right_sloping = Vec::new(); // Positive angles (15° to 60°)

    for line in lines {
        let angle = line.angle_degrees;
        if angle >= -60.0 && angle <= -15.0 {
            left_sloping.push(line);
        } else if angle >= 15.0 && angle <= 60.0 {
            right_sloping.push(line);
        }
        // Lines outside these ranges are ignored (horizontal/vertical)
    }

    (left_sloping, right_sloping)
}

/// Compute weighted median of angles
fn weighted_median(lines: &[&DetectedLine]) -> Option<(f64, f64)> {
    if lines.is_empty() {
        return None;
    }

    let total_weight: f64 = lines.iter().map(|l| l.length).sum();
    if total_weight == 0.0 {
        return None;
    }

    // Sort by angle
    let mut sorted: Vec<_> = lines.iter().map(|l| (l.angle_degrees, l.length)).collect();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Find weighted median
    let mut cumulative = 0.0;
    let half_weight = total_weight / 2.0;

    for (angle, weight) in &sorted {
        cumulative += weight;
        if cumulative >= half_weight {
            let confidence = total_weight / (lines.len() as f64 * 100.0);
            return Some((*angle, confidence.min(1.0)));
        }
    }

    // Fallback to simple weighted average
    let weighted_sum: f64 = lines.iter().map(|l| l.angle_degrees * l.length).sum();
    let avg_angle = weighted_sum / total_weight;
    let confidence = total_weight / (lines.len() as f64 * 100.0);

    Some((avg_angle, confidence.min(1.0)))
}

/// Main detection function: analyze an image to find isometric angles
pub fn detect_isometric_angles(img: &DynamicImage, verbose: bool) -> Result<DetectedGeometry> {
    let rgba = img.to_rgba8();

    // Find sprite bounds
    let bounds = find_sprite_bounds(&rgba, 10)
        .context("Could not find sprite bounds - image may be fully transparent")?;

    let center = (
        bounds.0 as f64 + bounds.2 as f64 / 2.0,
        bounds.1 as f64 + bounds.3 as f64 / 2.0,
    );

    if verbose {
        eprintln!("Sprite bounds: {:?}", bounds);
        eprintln!("Sprite center: ({:.1}, {:.1})", center.0, center.1);
    }

    // Convert to grayscale with alpha masking
    let gray = to_grayscale_masked(&rgba, 10);

    // Edge detection with adaptive thresholds
    let edges = detect_edges(&gray, 30.0, 100.0);

    if verbose {
        eprintln!("Applied Canny edge detection (30.0, 100.0)");
    }

    // Hough line detection
    let options = LineDetectionOptions {
        vote_threshold: 40,
        suppression_radius: 8,
    };

    let polar_lines = detect_lines(&edges, options);

    if verbose {
        eprintln!("Detected {} Hough lines", polar_lines.len());
    }

    // Convert to our line representation with estimated lengths
    let detected_lines: Vec<DetectedLine> = polar_lines
        .iter()
        .map(|pl| {
            let angle_degrees = polar_to_angle_degrees(pl);
            let length = estimate_line_length(&edges, pl);
            DetectedLine { angle_degrees, length }
        })
        .collect();

    // Classify into left and right sloping
    let (left_lines, right_lines) = classify_lines(&detected_lines);

    if verbose {
        eprintln!(
            "Classified: {} left-sloping, {} right-sloping lines",
            left_lines.len(),
            right_lines.len()
        );
    }

    // Compute robust angle estimates
    let (left_angle, left_conf) = weighted_median(&left_lines)
        .unwrap_or((-26.565, 0.0)); // Default to ideal if not found

    let (right_angle, right_conf) = weighted_median(&right_lines)
        .unwrap_or((26.565, 0.0)); // Default to ideal if not found

    if verbose {
        eprintln!(
            "Left angle: {:.2}° (confidence: {:.2})",
            left_angle, left_conf
        );
        eprintln!(
            "Right angle: {:.2}° (confidence: {:.2})",
            right_angle, right_conf
        );
    }

    let angles = DetectedAngles::new(left_angle, right_angle, left_conf, right_conf);

    Ok(DetectedGeometry {
        angles,
        bounds,
        center,
        line_count: polar_lines.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_find_bounds_empty() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        assert!(find_sprite_bounds(&img, 10).is_none());
    }

    #[test]
    fn test_find_bounds_full() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        let bounds = find_sprite_bounds(&img, 10).unwrap();
        assert_eq!(bounds, (0, 0, 10, 10));
    }
}
