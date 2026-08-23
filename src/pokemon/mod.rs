//! Turning what the user typed into a species.
//!
//! A selector arrives as [`Option<&str>`] because there are three different
//! ways to ask for a Pokemon and they all funnel through one function:
//!
//! | The user typed | Selector    | Result                        |
//! |----------------|-------------|-------------------------------|
//! | nothing        | `None`      | a random species              |
//! | `random`       | `Some(..)`  | a random species              |
//! | `pikachu`      | `Some(..)`  | #025 by name                  |
//! | `25`           | `Some(..)`  | #025 by Pokedex number        |
//!
//! # Rust concepts on display
//!
//! - **`let ... else`**: [`resolve_available`] peels the "no name given" case
//!   off the front and returns early, so the rest of the function can treat
//!   `selector` as a plain `&str` with no unwrapping.
//! - **Generic callbacks**: `impl FnMut(u16) -> bool` lets a caller filter the
//!   random pool without this module knowing anything about sprite bundles.
//! - **Newtype-free domain modelling**: [`Pokemon`] is a plain struct; not
//!   every value needs a wrapper type.

mod names;

use anyhow::{bail, Result};
use rand::Rng;

use crate::config::SpriteConfig;
use names::NAMES;

/// Highest Pokedex number Pokefetch will accept from a user.
///
/// Artwork only ships through Generation III, but a number beyond the bundled
/// range is a *missing sprite*, not a malformed request, so the accepted range
/// tracks the National Pokedex rather than the asset corpus.
pub const MAX_DEX_ID: u16 = 1025;

/// One resolved species: a Pokedex number and the name to print beside it.
///
/// Deriving `Eq`/`PartialEq` is what lets tests write
/// `assert_eq!(resolved, expected)`; deriving `Clone` means callers can keep a
/// copy without borrowing. Neither is free, so derive only what you use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pokemon {
    /// National Pokedex number, starting at 1.
    pub id: u16,
    /// Display name, including punctuation such as `Farfetch'd`.
    pub name: String,
}

impl Pokemon {
    /// Formats the species as it appears in the greeting, e.g. `#025 Pikachu`.
    ///
    /// ```
    /// # use pokefetch::pokemon::resolve_by_name;
    /// let pikachu = resolve_by_name("pikachu").unwrap();
    /// assert_eq!(pikachu.label(), "#025 Pikachu");
    /// ```
    ///
    /// `{:03}` zero-pads to three digits so every row lines up in a column.
    pub fn label(&self) -> String {
        format!("#{:03} {}", self.id, self.name)
    }
}

/// Resolves a selector against the full Pokedex range.
///
/// This is [`resolve_available`] with a filter that accepts everything.
///
/// # Errors
///
/// Returns an error when the selector names no known species or the numeric
/// id falls outside `1..=`[`MAX_DEX_ID`].
pub fn resolve(selector: Option<&str>, config: &SpriteConfig) -> Result<Pokemon> {
    resolve_available(selector, config, |_| true)
}

/// Resolves a selector, restricting *random* picks to species `available`
/// accepts.
///
/// The filter applies only to random selection. Asking for `pikachu` by name
/// returns Pikachu even when no bundled sprite exists, because the caller can
/// still fall back to downloading one.
///
/// # Errors
///
/// Returns an error when the selector names no known species, when a numeric
/// id falls outside `1..=`[`MAX_DEX_ID`], or when `available` rejects every
/// candidate in the configured range.
pub fn resolve_available(
    selector: Option<&str>,
    config: &SpriteConfig,
    mut available: impl FnMut(u16) -> bool,
) -> Result<Pokemon> {
    // `let ... else` must diverge (return, break, panic). That requirement is
    // what makes the rest of this function unwrap-free: past this point
    // `selector` is a plain `&str`, not an Option.
    let Some(selector) = explicit_selector(selector) else {
        return Ok(from_id(random_id(config, &mut available)?));
    };

    if let Ok(id) = selector.parse::<u16>() {
        if (1..=MAX_DEX_ID).contains(&id) {
            return Ok(from_id(id));
        }
        bail!("Pokemon id must be between 1 and {MAX_DEX_ID}");
    }

    let wanted = normalize(selector);
    if let Some(id) = NAMES
        .iter()
        .position(|name| normalize(name) == wanted)
        .and_then(|index| u16::try_from(index).ok())
    {
        return Ok(from_id(id + 1));
    }

    // Species whose real names carry symbols that keyboards do not: the
    // Nidoran gender signs, and the punctuation in Mr. Mime / Farfetch'd.
    let alias = match wanted.as_str() {
        "nidoranf" | "nidoranfemale" => Some(29),
        "nidoranm" | "nidoranmale" => Some(32),
        "mrmime" => Some(122),
        "farfetchd" => Some(83),
        _ => None,
    };
    if let Some(id) = alias {
        return Ok(from_id(id));
    }

    bail!("unknown Pokemon {selector:?}; use a name, numeric id, or 'random'")
}

