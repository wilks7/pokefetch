use anyhow::{bail, Result};
use rand::Rng;

use crate::config::SpriteConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pokemon {
    pub id: u16,
    pub name: String,
}

impl Pokemon {
    pub fn label(&self) -> String {
        format!("#{:03} {}", self.id, self.name)
    }
}

pub fn resolve(selector: Option<&str>, config: &SpriteConfig) -> Result<Pokemon> {
    resolve_available(selector, config, |_| true)
}

pub fn resolve_available(
    selector: Option<&str>,
    config: &SpriteConfig,
    mut available: impl FnMut(u16) -> bool,
) -> Result<Pokemon> {
    let selector = selector.map(str::trim).filter(|value| !value.is_empty());
    if selector.is_none() || selector == Some("random") {
        return Ok(from_id(random_id(config, &mut available)?));
    }

    let selector = selector.expect("checked above");
    if let Ok(id) = selector.parse::<u16>() {
        if (1..=1025).contains(&id) {
            return Ok(from_id(id));
        }
        bail!("Pokemon id must be between 1 and 1025");
    }

    let wanted = normalize(selector);
    for (index, name) in NAMES.iter().enumerate() {
        if normalize(name) == wanted {
            return Ok(Pokemon {
                id: index as u16 + 1,
                name: (*name).to_string(),
            });
        }
    }

    let special = match wanted.as_str() {
        "nidoranf" | "nidoranfemale" => Some(29),
        "nidoranm" | "nidoranmale" => Some(32),
        "mrmime" => Some(122),
        "farfetchd" => Some(83),
        _ => None,
    };
    if let Some(id) = special {
        return Ok(from_id(id));
    }

    bail!("unknown Pokemon {selector:?}; use a name, numeric id, or 'random'")
}

pub fn is_random_selector(selector: Option<&str>) -> bool {
    selector
        .map(str::trim)
        .is_none_or(|value| value.is_empty() || value == "random")
}

fn random_id(config: &SpriteConfig, available: &mut impl FnMut(u16) -> bool) -> Result<u16> {
    let candidates = if !config.pokemon.is_empty() {
        config
            .pokemon
            .iter()
            .copied()
            .filter(|id| available(*id))
            .collect::<Vec<_>>()
    } else {
        (config.range_start..=config.range_end)
            .filter(|id| available(*id))
            .collect::<Vec<_>>()
    };
    if candidates.is_empty() {
        bail!("no Pokemon in the configured selection have a bundled sprite");
    }
    let index = rand::rng().random_range(0..candidates.len());
    Ok(candidates[index])
}

fn from_id(id: u16) -> Pokemon {
    let name = NAMES
        .get(usize::from(id.saturating_sub(1)))
        .map(|name| (*name).to_string())
        .unwrap_or_else(|| format!("Pokemon {id}"));
    Pokemon { id, name }
}

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

