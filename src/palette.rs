//! Choosing eight terminal colors that look like a sprite.
//!
//! Naively taking the eight most common colors gives you eight shades of the
//! same blue on a Squirtle. This module instead runs a small clustering pass so
//! the result spans the sprite, then nudges each color until it is readable
//! against the terminal background.
//!
//! The pipeline:
//!
//! ```text
//!   pixels
//!     |  drop anything mostly transparent
//!     |  quantize to a 32x32x32 grid, averaging each cell   -> candidates
//!     |  k-means++ style seeding, then 8 refinement rounds  -> 8 clusters
//!     |  reorder: dominant first, then most different       -> ordered
//!     |  lighten anything unreadable on the background      -> palette
//!     v
//!   [Color; 8]
//! ```
//!
//! # Why eight?
//!
//! Eight is enough variety for a greeting to feel like the sprite, and it
//! matches the number of information lines the layout can display. The
//! greeting currently uses five, leaving three slots for future rows.
//!
//! # Rust concepts on display
//!
//! - **`Copy` types**: [`Color`] is three bytes, so it is [`Copy`] — assigning
//!   it duplicates it rather than moving it. Compare with [`String`], where a
//!   copy would mean an allocation, so Rust makes you say `.clone()`.
//! - **`std::array::from_fn`**: building a fixed-size `[Color; 8]` without
//!   `unwrap`, by calling a closure once per index.
//! - **Avoiding `as`**: this module deliberately uses [`u8::abs_diff`],
//!   [`u8::try_from`], and [`f64::from`] instead of `as` casts. `as` silently
//!   truncates; these say what should happen when a value does not fit.
//! - **Sorting by a tuple**: `then_with` builds a tie-break chain, which is
//!   what makes extraction deterministic for a given sprite.

use std::collections::HashMap;

use image::RgbaImage;

/// Number of colors in every extracted palette.
pub const SIZE: usize = 8;

/// Pixels at or below this alpha are treated as background and ignored.
const OPACITY_THRESHOLD: u8 = 128;

/// Bits dropped per channel when grouping similar pixels.
///
/// Shifting right by 3 collapses 256 levels to 32, so near-identical shades
/// land in one bucket instead of competing as separate colors.
const QUANTIZE_SHIFT: u8 = 3;

/// Refinement rounds for the clustering pass. Converges well before this.
const REFINEMENT_ROUNDS: usize = 8;

/// Minimum contrast ratio a palette color must reach against the background.
const MIN_CONTRAST: f64 = 3.5;

/// Colors used when a sprite yields nothing at all (fully transparent input).
const FALLBACKS: [Color; SIZE] = [
    Color::rgb(125, 207, 255),
    Color::rgb(255, 199, 119),
    Color::rgb(195, 232, 141),
    Color::rgb(255, 117, 127),
    Color::rgb(180, 190, 254),
    Color::rgb(255, 150, 213),
    Color::rgb(134, 200, 190),
    Color::rgb(198, 160, 246),
];

/// An 8-bit-per-channel RGB color.
///
/// Small and [`Copy`], so it is passed by value throughout. A type this size
/// is cheaper to copy than to reference.
///
/// `Ord` is derived, which orders colors lexicographically by red, then green,
/// then blue. That ordering carries no visual meaning — it exists purely to
/// give sorting a deterministic tie-break. Field order in the struct *is* the
/// comparison order, so reordering these fields would change it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Color {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

impl Color {
    /// Builds a color from three channels.
    ///
    /// `const fn` means this can be called while building a `const`, which is
    /// what lets [`FALLBACKS`] be a compile-time table.
    ///
    /// ```
    /// # use pokefetch::palette::Color;
    /// assert_eq!(Color::rgb(255, 0, 0).hex(), "#FF0000");
    /// ```
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Formats the color as uppercase `#RRGGBB`.
    ///
    /// Takes `self` by value, not `&self`, because [`Color`] is [`Copy`].
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }
}

/// A running total used to average the colors that fall into one group.
///
/// The channel sums are [`u64`] because a large sprite can hold far more than
/// 255 pixels per bucket, and `u8` would overflow immediately.
#[derive(Clone, Default)]
struct Bucket {
    count: u32,
    red: u64,
    green: u64,
    blue: u64,
}

