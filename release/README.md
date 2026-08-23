# Release process

Releases are deliberately manual. The workflow packages binaries and creates a
draft; it does not create tags or publish releases.

1. Update `Cargo.toml` and move the relevant `CHANGELOG.md` entries under the
   version being prepared.
2. Run `release/package --tag vX.Y.Z`, then `release/verify`. The verifier
   checks archive contents, executable versions, embedded bundle identities,
   build provenance, checksums, a clean greeting, and preservation of an
   existing config file.
3. Get fresh confirmation before creating or pushing the annotated `vX.Y.Z`
   tag.
4. Run the manual `Release` workflow with that existing tag. It rebuilds the
   profiles in `release/bundles.txt`, preserves them as workflow artifacts, and
   creates a draft GitHub Release.
5. Inspect the draft notes, archives, checksums, bundle identities, and install
   behavior.
6. Get fresh confirmation immediately before publishing the draft release.

Add a profile to `release/bundles.txt` only when it deserves a separately
downloadable executable. A bundle's existence in `manifests/bundles.toml` does
not require publishing it as a release asset.
