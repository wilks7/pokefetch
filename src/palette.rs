use std::collections::HashMap;

use image::RgbaImage;

pub const SIZE: usize = 8;

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

#[derive(Clone, Default)]
struct Bucket {
    count: u32,
    red: u64,
    green: u64,
    blue: u64,
}

#[derive(Clone, Copy)]
struct Candidate {
    count: u32,
    color: Color,
}

pub fn extract(image: &RgbaImage, background: &str) -> [Color; SIZE] {
    let background = parse_hex(background).unwrap_or(Color {
        red: 34,
        green: 36,
        blue: 54,
    });
    let mut buckets: HashMap<(u8, u8, u8), Bucket> = HashMap::new();

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 128 {
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

    let mut candidates: Vec<Candidate> = buckets
        .into_values()
        .map(|bucket| {
            let count = u64::from(bucket.count);
            Candidate {
                count: bucket.count,
                color: Color {
                    red: (bucket.red / count) as u8,
                    green: (bucket.green / count) as u8,
                    blue: (bucket.blue / count) as u8,
                },
            }
        })
        .collect();
    candidates.sort_by(|left, right| {
        right.count.cmp(&left.count).then_with(|| {
            (left.color.red, left.color.green, left.color.blue).cmp(&(
                right.color.red,
                right.color.green,
                right.color.blue,
            ))
        })
    });

    let mut selected = clustered_palette(&candidates);
    for color in &mut selected {
        *color = ensure_contrast(*color, background);
    }
    let mut unique = Vec::with_capacity(SIZE);
    selected.retain(|color| {
        if unique.contains(color) {
            false
        } else {
            unique.push(*color);
            true
        }
    });

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
        Color {
            red: 180,
            green: 190,
            blue: 254,
        },
        Color {
            red: 255,
            green: 150,
            blue: 213,
        },
        Color {
            red: 134,
            green: 200,
            blue: 190,
        },
        Color {
            red: 198,
            green: 160,
            blue: 246,
        },
    ];
    if selected.is_empty() {
        selected.extend(fallbacks.map(|color| ensure_contrast(color, background)));
    } else {
        let extracted = selected.len();
        while selected.len() < SIZE {
            selected.push(selected[selected.len() % extracted]);
        }
    }

    std::array::from_fn(|index| selected[index])
}

fn clustered_palette(candidates: &[Candidate]) -> Vec<Color> {
    let target = SIZE.min(candidates.len());
    if target == 0 {
        return Vec::new();
    }

    let mut centers = vec![candidates[0].color];
    while centers.len() < target {
        let next = candidates
            .iter()
            .filter(|candidate| !centers.contains(&candidate.color))
            .max_by(|left, right| {
                seed_score(**left, &centers).total_cmp(&seed_score(**right, &centers))
            })
            .map(|candidate| candidate.color);
        let Some(next) = next else {
            break;
        };
        centers.push(next);
    }

    for _ in 0..8 {
        let mut totals = vec![Bucket::default(); centers.len()];
        for candidate in candidates {
            let index = nearest_center(candidate.color, &centers);
            let total = &mut totals[index];
            total.count += candidate.count;
            total.red += u64::from(candidate.color.red) * u64::from(candidate.count);
            total.green += u64::from(candidate.color.green) * u64::from(candidate.count);
            total.blue += u64::from(candidate.color.blue) * u64::from(candidate.count);
        }
        for (center, total) in centers.iter_mut().zip(totals) {
            if total.count == 0 {
                continue;
            }
            let count = u64::from(total.count);
            *center = Color {
                red: (total.red / count) as u8,
                green: (total.green / count) as u8,
                blue: (total.blue / count) as u8,
            };
        }
    }

    let mut clusters = centers
        .into_iter()
        .map(|color| Candidate { count: 0, color })
        .collect::<Vec<_>>();
    let colors = clusters
        .iter()
        .map(|cluster| cluster.color)
        .collect::<Vec<_>>();
    for candidate in candidates {
        let index = nearest_center(candidate.color, &colors);
        clusters[index].count += candidate.count;
    }
    clusters.sort_by(|left, right| {
        right.count.cmp(&left.count).then_with(|| {
            (left.color.red, left.color.green, left.color.blue).cmp(&(
                right.color.red,
                right.color.green,
                right.color.blue,
            ))
        })
    });

    let mut ordered = vec![clusters.remove(0).color];
    while !clusters.is_empty() {
        let index = clusters
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| {
                ordering_score(**left, &ordered).total_cmp(&ordering_score(**right, &ordered))
            })
            .map(|(index, _)| index)
            .expect("non-empty clusters");
        ordered.push(clusters.remove(index).color);
    }
    ordered
}

fn seed_score(candidate: Candidate, centers: &[Color]) -> f64 {
    f64::from(candidate.count).sqrt() * nearest_distance_squared(candidate.color, centers).sqrt()
}

fn ordering_score(candidate: Candidate, selected: &[Color]) -> f64 {
    let maximum = candidate
        .color
        .red
        .max(candidate.color.green)
        .max(candidate.color.blue);
    let minimum = candidate
        .color
        .red
        .min(candidate.color.green)
        .min(candidate.color.blue);
    let chroma_weight = 0.6 + f64::from(maximum - minimum) / 255.0;
    f64::from(candidate.count).powf(0.35)
        * (8.0 + nearest_distance_squared(candidate.color, selected).sqrt())
        * chroma_weight
}

fn nearest_center(color: Color, centers: &[Color]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by_key(|(_, center)| color_distance_squared(color, **center))
        .map(|(index, _)| index)
        .expect("palette has at least one center")
}

fn nearest_distance_squared(color: Color, centers: &[Color]) -> f64 {
    centers
        .iter()
        .map(|center| color_distance_squared(color, *center))
        .min()
        .map(|distance| distance as f64)
        .unwrap_or_default()
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

fn color_distance_squared(left: Color, right: Color) -> u32 {
    let red = i32::from(left.red) - i32::from(right.red);
    let green = i32::from(left.green) - i32::from(right.green);
    let blue = i32::from(left.blue) - i32::from(right.blue);
    (red * red + green * green + blue * blue) as u32
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
    use std::collections::BTreeSet;

    use image::{Rgba, RgbaImage};

    use super::{extract, Color, SIZE};

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
        assert_eq!(palette.len(), SIZE);
    }

    #[test]
    fn keeps_dominant_colors_ahead_of_small_accents() {
        let mut image = RgbaImage::from_pixel(10, 10, Rgba([30, 160, 80, 255]));
        for x in 0..10 {
            image.put_pixel(x, 0, Rgba([250, 40, 30, 255]));
        }
        let palette = extract(&image, "#000000");
        assert!(palette[0].green > palette[0].red);
    }

    #[test]
    fn repeats_real_colors_when_a_sprite_has_fewer_than_eight() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([220, 40, 30, 255]));
        image.put_pixel(1, 0, Rgba([40, 80, 220, 255]));
        let palette = extract(&image, "#000000");
        let unique = palette
            .iter()
            .map(|color| color.hex())
            .collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), 2);
        assert_eq!(palette[0], palette[2]);
    }

    #[test]
    fn preserves_opaque_white_as_a_sprite_color() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        assert_eq!(
            extract(&image, "#000000")[0],
            Color {
                red: 255,
                green: 255,
                blue: 255
            }
        );
    }
}