impl Bucket {
    /// Adds one pixel, weighted by how many source pixels it represents.
    fn add(&mut self, color: Color, weight: u32) {
        self.count += weight;
        self.red += u64::from(color.red) * u64::from(weight);
        self.green += u64::from(color.green) * u64::from(weight);
        self.blue += u64::from(color.blue) * u64::from(weight);
    }

    /// Returns the weighted average color, or [`None`] if nothing was added.
    fn average(&self) -> Option<Color> {
        let count = u64::from(self.count);
        if count == 0 {
            return None;
        }
        Some(Color::rgb(
            average_channel(self.red, count),
            average_channel(self.green, count),
            average_channel(self.blue, count),
        ))
    }
}

/// Divides a channel total by a count, saturating instead of truncating.
///
/// The result is an average of `u8` values, so it always fits in a `u8`. Using
/// `try_from` rather than `as` documents that expectation and degrades to 255
/// instead of silently wrapping if the invariant were ever broken.
fn average_channel(total: u64, count: u64) -> u8 {
    u8::try_from(total / count).unwrap_or(u8::MAX)
}

/// One quantized color and how many source pixels it stands for.
#[derive(Clone, Copy)]
struct Candidate {
    count: u32,
    color: Color,
}

/// Extracts eight representative, readable colors from a sprite.
///
/// `background` is the terminal background as `#RRGGBB`; colors too close to
/// it are lightened until they are legible. An unparseable value falls back to
/// a dark slate, so a typo in the config dims the palette rather than failing.
///
/// Sprites with fewer than eight distinct colors repeat real ones rather than
/// inventing new ones — a two-color sprite stays a two-color palette.
///
/// ```
/// # use pokefetch::palette::{extract, SIZE};
/// # use image::{Rgba, RgbaImage};
/// let sprite = RgbaImage::from_pixel(4, 4, Rgba([220, 40, 30, 255]));
/// let colors = extract(&sprite, "#222436");
/// assert_eq!(colors.len(), SIZE);
/// ```
pub fn extract(image: &RgbaImage, background: &str) -> [Color; SIZE] {
    let background = parse_hex(background).unwrap_or(Color::rgb(34, 36, 54));
    let candidates = quantize(image);

    let mut selected = clustered_palette(&candidates);
    for color in &mut selected {
        *color = ensure_contrast(*color, background);
    }
    // Contrast correction can collapse two near colors onto the same value.
    // `retain` keeps the first occurrence of each and drops later duplicates.
    let mut unique: Vec<Color> = Vec::with_capacity(SIZE);
    selected.retain(|color| {
        let fresh = !unique.contains(color);
        if fresh {
            unique.push(*color);
        }
        fresh
    });

    if selected.is_empty() {
        selected.extend(FALLBACKS.map(|color| ensure_contrast(color, background)));
    } else {
        // Cycle through the real colors rather than padding with invented
        // ones, so a two-color sprite still looks like a two-color sprite.
        let extracted = selected.len();
        while selected.len() < SIZE {
            selected.push(selected[selected.len() % extracted]);
        }
    }

    std::array::from_fn(|index| selected[index])
}

/// Groups opaque pixels into a coarse color grid and averages each cell.
///
/// Returns candidates sorted by population, with a color tie-break so the same
/// sprite always produces the same palette.
fn quantize(image: &RgbaImage) -> Vec<Candidate> {
    let mut buckets: HashMap<(u8, u8, u8), Bucket> = HashMap::new();

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < OPACITY_THRESHOLD {
            continue;
        }
        // `entry(..).or_default()` inserts a zeroed Bucket the first time this
        // cell is seen, then hands back a mutable reference either way.
        buckets
            .entry((
                red >> QUANTIZE_SHIFT,
                green >> QUANTIZE_SHIFT,
                blue >> QUANTIZE_SHIFT,
            ))
            .or_default()
            .add(Color::rgb(red, green, blue), 1);
    }

    let mut candidates = buckets
        .into_values()
        .filter_map(|bucket| {
            bucket.average().map(|color| Candidate {
                count: bucket.count,
                color,
            })
        })
        .collect::<Vec<_>>();
    // A HashMap has no defined iteration order, so sorting is what makes
    // extraction reproducible. Population first, then color as a tie-break.
    candidates.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.color.cmp(&right.color))
    });
    candidates
}

