//! Bounded deterministic pixel operations. No character/number substitution.
use complyph_extract::{OcrPageProcessing, OcrRotation, OcrSettings};
pub(super) struct Raster {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
}
impl Raster {
    pub fn from_bgra(input: &[u8], width: usize, height: usize) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width > super::MAX_SIDE as usize
            || height > super::MAX_SIDE as usize
            || input.len() != width * height * 4
        {
            return Err("The local page bitmap exceeds its pixel limit.".into());
        }
        Ok(Self {
            width,
            height,
            pixels: input
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| {
                    ((77 * u32::from(p[2]) + 150 * u32::from(p[1]) + 29 * u32::from(p[0])) >> 8)
                        as u8
                })
                .collect(),
        })
    }
    fn cardinal(self, rotation: OcrRotation) -> Self {
        if rotation == OcrRotation::None {
            return self;
        }
        let (w, h) = match rotation {
            OcrRotation::Clockwise90 | OcrRotation::Clockwise270 => (self.height, self.width),
            _ => (self.width, self.height),
        };
        let mut pixels = vec![255; w * h];
        for y in 0..self.height {
            for x in 0..self.width {
                let (a, b) = match rotation {
                    OcrRotation::Clockwise90 => (self.height - 1 - y, x),
                    OcrRotation::Clockwise180 => (self.width - 1 - x, self.height - 1 - y),
                    OcrRotation::Clockwise270 => (y, self.width - 1 - x),
                    OcrRotation::None => (x, y),
                };
                pixels[b * w + a] = self.pixels[y * self.width + x];
            }
        }
        Self {
            pixels,
            width: w,
            height: h,
        }
    }
    fn contrast(&mut self) -> (u8, u8, bool) {
        let mut histogram = [0usize; 256];
        for p in &self.pixels {
            histogram[*p as usize] += 1;
        }
        let percentile = |target: usize| {
            let mut sum = 0;
            for (i, n) in histogram.iter().enumerate() {
                sum += n;
                if sum >= target {
                    return i as u8;
                }
            }
            255
        };
        let low = percentile((self.pixels.len() / 1000).max(1));
        let high = percentile((self.pixels.len() * 999 / 1000).max(1));
        if high <= low || high - low >= 240 {
            return (0, 255, false);
        }
        for pixel in &mut self.pixels {
            *pixel = ((u32::from(pixel.saturating_sub(low)) * 255) / u32::from(high - low)).min(255)
                as u8;
        }
        (low, high, true)
    }
    fn skew(&self) -> (i16, bool) {
        let step = (self.width.max(self.height) / 800).max(1);
        let mut points = vec![];
        for y in (0..self.height).step_by(step) {
            for x in (0..self.width).step_by(step) {
                if self.pixels[y * self.width + x] < 128 {
                    points.push((x as f64, y as f64));
                }
            }
        }
        let samples = (self.width / step + 1) * (self.height / step + 1);
        if points.len() < 200 || points.len() > samples * 35 / 100 {
            return (0, false);
        }
        let padding = (self.width as f64 * 0.06).ceil() as usize + 4;
        let score = |angle: f64| {
            let slope = angle.to_radians().tan();
            let cx = self.width as f64 / 2.0;
            let mut rows = vec![0u32; self.height + padding * 2];
            for (x, y) in &points {
                let bin = (y + slope * (x - cx) + padding as f64).round() as usize;
                if let Some(row) = rows.get_mut(bin) {
                    *row += 1;
                }
            }
            rows.into_iter()
                .map(|n| u64::from(n) * u64::from(n))
                .sum::<u64>()
        };
        let zero = score(0.0);
        let (mut best, mut angle) = (zero, 0);
        for half_degrees in -6i16..=6 {
            let value = score(f64::from(half_degrees) / 2.0);
            if value > best {
                best = value;
                angle = half_degrees;
            }
        }
        if best * 100 < zero * 118 {
            return (0, false);
        }
        if angle.abs() == 6 {
            return (0, true);
        }
        (angle * 50, false)
    }
    fn straighten(self, hundredths: i16) -> Self {
        if hundredths == 0 {
            return self;
        }
        let angle = f64::from(hundredths) / 100.0;
        let (sin, cos) = angle.to_radians().sin_cos();
        let full_w = self.width as f64 * cos.abs() + self.height as f64 * sin.abs();
        let full_h = self.height as f64 * cos.abs() + self.width as f64 * sin.abs();
        let scale = (super::MAX_SIDE as f64 / full_w.max(full_h)).min(1.0);
        let w = (full_w * scale).ceil().min(super::MAX_SIDE as f64) as usize;
        let h = (full_h * scale).ceil().min(super::MAX_SIDE as f64) as usize;
        let mut pixels = vec![255; w * h];
        for y in 0..h {
            for x in 0..w {
                let u = (x as f64 + 0.5 - w as f64 / 2.0) / scale;
                let v = (y as f64 + 0.5 - h as f64 / 2.0) / scale;
                let source_x = u * cos + v * sin + self.width as f64 / 2.0;
                let source_y = -u * sin + v * cos + self.height as f64 / 2.0;
                if source_x >= 0.0
                    && source_y >= 0.0
                    && source_x < self.width as f64
                    && source_y < self.height as f64
                {
                    pixels[y * w + x] =
                        self.pixels[source_y as usize * self.width + source_x as usize];
                }
            }
        }
        Self {
            pixels,
            width: w,
            height: h,
        }
    }
    fn table_lines(&self) -> bool {
        let mut horizontal = 0;
        let mut previous = false;
        for y in 0..self.height {
            let (mut longest, mut run) = (0, 0);
            for x in 0..self.width {
                if self.pixels[y * self.width + x] < 128 {
                    run += 1;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
            }
            let line = longest > self.width * 2 / 5;
            if line && !previous {
                horizontal += 1;
            }
            previous = line;
        }
        if horizontal < 2 {
            return false;
        }
        let mut vertical = 0;
        previous = false;
        for x in 0..self.width {
            let (mut longest, mut run) = (0, 0);
            for y in 0..self.height {
                if self.pixels[y * self.width + x] < 128 {
                    run += 1;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
            }
            let line = longest > self.height * 3 / 20;
            if line && !previous {
                vertical += 1;
            }
            previous = line;
        }
        vertical >= 2
    }
    pub fn prepare(self, settings: &OcrSettings) -> (Self, OcrPageProcessing) {
        let mut image = self.cardinal(settings.rotation);
        let (mut low, mut high, mut changed) = (0, 255, false);
        let (mut angle, mut limited) = (0, false);
        if settings.enhance_scan {
            (low, high, changed) = image.contrast();
            (angle, limited) = image.skew();
            image = image.straighten(angle);
        }
        let info = OcrPageProcessing {
            width: image.width as u32,
            height: image.height as u32,
            deskew_hundredths: angle,
            contrast_stretched: changed,
            contrast_low: low,
            contrast_high: high,
            skew_limit_reached: limited,
            table_lines_detected: image.table_lines(),
        };
        (image, info)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cardinal_rotation_preserves_every_pixel_and_records_the_new_bounds() {
        let image = || Raster {
            pixels: vec![1, 2, 3, 4, 5, 6],
            width: 3,
            height: 2,
        };
        let rotated = image().cardinal(OcrRotation::Clockwise90);
        assert_eq!((rotated.width, rotated.height), (2, 3));
        assert_eq!(rotated.pixels, vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(
            image().cardinal(OcrRotation::Clockwise180).pixels,
            vec![6, 5, 4, 3, 2, 1]
        );
        assert_eq!(
            image().cardinal(OcrRotation::Clockwise270).pixels,
            vec![3, 6, 2, 5, 1, 4]
        );
    }
    #[test]
    fn enhancement_recovers_faint_pixel_contrast_without_replacing_text_or_changing_blank_pages() {
        let mut image = Raster {
            pixels: vec![230; 10000],
            width: 100,
            height: 100,
        };
        image.pixels[1000..1100].fill(210);
        let (_, info) = image.prepare(&OcrSettings::default());
        assert!(info.contrast_stretched);
        assert_eq!((info.contrast_low, info.contrast_high), (210, 230));
        assert!(info.valid());
        let (image, info) = Raster {
            pixels: vec![220; 10000],
            width: 100,
            height: 100,
        }
        .prepare(&OcrSettings::default());
        assert!(!info.contrast_stretched);
        assert_eq!(image.pixels, vec![220; 10000]);
        assert_eq!(info.deskew_hundredths, 0);
    }
    #[test]
    fn mild_skew_is_measured_and_large_skew_is_left_for_manual_review() {
        let image = |angle: f64| {
            let mut p = vec![255; 800 * 500];
            for row in [60.0, 130.0, 200.0, 270.0, 340.0] {
                for x in 60..740 {
                    let y = (row + angle.to_radians().tan() * x as f64) as usize;
                    for dy in 0..3 {
                        p[(y + dy) * 800 + x] = 0;
                    }
                }
            }
            Raster {
                pixels: p,
                width: 800,
                height: 500,
            }
        };
        let (angle, limited) = image(1.5).skew();
        assert_eq!(angle, -150);
        assert!(!limited);
        let (angle, limited) = image(5.0).skew();
        assert_eq!(angle, 0);
        assert!(limited);
        assert_eq!(image(0.0).skew(), (0, false));
    }
}
