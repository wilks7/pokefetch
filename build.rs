use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[path = "src/palette.rs"]
#[allow(dead_code)]
mod palette;

#[derive(Deserialize)]
struct SetCatalog {
    schema_version: u16,
    sets: Vec<SpriteSet>,
}

#[derive(Deserialize)]
struct SpriteSet {
    id: String,
    dex_end: u16,
    core_ranges: Vec<String>,
    variants: Vec<SpriteVariant>,
}

#[derive(Deserialize)]
struct SpriteVariant {
    id: String,
}

#[derive(Deserialize)]
struct BundleCatalog {
    schema_version: u16,
    bundles: Vec<Bundle>,
}

#[derive(Deserialize)]
struct Bundle {
    id: String,
    members: Vec<BundleMember>,
}

#[derive(Deserialize)]
struct BundleMember {
    set: String,
    scope: String,
    variants: Vec<String>,
}

#[derive(Deserialize)]
struct AssetInventory {
    assets: Vec<AssetRecord>,
}

#[derive(Deserialize)]
struct AssetRecord {
    set: String,
    variant: String,
    species: String,
    path: PathBuf,
    terminal_palette: [String; 4],
}

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLE_GEN1");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLE_ASSETS");
    println!("cargo:rerun-if-env-changed=POKEFETCH_BUNDLE");
    println!("cargo:rerun-if-changed=manifests/sets.toml");
    println!("cargo:rerun-if-changed=manifests/bundles.toml");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest path"));
    let catalog_path = manifest_dir.join("manifests/sets.toml");
    let catalog: SetCatalog = toml::from_str(
        &fs::read_to_string(&catalog_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", catalog_path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", catalog_path.display()));
    assert_eq!(catalog.schema_version, 1, "unsupported set catalog schema");
    let bundles_path = manifest_dir.join("manifests/bundles.toml");
    let bundles: BundleCatalog = toml::from_str(
        &fs::read_to_string(&bundles_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", bundles_path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", bundles_path.display()));
    assert_eq!(
        bundles.schema_version, 1,
        "unsupported bundle catalog schema"
    );
    validate_manifests(&catalog, &bundles);
    let generated = env::var_os("OUT_DIR").expect("OUT_DIR");
    let generated = PathBuf::from(generated).join("bundled.rs");
    let legacy = env::var_os("CARGO_FEATURE_BUNDLE_GEN1").is_some();
    let assets = env::var_os("CARGO_FEATURE_BUNDLE_ASSETS").is_some();
    assert!(!(legacy && assets), "choose only one bundle feature");

    let source = if assets {
        generate_asset_bundle(&manifest_dir, &catalog, &bundles)
    } else if legacy {
        generate_legacy_bundle(&manifest_dir, &catalog)
    } else {
        generate_empty_bundle()
    };
    fs::write(generated, source).expect("write generated sprite index");
}

