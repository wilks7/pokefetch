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

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLE_GEN1");
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
    let red_blue = catalog
        .sets
        .iter()
        .find(|set| set.id == "red-blue")
        .expect("set catalog must define red-blue");
    let sprite_dir = manifest_dir.join("sprites/red-blue");
    let generated = env::var_os("OUT_DIR").expect("OUT_DIR");
    let generated = PathBuf::from(generated).join("bundled.rs");
    let bundled = env::var_os("CARGO_FEATURE_BUNDLE_GEN1").is_some();

    let mut source = String::from("pub(crate) fn sprite(id: u16) -> Option<&'static [u8]> {\n");
    let mut palettes =
        String::from("pub(crate) fn palette(id: u16) -> Option<[(u8, u8, u8); 4]> {\n");
    if bundled {
        source.push_str("    match id {\n");
        palettes.push_str("    match id {\n");
        for id in 1..=red_blue.dex_end {
            let path = sprite_dir.join(format!("{id}.png"));
            println!("cargo:rerun-if-changed={}", path.display());
            if path.is_file() {
                let _ = writeln!(
                    source,
                    "        {id} => Some(include_bytes!({:?})),",
                    path.to_string_lossy()
                );
                let image = image::open(&path)
                    .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
                    .to_rgba8();
                let colors = palette::extract(&image, "#222436");
                let _ = writeln!(
                    palettes,
                    "        {id} => Some([{}]),",
                    colors
                        .iter()
                        .map(|color| format!("({}, {}, {})", color.red, color.green, color.blue))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        source.push_str("        _ => None,\n    }\n");
        palettes.push_str("        _ => None,\n    }\n");
    } else {
        source.push_str("    let _ = id;\n    None\n");
        palettes.push_str("    let _ = id;\n    None\n");
    }
    source.push_str("}\n");
    palettes.push_str("}\n");
    source.push_str(&palettes);
    fs::write(generated, source).expect("write generated sprite index");
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
