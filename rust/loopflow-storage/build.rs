//! Generate the ordered migration catalogue, canonical schema and fixture image.
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{env, fs};
#[path = "src/migration_build.rs"]
mod migration_build;
#[allow(dead_code)] // Runtime identity helpers share this source with the generator.
#[path = "src/migration_catalog.rs"]
mod migration_catalog;
#[path = "src/migration_schema.rs"]
mod migration_schema;
fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output"));
    let root = manifest.ancestors().find(|path| path.join(".git").exists());
    let unmerged = emit_migration_catalog(&manifest, root, &out);
    let canonical_fixture = manifest.join("tests/fixtures/migrations/canonical_ids_authority.sql");
    println!("cargo:rerun-if-changed={}", canonical_fixture.display());
    let sql = fs::read_to_string(canonical_fixture).expect("read canonical authority fixture");
    fs::write(out.join("canonical_fixture.rs"), format!("{sql:?}"))
        .expect("write canonical authority fixture");
    println!("cargo:unmerged={}", unmerged.join(","));
    if let Some(root) = root {
        for path in migration_build::reference_watches(root) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
/// Generate `migrations.rs`, the ordered catalog, and the schema it builds;
/// replaying every migration would cost each process that opens a store.
/// Returns the slug migrations `origin/main` has not added.
fn emit_migration_catalog(
    manifest_dir: &Path,
    git_root: Option<&Path>,
    out_dir: &Path,
) -> Vec<String> {
    let directory = manifest_dir.join("src/migrations");
    println!("cargo:rerun-if-changed={}", directory.display());
    // A source snapshot without Git history (a container build) names the
    // order origin/main gave its migrations, one slug per line.
    println!("cargo:rerun-if-env-changed=LOOPFLOW_MIGRATION_ORDER");
    let merged_order = match env::var_os("LOOPFLOW_MIGRATION_ORDER") {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", Path::new(&path).display());
            fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read LOOPFLOW_MIGRATION_ORDER: {error}"))
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        }
        None => match git_root {
            Some(root) => {
                let root = root
                    .canonicalize()
                    .expect("canonicalize Loopflow source root");
                let directory = directory
                    .canonicalize()
                    .expect("canonicalize migrations directory");
                migration_build::merged_order(&root, &directory)
            }
            None => {
                let path = manifest_dir.join("migration-order.txt");
                println!("cargo:rerun-if-changed={}", path.display());
                fs::read_to_string(path)
                    .expect("read packaged migration order")
                    .lines()
                    .map(str::to_string)
                    .collect()
            }
        },
    };
    let catalog = migration_build::order_catalog(&directory, &merged_order)
        .unwrap_or_else(|error| panic!("order migrations: {error}"));
    let connection = migration_build::apply_catalog(&catalog)
        .unwrap_or_else(|error| panic!("apply migration catalog: {error}"));

    let code = migration_build::render_catalog(&catalog)
        .unwrap_or_else(|error| panic!("render migration catalog: {error}"));
    fs::write(out_dir.join("migrations.rs"), code).expect("write migration catalog");

    let schema =
        migration_schema::product_schema(&connection).expect("project catalog schema reference");
    let json = serde_json::to_vec(&*schema).expect("serialize catalog schema reference");
    fs::write(out_dir.join("canonical_schema.json"), json).expect("write catalog schema reference");
    // Test processes each get an independent copy, including nextest's one-test
    // processes. Replaying the catalog here keeps that cost out of every fixture.
    let fixture = out_dir.join("test-store.db");
    let _ = fs::remove_file(&fixture);
    // Each copied fixture replaces this identity. Stable source bytes let the
    // test-support library reuse the same cache entry in another checkout.
    connection
        .execute(
            "UPDATE machines SET id='home_00000000000000000000000000000000',
             created_at=0,observed_at=0 WHERE is_local=1",
            [],
        )
        .expect("normalize the fixture Machine");
    for (index, entry) in catalog.iter().enumerate() {
        let sql = fs::read(&entry.path).expect("read fixture migration");
        let checksum = format!("{:x}", Sha256::digest(&sql));
        connection
            .execute(
                "INSERT INTO schema_migrations(version,applied_at,checksum) VALUES(?1,?2,?3)",
                rusqlite::params![entry.version, index as i64, checksum],
            )
            .expect("record fixture migration");
    }
    connection
        .execute(
            "VACUUM INTO ?1",
            [fixture.to_str().expect("UTF-8 build directory")],
        )
        .expect("write test store image");

    catalog
        .into_iter()
        .filter(|entry| !entry.merged)
        .map(|entry| entry.version)
        .collect()
}
