use std::collections::HashMap;

use image::RgbaImage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }
}

#[derive(Default)]
struct Bucket {
    count: u32,
    red: u64,
    green: u64,
    blue: u64,
}

pub fn extract(image: &RgbaImage, background: &str) -> [Color; 4] {
    let background = parse_hex(background).unwrap_or(Color {
        red: 34,
        green: 36,
        blue: 54,
    });
    let mut buckets: HashMap<(u8, u8, u8), Bucket> = HashMap::new();

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 128 || (red > 242 && green > 242 && blue > 242) {
            continue;
        }
        let bucket = buckets
            .entry((red >> 3, green >> 3, blue >> 3))
            .or_default();
        bucket.count += 1;
        bucket.red += u64::from(red);
        bucket.green += u64::from(green);
        bucket.blue += u64::from(blue);
    }

    let mut candidates: Vec<(u32, Color)> = buckets
        .into_values()
        .map(|bucket| {
            let count = u64::from(bucket.count);
            (
                bucket.count,
                Color {
                    red: (bucket.red / count) as u8,
                    green: (bucket.green / count) as u8,
                    blue: (bucket.blue / count) as u8,
                },
            )
        })
        .collect();
    candidates.sort_by(|(left_count, left), (right_count, right)| {
        colorfulness(*right)
            .cmp(&colorfulness(*left))
            .then(right_count.cmp(left_count))
    });

    let mut selected = Vec::with_capacity(4);
    for (_, color) in &candidates {
        if selected
            .iter()
            .all(|existing| color_distance(*existing, *color) >= 52.0)
        {
            selected.push(*color);
            if selected.len() == 4 {
                break;
            }
        }
    }
    for (_, color) in candidates {
        if !selected.contains(&color) {
            selected.push(color);
            if selected.len() == 4 {
                break;
            }
        }
    }

    let fallbacks = [
        Color {
            red: 125,
            green: 207,
            blue: 255,
        },
        Color {
            red: 255,
            green: 199,
            blue: 119,
        },
        Color {
            red: 195,
            green: 232,
            blue: 141,
        },
        Color {
            red: 255,
            green: 117,
            blue: 127,
        },
    ];
    while selected.len() < 4 {
        selected.push(fallbacks[selected.len()]);
    }

    std::array::from_fn(|index| ensure_contrast(selected[index], background))
}

fn parse_hex(value: &str) -> Option<Color> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 {
        return None;
    }
    Some(Color {
        red: u8::from_str_radix(&value[0..2], 16).ok()?,
        green: u8::from_str_radix(&value[2..4], 16).ok()?,
        blue: u8::from_str_radix(&value[4..6], 16).ok()?,
    })
}

fn colorfulness(color: Color) -> u16 {
    let maximum = color.red.max(color.green).max(color.blue);
    let minimum = color.red.min(color.green).min(color.blue);
    u16::from(maximum - minimum)
}

fn color_distance(left: Color, right: Color) -> f64 {
    let red = f64::from(left.red) - f64::from(right.red);
    let green = f64::from(left.green) - f64::from(right.green);
    let blue = f64::from(left.blue) - f64::from(right.blue);
    (red * red + green * green + blue * blue).sqrt()
}

fn ensure_contrast(mut foreground: Color, background: Color) -> Color {
    for _ in 0..12 {
        if contrast_ratio(foreground, background) >= 3.5 {
            break;
        }
        foreground = Color {
            red: foreground.red.saturating_add((255 - foreground.red) / 4),
            green: foreground
                .green
                .saturating_add((255 - foreground.green) / 4),
            blue: foreground.blue.saturating_add((255 - foreground.blue) / 4),
        };
    }
    foreground
}

fn contrast_ratio(left: Color, right: Color) -> f64 {
    let left = luminance(left);
    let right = luminance(right);
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

fn luminance(color: Color) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.red) + 0.7152 * channel(color.green) + 0.0722 * channel(color.blue)
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{extract, Color};

    #[test]
    fn ignores_transparency_and_preserves_sprite_colors() {
        let mut image = RgbaImage::from_pixel(4, 1, Rgba([255, 255, 255, 0]));
        image.put_pixel(0, 0, Rgba([220, 40, 30, 255]));
        image.put_pixel(1, 0, Rgba([20, 180, 70, 255]));
        image.put_pixel(2, 0, Rgba([40, 80, 220, 255]));
        image.put_pixel(3, 0, Rgba([240, 180, 20, 255]));
        let palette = extract(&image, "#000000");
        assert!(palette.contains(&Color {
            red: 220,
            green: 40,
            blue: 30
        }));
        assert_eq!(palette.len(), 4);
    }
}