fn generate_asset_bundle(
    manifest_dir: &std::path::Path,
    catalog: &SetCatalog,
    bundles: &BundleCatalog,
) -> String {
    let profile_id = env::var("POKEFETCH_BUNDLE").unwrap_or_else(|_| "red-blue-core".to_string());
    let profile = bundles
        .bundles
        .iter()
        .find(|bundle| bundle.id == profile_id)
        .unwrap_or_else(|| panic!("unknown POKEFETCH_BUNDLE {profile_id:?}"));
    let inventory_path = manifest_dir.join("assets/manifest.toml");
    println!("cargo:rerun-if-changed={}", inventory_path.display());
    let inventory: AssetInventory = toml::from_str(
        &fs::read_to_string(&inventory_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", inventory_path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", inventory_path.display()));

    let selected = inventory
        .assets
        .iter()
        .filter(|asset| bundle_includes(profile, catalog, asset))
        .collect::<Vec<_>>();
    assert!(
        !selected.is_empty(),
        "bundle {profile_id} selected no imported assets"
    );
    assert!(
        selected.windows(2).all(|pair| {
            (&pair[0].set, &pair[0].variant, &pair[0].species)
                < (&pair[1].set, &pair[1].variant, &pair[1].species)
        }),
        "asset inventory must be uniquely sorted by set, variant, and species"
    );

    let mut source = generated_prelude();
    for asset in selected {
        let path = manifest_dir.join(&asset.path);
        assert!(path.is_file(), "missing imported asset {}", path.display());
        println!("cargo:rerun-if-changed={}", path.display());
        let colors = asset
            .terminal_palette
            .iter()
            .map(|color| parse_hex(color))
            .map(|(red, green, blue)| format!("({red}, {green}, {blue})"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            source,
            "    Asset {{ game: {:?}, variant: {:?}, species: {:?}, bytes: include_bytes!({:?}), palette: [{colors}] }},",
            asset.set,
            asset.variant,
            asset.species,
            path.to_string_lossy()
        );
    }
    source.push_str(generated_postlude());
    let _ = writeln!(source, "pub(crate) const PROFILE: &str = {:?};", profile_id);
    source
}

fn generate_legacy_bundle(manifest_dir: &std::path::Path, catalog: &SetCatalog) -> String {
    let red_blue = catalog
        .sets
        .iter()
        .find(|set| set.id == "red-blue")
        .expect("set catalog must define red-blue");
    let sprite_dir = manifest_dir.join("sprites/red-blue");
    let mut source = generated_prelude();
    for id in 1..=red_blue.dex_end {
        let path = sprite_dir.join(format!("{id}.png"));
        println!("cargo:rerun-if-changed={}", path.display());
        if !path.is_file() {
            continue;
        }
        let image = image::open(&path)
            .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
            .to_rgba8();
        let colors = palette::extract(&image, "#222436");
        let colors = colors
            .iter()
            .map(|color| format!("({}, {}, {})", color.red, color.green, color.blue))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            source,
            "    Asset {{ game: \"red-blue\", variant: \"front\", species: \"{id}\", bytes: include_bytes!({:?}), palette: [{colors}] }},",
            path.to_string_lossy()
        );
    }
    source.push_str(generated_postlude());
    source.push_str("pub(crate) const PROFILE: &str = \"red-blue-core-legacy\";\n");
    source
}

fn generate_empty_bundle() -> String {
    "pub(crate) fn sprite(_: &str, _: &str, _: &str) -> Option<&'static [u8]> { None }\n\
     pub(crate) fn palette(_: &str, _: &str, _: &str) -> Option<[(u8, u8, u8); 4]> { None }\n\
     pub(crate) const PROFILE: &str = \"none\";\n"
        .to_string()
}

fn generated_prelude() -> String {
    "struct Asset {\n\
         game: &'static str,\n\
         variant: &'static str,\n\
         species: &'static str,\n\
         bytes: &'static [u8],\n\
         palette: [(u8, u8, u8); 4],\n\
     }\n\
     static ASSETS: &[Asset] = &[\n"
        .to_string()
}

fn generated_postlude() -> &'static str {
    "];\n\
     fn find(game: &str, variant: &str, species: &str) -> Option<&'static Asset> {\n\
         ASSETS.binary_search_by(|asset| (asset.game, asset.variant, asset.species).cmp(&(game, variant, species)))\n\
             .ok()\n\
             .map(|index| &ASSETS[index])\n\
     }\n\
     pub(crate) fn sprite(game: &str, variant: &str, species: &str) -> Option<&'static [u8]> {\n\
         find(game, variant, species).map(|asset| asset.bytes)\n\
     }\n\
     pub(crate) fn palette(game: &str, variant: &str, species: &str) -> Option<[(u8, u8, u8); 4]> {\n\
         find(game, variant, species).map(|asset| asset.palette)\n\
     }\n"
}