/// Reduces candidates to [`SIZE`] cluster centers, ordered for display.
///
/// This is k-means with a k-means++ style seeding pass: start from the most
/// populous color, then repeatedly add whichever candidate is the best
/// combination of "common" and "unlike everything chosen so far". Seeding this
/// way is what keeps a mostly-blue sprite from producing eight blues.
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
                // `total_cmp` orders floats completely, including NaN. `>` and
                // `partial_cmp` cannot, which is why sorting f64 needs it.
                seed_score(**left, &centers).total_cmp(&seed_score(**right, &centers))
            })
            .map(|candidate| candidate.color);
        let Some(next) = next else { break };
        centers.push(next);
    }

    // Standard k-means refinement: assign every candidate to its nearest
    // center, then move each center to the weighted average of what it caught.
    for _ in 0..REFINEMENT_ROUNDS {
        let mut totals = vec![Bucket::default(); centers.len()];
        for candidate in candidates {
            let index = nearest_center(candidate.color, &centers);
            totals[index].add(candidate.color, candidate.count);
        }
        for (center, total) in centers.iter_mut().zip(&totals) {
            if let Some(average) = total.average() {
                *center = average;
            }
        }
    }

    order_for_display(centers, candidates)
}

/// Orders finished clusters: most populous first, then most contrasting.
///
/// The first color is the sprite's dominant tone. After that each slot picks
/// whichever remaining cluster is the most visually distinct from what has
/// already been chosen, so adjacent greeting lines never look identical.
fn order_for_display(centers: Vec<Color>, candidates: &[Candidate]) -> Vec<Color> {
    let colors = centers.clone();
    let mut clusters = centers
        .into_iter()
        .map(|color| Candidate { count: 0, color })
        .collect::<Vec<_>>();
    for candidate in candidates {
        clusters[nearest_center(candidate.color, &colors)].count += candidate.count;
    }
    clusters.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.color.cmp(&right.color))
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
            .expect("clusters is non-empty inside this loop");
        ordered.push(clusters.remove(index).color);
    }
    ordered
}

/// Scores a seed candidate: common colors that are also far from the chosen
/// ones win. The square roots damp both terms so neither dominates.
fn seed_score(candidate: Candidate, centers: &[Color]) -> f64 {
    f64::from(candidate.count).sqrt() * nearest_distance_squared(candidate.color, centers).sqrt()
}

/// Scores a cluster for display order, favoring saturated, distinct colors.
///
/// `chroma_weight` boosts vivid colors over grays, so a sprite's accent tones
/// surface early instead of being buried behind three shades of shadow.
fn ordering_score(candidate: Candidate, selected: &[Color]) -> f64 {
    let Color { red, green, blue } = candidate.color;
    let chroma = red.max(green).max(blue) - red.min(green).min(blue);
    let chroma_weight = 0.6 + f64::from(chroma) / 255.0;
    f64::from(candidate.count).powf(0.35)
        * (8.0 + nearest_distance_squared(candidate.color, selected).sqrt())
        * chroma_weight
}

/// Returns the index of the closest center to `color`.
fn nearest_center(color: Color, centers: &[Color]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by_key(|(_, center)| color_distance_squared(color, **center))
        .map(|(index, _)| index)
        .expect("clustering always has at least one center")
}

/// Distance from `color` to the nearest center, or 0 when there are none.
fn nearest_distance_squared(color: Color, centers: &[Color]) -> f64 {
    centers
        .iter()
        .map(|center| color_distance_squared(color, *center))
        .min()
        .map_or(0.0, f64::from)
}