/// Resolves a species by name or id using default configuration.
///
/// A convenience entry point for callers (and doctests) that only want the
/// name lookup and never random selection.
///
/// # Errors
///
/// Returns an error when `selector` names no known species.
pub fn resolve_by_name(selector: &str) -> Result<Pokemon> {
    resolve(Some(selector), &SpriteConfig::default())
}

/// Reports whether a selector asks for a random species.
///
/// Callers use this to decide *ordering*: a random pick has to know which
/// sprites exist before choosing, while a named pick can choose first.
pub fn is_random_selector(selector: Option<&str>) -> bool {
    explicit_selector(selector).is_none()
}

/// Normalizes a selector to `Some(name)`, or `None` for "surprise me".
///
/// Blank input and the literal word `random` mean the same thing.
fn explicit_selector(selector: Option<&str>) -> Option<&str> {
    selector
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "random")
}

/// Picks a random Pokedex number the caller's filter accepts.
///
/// An explicit `sprites.pokemon` list wins over `range_start..=range_end`.
fn random_id(config: &SpriteConfig, available: &mut impl FnMut(u16) -> bool) -> Result<u16> {
    let candidates = if config.pokemon.is_empty() {
        (config.range_start..=config.range_end)
            .filter(|id| available(*id))
            .collect::<Vec<_>>()
    } else {
        config
            .pokemon
            .iter()
            .copied()
            .filter(|id| available(*id))
            .collect::<Vec<_>>()
    };
    if candidates.is_empty() {
        bail!("no Pokemon in the configured selection have a bundled sprite");
    }
    let index = rand::rng().random_range(0..candidates.len());
    Ok(candidates[index])
}

/// Builds a [`Pokemon`] from a Pokedex number, naming unknown ids generically.
///
/// Ids past the name table are still renderable if a sprite turns up, so this
/// degrades to `Pokemon 802` rather than failing.
fn from_id(id: u16) -> Pokemon {
    let name = NAMES
        .get(usize::from(id.saturating_sub(1)))
        .map_or_else(|| format!("Pokemon {id}"), |name| (*name).to_string());
    Pokemon { id, name }
}

/// Reduces a name to lowercase alphanumerics so user spelling can be loose.
///
/// `Mr. Mime`, `mr-mime`, and `MR MIME` all normalize to `mrmime`. The gender
/// signs are spelled out because no keyboard produces them directly.
///
/// `char::to_lowercase` returns an *iterator*, not a `char`: lowercasing is
/// not always one-to-one across languages (German ß is the classic example),
/// so `flat_map` is required rather than `map`.
fn normalize(value: &str) -> String {
    let mut normalized = String::new();
    for character in value.chars().flat_map(char::to_lowercase) {
        match character {
            '♀' => normalized.push_str("female"),
            '♂' => normalized.push_str("male"),
            _ if character.is_alphanumeric() => normalized.push(character),
            _ => {}
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{is_random_selector, resolve, resolve_available};
    use crate::config::SpriteConfig;

    #[test]
    fn resolves_names_punctuation_and_ids() {
        let config = SpriteConfig::default();
        assert_eq!(resolve(Some("pikachu"), &config).unwrap().id, 25);
        assert_eq!(resolve(Some("Farfetch'd"), &config).unwrap().id, 83);
        assert_eq!(resolve(Some("mr mime"), &config).unwrap().id, 122);
        assert_eq!(resolve(Some("151"), &config).unwrap().name, "Mew");
        assert_eq!(resolve(Some("chikorita"), &config).unwrap().id, 152);
        assert_eq!(resolve(Some("ho oh"), &config).unwrap().id, 250);
        assert_eq!(resolve(Some("treecko"), &config).unwrap().id, 252);
        assert_eq!(resolve(Some("rayquaza"), &config).unwrap().id, 384);
        assert_eq!(resolve(Some("386"), &config).unwrap().name, "Deoxys");
    }

    #[test]
    fn resolves_gender_aliases() {
        let config = SpriteConfig::default();
        assert_eq!(resolve(Some("nidoran-f"), &config).unwrap().id, 29);
        assert_eq!(resolve(Some("nidoran male"), &config).unwrap().id, 32);
    }

    #[test]
    fn treats_blank_and_random_selectors_as_random() {
        assert!(is_random_selector(None));
        assert!(is_random_selector(Some("  ")));
        assert!(is_random_selector(Some("random")));
        assert!(!is_random_selector(Some("pikachu")));
    }

    #[test]
    fn rejects_ids_outside_the_pokedex() {
        let config = SpriteConfig::default();
        assert!(resolve(Some("0"), &config).is_err());
        assert!(resolve(Some("1026"), &config).is_err());
        assert!(resolve(Some("nosuchmon"), &config).is_err());
    }

    #[test]
    fn limits_random_selection_to_available_species() {
        let config = SpriteConfig {
            range_start: 1,
            range_end: 386,
            ..SpriteConfig::default()
        };
        let pokemon = resolve_available(None, &config, |id| id == 384).unwrap();
        assert_eq!(pokemon.id, 384);
        assert_eq!(pokemon.name, "Rayquaza");
    }
}
