use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "../palette.rs"]
#[allow(dead_code)]
mod palette;

const INVENTORY_PATH: &str = "assets/manifest.toml";
const TERMINAL_BACKGROUND: &str = "#222436";

#[derive(Debug, Parser)]
#[command(about = "Import pinned PokeAPI sprite sets into Pokefetch")]
struct Cli {
    #[arg(long, default_value = "manifests/sets.toml")]
    manifest: PathBuf,

    #[arg(long, default_value = "manifests/bundles.toml")]
    bundle_manifest: PathBuf,

    #[command(subcommand)]
    command: AssetCommand,
}

#[derive(Debug, Subcommand)]
enum AssetCommand {
    /// List the configured game/version sprite sets.
    List,
    /// Plan an import, or write it only when --apply is present.
    Import {
        /// Local checkout of the pinned PokeAPI/sprites repository.
        #[arg(long)]
        source: PathBuf,
        /// Import only these set IDs; repeat for multiple sets. Defaults to all.
        #[arg(long = "set")]
        sets: Vec<String>,
        /// Copy assets and update assets/manifest.toml.
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Debug, Deserialize)]
struct SetCatalog {
    schema_version: u16,
    source: UpstreamSource,
    sets: Vec<SpriteSet>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSource {
    repository: String,
    revision: String,
    root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SpriteSet {
    id: String,
    generation: u8,
    source: PathBuf,
    dex_end: u16,
    core_ranges: Vec<String>,
    variants: Vec<SpriteVariant>,
}

#[derive(Debug, Deserialize)]
struct SpriteVariant {
    id: String,
    source: PathBuf,
    #[serde(default = "default_asset_format")]
    format: AssetFormat,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum AssetFormat {
    Gif,
    Png,
}

#[derive(Debug, Deserialize)]
struct BundleCatalog {
    schema_version: u16,
    bundles: Vec<Bundle>,
}

#[derive(Debug, Deserialize)]
struct Bundle {
    id: String,
    members: Vec<BundleMember>,
}

#[derive(Debug, Deserialize)]
struct BundleMember {
    set: String,
    scope: BundleScope,
    variants: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BundleScope {
    Core,
    Full,
}

#[derive(Debug)]
struct Candidate {
    set: String,
    variant: String,
    species: String,
    format: AssetFormat,
    source: PathBuf,
    destination: PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AssetInventory {
    schema_version: u16,
    source_repository: String,
    source_revision: String,
    assets: Vec<AssetRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssetRecord {
    set: String,
    variant: String,
    species: String,
    format: AssetFormat,
    path: String,
    bytes: u64,
    sha256: String,
    terminal_palette: Vec<String>,
}

fn default_asset_format() -> AssetFormat {
    AssetFormat::Png
}

fn main() {
    if let Err(error) = run() {
        eprintln!("pokefetch-assets: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let catalog = load_catalog(&cli.manifest)?;
    let bundles = load_bundle_catalog(&cli.bundle_manifest)?;
    validate_catalog(&catalog)?;
    validate_bundles(&bundles, &catalog)?;

    match cli.command {
        AssetCommand::List => list_catalog(&catalog, &bundles),
        AssetCommand::Import {
            source,
            sets,
            apply,
        } => import_assets(&catalog, &source, &sets, apply),
    }
}

fn load_bundle_catalog(path: &Path) -> Result<BundleCatalog> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading bundle catalog {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing bundle catalog {}", path.display()))
}

fn load_catalog(path: &Path) -> Result<SetCatalog> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading set catalog {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing set catalog {}", path.display()))
}

fn validate_catalog(catalog: &SetCatalog) -> Result<()> {
    anyhow::ensure!(
        catalog.schema_version == 1,
        "unsupported set catalog schema"
    );
    anyhow::ensure!(
        !catalog.source.revision.is_empty(),
        "source revision is empty"
    );
    let mut ids = BTreeSet::new();
    for set in &catalog.sets {
        anyhow::ensure!(ids.insert(&set.id), "duplicate sprite set {}", set.id);
        anyhow::ensure!(set.dex_end > 0, "{} has an empty Pokedex", set.id);
        anyhow::ensure!(!set.core_ranges.is_empty(), "{} has no core roster", set.id);
        let mut core_species = BTreeSet::new();
        for range in &set.core_ranges {
            let (start, end) = parse_dex_range(range)
                .with_context(|| format!("invalid core range in {}", set.id))?;
            anyhow::ensure!(end <= set.dex_end, "core range {range} exceeds {}", set.id);
            for species in start..=end {
                anyhow::ensure!(
                    core_species.insert(species),
                    "overlapping core range {range} in {}",
                    set.id
                );
            }
        }
        anyhow::ensure!(!set.variants.is_empty(), "{} has no variants", set.id);
        let mut variants = BTreeSet::new();
        for variant in &set.variants {
            anyhow::ensure!(
                variants.insert(&variant.id),
                "duplicate variant {} in {}",
                variant.id,
                set.id
            );
        }
    }
    Ok(())
}

fn parse_dex_range(value: &str) -> Result<(u16, u16)> {
    let (start, end) = value
        .split_once('-')
        .with_context(|| format!("expected START-END, got {value}"))?;
    let start = start
        .parse::<u16>()
        .with_context(|| format!("invalid start in {value}"))?;
    let end = end
        .parse::<u16>()
        .with_context(|| format!("invalid end in {value}"))?;
    anyhow::ensure!(start > 0 && start <= end, "invalid range {value}");
    Ok((start, end))
}

fn validate_bundles(bundles: &BundleCatalog, catalog: &SetCatalog) -> Result<()> {
    anyhow::ensure!(
        bundles.schema_version == 1,
        "unsupported bundle catalog schema"
    );
    let sets = catalog
        .sets
        .iter()
        .map(|set| (set.id.as_str(), set))
        .collect::<BTreeMap<_, _>>();
    let mut bundle_ids = BTreeSet::new();
    for bundle in &bundles.bundles {
        anyhow::ensure!(
            bundle_ids.insert(&bundle.id),
            "duplicate bundle {}",
            bundle.id
        );
        anyhow::ensure!(!bundle.members.is_empty(), "{} has no members", bundle.id);
        let mut member_sets = BTreeSet::new();
        for member in &bundle.members {
            anyhow::ensure!(
                member_sets.insert(&member.set),
                "duplicate set {} in {}",
                member.set,
                bundle.id
            );
            let set = sets
                .get(member.set.as_str())
                .with_context(|| format!("unknown set {} in {}", member.set, bundle.id))?;
            anyhow::ensure!(!member.variants.is_empty(), "{} has no variants", bundle.id);
            if member.variants.contains(&"*".to_string()) {
                anyhow::ensure!(
                    member.variants.len() == 1,
                    "wildcard must be the only variant in {}",
                    bundle.id
                );
            } else {
                let known = set
                    .variants
                    .iter()
                    .map(|variant| variant.id.as_str())
                    .collect::<BTreeSet<_>>();
                for variant in &member.variants {
                    anyhow::ensure!(
                        known.contains(variant.as_str()),
                        "unknown variant {variant} for {} in {}",
                        member.set,
                        bundle.id
                    );
                }
            }
        }
    }
    Ok(())
}

fn list_catalog(catalog: &SetCatalog, bundles: &BundleCatalog) -> Result<()> {
    println!("sprite sets:");
    for set in &catalog.sets {
        let variants = set
            .variants
            .iter()
            .map(|variant| variant.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  {} · Gen {} · full 1-{} · core {} · {}",
            set.id,
            set.generation,
            set.dex_end,
            set.core_ranges.join(","),
            variants
        );
    }
    println!("bundle profiles:");
    for bundle in &bundles.bundles {
        let members = bundle
            .members
            .iter()
            .map(|member| {
                let scope = match member.scope {
                    BundleScope::Core => "core",
                    BundleScope::Full => "full",
                };
                format!("{}:{scope}", member.set)
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {} · {members}", bundle.id);
    }
    Ok(())
}

fn import_assets(
    catalog: &SetCatalog,
    source_checkout: &Path,
    requested_sets: &[String],
    apply: bool,
) -> Result<()> {
    validate_checkout(source_checkout, catalog)?;
    let selected = select_sets(catalog, requested_sets)?;
    let candidates = collect_candidates(catalog, source_checkout, &selected)?;
    print_plan(&candidates);
    if !apply {
        println!("dry run; pass --apply to copy assets and update {INVENTORY_PATH}");
        return Ok(());
    }

    let selected_ids = selected
        .iter()
        .map(|set| set.id.as_str())
        .collect::<BTreeSet<_>>();
    let inventory_path = Path::new(INVENTORY_PATH);
    let mut inventory = load_inventory(inventory_path)?;
    inventory
        .assets
        .retain(|asset| !selected_ids.contains(asset.set.as_str()));
    for candidate in &candidates {
        inventory.assets.push(import_candidate(candidate)?);
    }
    inventory.assets.sort_by(|left, right| {
        (&left.set, &left.variant, &left.species).cmp(&(&right.set, &right.variant, &right.species))
    });
    inventory.schema_version = 2;
    inventory
        .source_repository
        .clone_from(&catalog.source.repository);
    inventory
        .source_revision
        .clone_from(&catalog.source.revision);
    let expected = inventory
        .assets
        .iter()
        .map(|asset| PathBuf::from(&asset.path))
        .collect::<BTreeSet<_>>();
    let mut pruned = 0;
    for set in &selected {
        pruned += prune_tree(&Path::new("assets/sets").join(&set.id), &expected)?;
    }
    write_inventory(inventory_path, &inventory)?;
    println!(
        "updated {INVENTORY_PATH} with {} assets",
        inventory.assets.len()
    );
    if pruned > 0 {
        println!("pruned {pruned} stale imported assets");
    }
    Ok(())
}

fn prune_tree(path: &Path, expected: &BTreeSet<PathBuf>) -> Result<usize> {
    if !path.is_dir() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in std::fs::read_dir(path).with_context(|| format!("reading {}", path.display()))? {
        let entry = entry.with_context(|| format!("reading {} entry", path.display()))?;
        let child = entry.path();
        if child.is_dir() {
            removed += prune_tree(&child, expected)?;
            if std::fs::read_dir(&child)
                .with_context(|| format!("checking {}", child.display()))?
                .next()
                .is_none()
            {
                std::fs::remove_dir(&child)
                    .with_context(|| format!("removing empty {}", child.display()))?;
            }
        } else if !expected.contains(&child) {
            std::fs::remove_file(&child).with_context(|| format!("pruning {}", child.display()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn validate_checkout(source_checkout: &Path, catalog: &SetCatalog) -> Result<()> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(source_checkout)
        .args(["rev-parse", "HEAD"])
        .output()
        .with_context(|| format!("checking {} revision", source_checkout.display()))?;
    anyhow::ensure!(output.status.success(), "source is not a Git checkout");
    let revision = String::from_utf8_lossy(&output.stdout);
    anyhow::ensure!(
        revision.trim() == catalog.source.revision,
        "source revision {} does not match pinned {}",
        revision.trim(),
        catalog.source.revision
    );
    Ok(())
}

fn select_sets<'a>(catalog: &'a SetCatalog, requested: &[String]) -> Result<Vec<&'a SpriteSet>> {
    if requested.is_empty() {
        return Ok(catalog.sets.iter().collect());
    }
    let requested = requested
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let selected = catalog
        .sets
        .iter()
        .filter(|set| requested.contains(set.id.as_str()))
        .collect::<Vec<_>>();
    if selected.len() != requested.len() {
        let known = catalog
            .sets
            .iter()
            .map(|set| set.id.as_str())
            .collect::<BTreeSet<_>>();
        let unknown = requested.difference(&known).copied().collect::<Vec<_>>();
        bail!("unknown sprite set(s): {}", unknown.join(", "));
    }
    Ok(selected)
}

fn collect_candidates(
    catalog: &SetCatalog,
    source_checkout: &Path,
    sets: &[&SpriteSet],
) -> Result<Vec<Candidate>> {
    let mut candidates = Vec::new();
    for set in sets {
        for variant in &set.variants {
            let source_dir = source_checkout
                .join(&catalog.source.root)
                .join(&set.source)
                .join(&variant.source);
            let entries = std::fs::read_dir(&source_dir)
                .with_context(|| format!("reading {}", source_dir.display()))?;
            for entry in entries {
                let entry =
                    entry.with_context(|| format!("reading {} entry", source_dir.display()))?;
                let path = entry.path();
                let Some((dex_id, species)) = species_key(&path, variant.format) else {
                    continue;
                };
                if dex_id > set.dex_end {
                    continue;
                }
                let filename = path
                    .file_name()
                    .context("asset has no filename")?
                    .to_owned();
                candidates.push(Candidate {
                    set: set.id.clone(),
                    variant: variant.id.clone(),
                    species,
                    format: variant.format,
                    source: path,
                    destination: Path::new("assets/sets")
                        .join(&set.id)
                        .join(&variant.id)
                        .join(filename),
                });
            }
        }
    }
    candidates.sort_by(|left, right| {
        (&left.set, &left.variant, &left.species).cmp(&(&right.set, &right.variant, &right.species))
    });
    Ok(candidates)
}

fn species_key(path: &Path, format: AssetFormat) -> Option<(u16, String)> {
    let expected = match format {
        AssetFormat::Gif => "gif",
        AssetFormat::Png => "png",
    };
    if path.extension()?.to_str()? != expected {
        return None;
    }
    let species = path.file_stem()?.to_str()?.to_string();
    let dex_id = species.split('-').next()?.parse().ok()?;
    Some((dex_id, species))
}

fn print_plan(candidates: &[Candidate]) {
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for candidate in candidates {
        *counts
            .entry((candidate.set.as_str(), candidate.variant.as_str()))
            .or_default() += 1;
    }
    for ((set, variant), count) in counts {
        println!("{set}/{variant}: {count}");
    }
    println!("total: {} assets", candidates.len());
}

fn import_candidate(candidate: &Candidate) -> Result<AssetRecord> {
    let bytes = std::fs::read(&candidate.source)
        .with_context(|| format!("reading {}", candidate.source.display()))?;
    let image_format = match candidate.format {
        AssetFormat::Gif => image::ImageFormat::Gif,
        AssetFormat::Png => image::ImageFormat::Png,
    };
    let image = image::load_from_memory_with_format(&bytes, image_format)
        .with_context(|| format!("validating {}", candidate.source.display()))?
        .to_rgba8();
    anyhow::ensure!(
        image.pixels().any(|pixel| pixel[3] < u8::MAX),
        "{} has no transparent pixels",
        candidate.source.display()
    );
    atomic_write(&candidate.destination, &bytes)?;
    let colors = palette::extract(&image, TERMINAL_BACKGROUND);
    Ok(AssetRecord {
        set: candidate.set.clone(),
        variant: candidate.variant.clone(),
        species: candidate.species.clone(),
        format: candidate.format,
        path: candidate.destination.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        terminal_palette: colors.into_iter().map(|color| color.hex()).collect(),
    })
}

fn load_inventory(path: &Path) -> Result<AssetInventory> {
    if !path.is_file() {
        return Ok(AssetInventory::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn write_inventory(path: &Path, inventory: &AssetInventory) -> Result<()> {
    let text = toml::to_string_pretty(inventory).context("serializing asset inventory")?;
    atomic_write(path, text.as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("destination has no parent")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("asset"),
        std::process::id()
    ));
    std::fs::write(&temporary, bytes)
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("installing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        load_bundle_catalog, load_catalog, load_inventory, parse_dex_range, species_key,
        validate_bundles, validate_catalog, AssetFormat,
    };
    use std::path::Path;

    #[test]
    fn checked_in_catalog_is_valid() {
        let catalog = load_catalog(Path::new("manifests/sets.toml")).unwrap();
        validate_catalog(&catalog).unwrap();
        assert_eq!(catalog.sets.len(), 8);
        assert!(catalog.sets.iter().any(|set| set.id == "firered-leafgreen"));
        let bundles = load_bundle_catalog(Path::new("manifests/bundles.toml")).unwrap();
        validate_bundles(&bundles, &catalog).unwrap();
        assert!(bundles
            .bundles
            .iter()
            .any(|bundle| bundle.id == "retro-master"));
    }

    #[test]
    fn parses_inclusive_dex_ranges() {
        assert_eq!(parse_dex_range("152-251").unwrap(), (152, 251));
        assert!(parse_dex_range("251-152").is_err());
        assert!(parse_dex_range("25").is_err());
    }

    #[test]
    fn recognizes_numeric_species_and_forms() {
        assert_eq!(
            species_key(Path::new("25.png"), AssetFormat::Png),
            Some((25, "25".to_string()))
        );
        assert_eq!(
            species_key(Path::new("201-a.png"), AssetFormat::Png),
            Some((201, "201-a".to_string()))
        );
        assert_eq!(
            species_key(Path::new("25.gif"), AssetFormat::Gif),
            Some((25, "25".to_string()))
        );
        assert_eq!(species_key(Path::new("README.md"), AssetFormat::Png), None);
    }

    #[test]
    fn checked_in_inventory_has_eight_color_palettes() {
        let inventory = load_inventory(Path::new("assets/manifest.toml")).unwrap();
        assert_eq!(inventory.schema_version, 2);
        assert_eq!(inventory.assets.len(), 2_362);
        assert!(inventory
            .assets
            .iter()
            .all(|asset| asset.terminal_palette.len() == 8));
    }
}
