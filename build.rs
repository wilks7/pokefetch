use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

#[path = "src/palette.rs"]
#[allow(dead_code)]
mod palette;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BUNDLE_GEN1");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest path"));
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
        for id in 1..=151 {
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