const NAMES: [&str; 386] = [
    "Bulbasaur",
    "Ivysaur",
    "Venusaur",
    "Charmander",
    "Charmeleon",
    "Charizard",
    "Squirtle",
    "Wartortle",
    "Blastoise",
    "Caterpie",
    "Metapod",
    "Butterfree",
    "Weedle",
    "Kakuna",
    "Beedrill",
    "Pidgey",
    "Pidgeotto",
    "Pidgeot",
    "Rattata",
    "Raticate",
    "Spearow",
    "Fearow",
    "Ekans",
    "Arbok",
    "Pikachu",
    "Raichu",
    "Sandshrew",
    "Sandslash",
    "Nidoran♀",
    "Nidorina",
    "Nidoqueen",
    "Nidoran♂",
    "Nidorino",
    "Nidoking",
    "Clefairy",
    "Clefable",
    "Vulpix",
    "Ninetales",
    "Jigglypuff",
    "Wigglytuff",
    "Zubat",
    "Golbat",
    "Oddish",
    "Gloom",
    "Vileplume",
    "Paras",
    "Parasect",
    "Venonat",
    "Venomoth",
    "Diglett",
    "Dugtrio",
    "Meowth",
    "Persian",
    "Psyduck",
    "Golduck",
    "Mankey",
    "Primeape",
    "Growlithe",
    "Arcanine",
    "Poliwag",
    "Poliwhirl",
    "Poliwrath",
    "Abra",
    "Kadabra",
    "Alakazam",
    "Machop",
    "Machoke",
    "Machamp",
    "Bellsprout",
    "Weepinbell",
    "Victreebel",
    "Tentacool",
    "Tentacruel",
    "Geodude",
    "Graveler",
    "Golem",
    "Ponyta",
    "Rapidash",
    "Slowpoke",
    "Slowbro",
    "Magnemite",
    "Magneton",
    "Farfetch'd",
    "Doduo",
    "Dodrio",
    "Seel",
    "Dewgong",
    "Grimer",
    "Muk",
    "Shellder",
    "Cloyster",
    "Gastly",
    "Haunter",
    "Gengar",
    "Onix",
    "Drowzee",
    "Hypno",
    "Krabby",
    "Kingler",
    "Voltorb",
    "Electrode",
    "Exeggcute",
    "Exeggutor",
    "Cubone",
    "Marowak",
    "Hitmonlee",
    "Hitmonchan",
    "Lickitung",
    "Koffing",
    "Weezing",
    "Rhyhorn",
    "Rhydon",
    "Chansey",
    "Tangela",
    "Kangaskhan",
    "Horsea",
    "Seadra",
    "Goldeen",
    "Seaking",
    "Staryu",
    "Starmie",
    "Mr. Mime",
    "Scyther",
    "Jynx",
    "Electabuzz",
    "Magmar",
    "Pinsir",
    "Tauros",
    "Magikarp",
    "Gyarados",
    "Lapras",
    "Ditto",
    "Eevee",
    "Vaporeon",
    "Jolteon",
    "Flareon",
    "Porygon",
    "Omanyte",
    "Omastar",
    "Kabuto",
    "Kabutops",
    "Aerodactyl",
    "Snorlax",
    "Articuno",
    "Zapdos",
    "Moltres",
    "Dratini",
    "Dragonair",
    "Dragonite",
    "Mewtwo",
    "Mew",
    "Chikorita",
    "Bayleef",
    "Meganium",
    "Cyndaquil",
    "Quilava",
    "Typhlosion",
    "Totodile",
    "Croconaw",
    "Feraligatr",
    "Sentret",
    "Furret",
    "Hoothoot",
    "Noctowl",
    "Ledyba",
    "Ledian",
    "Spinarak",
    "Ariados",
    "Crobat",
    "Chinchou",
    "Lanturn",
    "Pichu",
    "Cleffa",
    "Igglybuff",
    "Togepi",
    "Togetic",
    "Natu",
    "Xatu",
    "Mareep",
    "Flaaffy",
    "Ampharos",
    "Bellossom",
    "Marill",
    "Azumarill",
    "Sudowoodo",
    "Politoed",
    "Hoppip",
    "Skiploom",
    "Jumpluff",
    "Aipom",
    "Sunkern",
    "Sunflora",
    "Yanma",
    "Wooper",
    "Quagsire",
    "Espeon",
    "Umbreon",
    "Murkrow",
    "Slowking",
    "Misdreavus",
    "Unown",
    "Wobbuffet",
    "Girafarig",
    "Pineco",
    "Forretress",
    "Dunsparce",
    "Gligar",
    "Steelix",
    "Snubbull",
    "Granbull",
    "Qwilfish",
    "Scizor",
    "Shuckle",
    "Heracross",
    "Sneasel",
    "Teddiursa",
    "Ursaring",
    "Slugma",
    "Magcargo",
    "Swinub",
    "Piloswine",
    "Corsola",
    "Remoraid",
    "Octillery",
    "Delibird",
    "Mantine",
    "Skarmory",
    "Houndour",
    "Houndoom",
    "Kingdra",
    "Phanpy",
    "Donphan",
    "Porygon2",
    "Stantler",
    "Smeargle",
    "Tyrogue",
    "Hitmontop",
    "Smoochum",
    "Elekid",
    "Magby",
    "Miltank",
    "Blissey",
    "Raikou",
    "Entei",
    "Suicune",
    "Larvitar",
    "Pupitar",
    "Tyranitar",
    "Lugia",
    "Ho-Oh",
    "Celebi",
    "Treecko",
    "Grovyle",
    "Sceptile",
    "Torchic",
    "Combusken",
    "Blaziken",
    "Mudkip",
    "Marshtomp",
    "Swampert",
    "Poochyena",
    "Mightyena",
    "Zigzagoon",
    "Linoone",
    "Wurmple",
    "Silcoon",
    "Beautifly",
    "Cascoon",
    "Dustox",
    "Lotad",
    "Lombre",
    "Ludicolo",
    "Seedot",
    "Nuzleaf",
    "Shiftry",
    "Taillow",
    "Swellow",
    "Wingull",
    "Pelipper",
    "Ralts",
    "Kirlia",
    "Gardevoir",
    "Surskit",
    "Masquerain",
    "Shroomish",
    "Breloom",
    "Slakoth",
    "Vigoroth",
    "Slaking",
    "Nincada",
    "Ninjask",
    "Shedinja",
    "Whismur",
    "Loudred",
    "Exploud",
    "Makuhita",
    "Hariyama",
    "Azurill",
    "Nosepass",
    "Skitty",
    "Delcatty",
    "Sableye",
    "Mawile",
    "Aron",
    "Lairon",
    "Aggron",
    "Meditite",
    "Medicham",
    "Electrike",
    "Manectric",
    "Plusle",
    "Minun",
    "Volbeat",
    "Illumise",
    "Roselia",
    "Gulpin",
    "Swalot",
    "Carvanha",
    "Sharpedo",
    "Wailmer",
    "Wailord",
    "Numel",
    "Camerupt",
    "Torkoal",
    "Spoink",
    "Grumpig",
    "Spinda",
    "Trapinch",
    "Vibrava",
    "Flygon",
    "Cacnea",
    "Cacturne",
    "Swablu",
    "Altaria",
    "Zangoose",
    "Seviper",
    "Lunatone",
    "Solrock",
    "Barboach",
    "Whiscash",
    "Corphish",
    "Crawdaunt",
    "Baltoy",
    "Claydol",
    "Lileep",
    "Cradily",
    "Anorith",
    "Armaldo",
    "Feebas",
    "Milotic",
    "Castform",
    "Kecleon",
    "Shuppet",
    "Banette",
    "Duskull",
    "Dusclops",
    "Tropius",
    "Chimecho",
    "Absol",
    "Wynaut",
    "Snorunt",
    "Glalie",
    "Spheal",
    "Sealeo",
    "Walrein",
    "Clamperl",
    "Huntail",
    "Gorebyss",
    "Relicanth",
    "Luvdisc",
    "Bagon",
    "Shelgon",
    "Salamence",
    "Beldum",
    "Metang",
    "Metagross",
    "Regirock",
    "Regice",
    "Registeel",
    "Latias",
    "Latios",
    "Kyogre",
    "Groudon",
    "Rayquaza",
    "Jirachi",
    "Deoxys",
];

#[cfg(test)]
mod tests {
    use super::{resolve, resolve_available};
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