/// Squared Euclidean distance in RGB space.
///
/// Squared, because comparing distances never needs the square root, and
/// skipping it keeps this in integer arithmetic. `abs_diff` sidesteps signed
/// subtraction entirely, so no `as` cast is needed.
fn color_distance_squared(left: Color, right: Color) -> u32 {
    let red = u32::from(left.red.abs_diff(right.red));
    let green = u32::from(left.green.abs_diff(right.green));
    let blue = u32::from(left.blue.abs_diff(right.blue));
    red * red + green * green + blue * blue
}

/// Parses `#RRGGBB` or `RRGGBB`, returning [`None`] for anything else.
fn parse_hex(value: &str) -> Option<Color> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 {
        return None;
    }
    // `?` inside a function returning Option propagates None, exactly as it
    // propagates Err in a function returning Result.
    Some(Color::rgb(
        u8::from_str_radix(value.get(0..2)?, 16).ok()?,
        u8::from_str_radix(value.get(2..4)?, 16).ok()?,
        u8::from_str_radix(value.get(4..6)?, 16).ok()?,
    ))
}

/// Lightens a color until it is readable against the background.
///
/// Each pass closes a quarter of the remaining distance to white, which
/// converges quickly and preserves hue. The loop is bounded so a pathological
/// background cannot spin forever; it simply gives up with the best it found.
fn ensure_contrast(mut foreground: Color, background: Color) -> Color {
    for _ in 0..12 {
        if contrast_ratio(foreground, background) >= MIN_CONTRAST {
            break;
        }
        foreground = Color::rgb(
            foreground.red.saturating_add((255 - foreground.red) / 4),
            foreground
                .green
                .saturating_add((255 - foreground.green) / 4),
            foreground.blue.saturating_add((255 - foreground.blue) / 4),
        );
    }
    foreground
}

/// WCAG contrast ratio between two colors, from 1.0 to 21.0.
fn contrast_ratio(left: Color, right: Color) -> f64 {
    let left = luminance(left);
    let right = luminance(right);
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

/// Relative luminance, per the WCAG definition.
///
/// The per-channel curve undoes sRGB gamma encoding; the weights reflect how
/// much each channel contributes to perceived brightness. Note the nested
/// `fn` — a helper visible only inside this function.
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

    use super::{color_distance_squared, extract, parse_hex, Color, SIZE};

    #[test]
    fn ignores_transparency_and_preserves_sprite_colors() {
        let mut image = RgbaImage::from_pixel(4, 1, Rgba([255, 255, 255, 0]));
        image.put_pixel(0, 0, Rgba([220, 40, 30, 255]));
        image.put_pixel(1, 0, Rgba([20, 180, 70, 255]));
        image.put_pixel(2, 0, Rgba([40, 80, 220, 255]));
        image.put_pixel(3, 0, Rgba([240, 180, 20, 255]));
        let palette = extract(&image, "#000000");
        assert!(palette.contains(&Color::rgb(220, 40, 30)));
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
        assert_eq!(extract(&image, "#000000")[0], Color::rgb(255, 255, 255));
    }

    #[test]
    fn falls_back_when_every_pixel_is_transparent() {
        let image = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 0]));
        let palette = extract(&image, "#222436");
        assert_eq!(palette.len(), SIZE);
        let unique = palette.iter().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), SIZE, "fallbacks are all distinct");
    }

    #[test]
    fn extraction_is_deterministic_for_one_sprite() {
        let mut image = RgbaImage::from_pixel(8, 8, Rgba([30, 160, 80, 255]));
        image.put_pixel(0, 0, Rgba([250, 40, 30, 255]));
        image.put_pixel(1, 1, Rgba([40, 80, 220, 255]));
        assert_eq!(extract(&image, "#222436"), extract(&image, "#222436"));
    }

    #[test]
    fn parses_hex_with_and_without_a_leading_hash() {
        assert_eq!(parse_hex("#FF8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(parse_hex("ff8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(parse_hex("#FFF"), None);
        assert_eq!(parse_hex("not a color"), None);
    }

    #[test]
    fn distance_is_symmetric_and_zero_for_equal_colors() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        assert_eq!(color_distance_squared(red, red), 0);
        assert_eq!(
            color_distance_squared(red, blue),
            color_distance_squared(blue, red)
        );
    }
}