fn bundle_includes(profile: &Bundle, catalog: &SetCatalog, asset: &AssetRecord) -> bool {
    let Some(member) = profile
        .members
        .iter()
        .find(|member| member.set == asset.set)
    else {
        return false;
    };
    if member.variants != ["*"] && !member.variants.contains(&asset.variant) {
        return false;
    }
    if member.scope == "full" {
        return true;
    }
    let species = asset
        .species
        .split('-')
        .next()
        .and_then(|value| value.parse::<u16>().ok());
    let Some(species) = species else {
        return false;
    };
    let set = catalog
        .sets
        .iter()
        .find(|set| set.id == member.set)
        .expect("validated bundle set");
    set.core_ranges.iter().any(|range| {
        let (start, end) = range.split_once('-').expect("validated core range");
        let start = start.parse::<u16>().expect("validated core start");
        let end = end.parse::<u16>().expect("validated core end");
        (start..=end).contains(&species)
    })
}

fn parse_hex(value: &str) -> (u8, u8, u8) {
    let value = value
        .strip_prefix('#')
        .expect("palette color starts with #");
    assert_eq!(value.len(), 6, "palette color must have six digits");
    (
        u8::from_str_radix(&value[0..2], 16).expect("red palette component"),
        u8::from_str_radix(&value[2..4], 16).expect("green palette component"),
        u8::from_str_radix(&value[4..6], 16).expect("blue palette component"),
    )
}

fn validate_manifests(catalog: &SetCatalog, bundles: &BundleCatalog) {
    let mut sets = std::collections::BTreeMap::new();
    for set in &catalog.sets {
        assert!(
            sets.insert(set.id.as_str(), set).is_none(),
            "duplicate sprite set {}",
            set.id
        );
        assert!(set.dex_end > 0, "{} has an empty Pokedex", set.id);
        assert!(!set.core_ranges.is_empty(), "{} has no core roster", set.id);
        assert!(!set.variants.is_empty(), "{} has no variants", set.id);
        let mut core_species = std::collections::BTreeSet::new();
        for range in &set.core_ranges {
            let (start, end) = range.split_once('-').expect("core range must be START-END");
            let start = start
                .parse::<u16>()
                .expect("core range start must be numeric");
            let end = end.parse::<u16>().expect("core range end must be numeric");
            assert!(
                start > 0 && start <= end && end <= set.dex_end,
                "invalid core range {range}"
            );
            for species in start..=end {
                assert!(
                    core_species.insert(species),
                    "overlapping core range {range} in {}",
                    set.id
                );
            }
        }
        let mut variant_ids = std::collections::BTreeSet::new();
        assert!(
            set.variants
                .iter()
                .all(|variant| variant_ids.insert(variant.id.as_str())),
            "duplicate variant in {}",
            set.id
        );
    }

    let mut bundle_ids = std::collections::BTreeSet::new();
    for bundle in &bundles.bundles {
        assert!(
            bundle_ids.insert(bundle.id.as_str()),
            "duplicate bundle {}",
            bundle.id
        );
        assert!(!bundle.members.is_empty(), "{} has no members", bundle.id);
        let mut member_sets = std::collections::BTreeSet::new();
        for member in &bundle.members {
            assert!(
                member_sets.insert(member.set.as_str()),
                "duplicate set {} in {}",
                member.set,
                bundle.id
            );
            let set = sets
                .get(member.set.as_str())
                .unwrap_or_else(|| panic!("unknown set {}", member.set));
            assert!(
                matches!(member.scope.as_str(), "core" | "full"),
                "invalid scope"
            );
            assert!(!member.variants.is_empty(), "{} has no variants", bundle.id);
            if member.variants.iter().any(|variant| variant == "*") {
                assert_eq!(
                    member.variants,
                    ["*"],
                    "wildcard must be the only variant in {}",
                    bundle.id
                );
            } else {
                for variant in &member.variants {
                    assert!(
                        set.variants.iter().any(|known| known.id == *variant),
                        "unknown variant {variant} for {}",
                        member.set
                    );
                }
            }
        }
    }
    assert!(
        bundle_ids.contains("retro-master"),
        "missing retro-master bundle"
    );
}
