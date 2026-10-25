//! The first-party packs, embedded in the binary and written into the packs
//! directory on first run so authors can read them as worked examples.
use std::path::Path;

pub struct BuiltinPack {
    pub folder: &'static str,
    pub files: &'static [(&'static str, &'static [u8])],
}

pub const BUILTIN: &[BuiltinPack] = &[
    BuiltinPack {
        folder: "budgie-classic",
        files: &[
            (
                "pet.json",
                include_bytes!("../../../packs/budgie-classic/pet.json"),
            ),
            (
                "LICENSE.txt",
                include_bytes!("../../../packs/budgie-classic/LICENSE.txt"),
            ),
            (
                "preview.png",
                include_bytes!("../../../packs/budgie-classic/preview.png"),
            ),
            (
                "sprites/alert.png",
                include_bytes!("../../../packs/budgie-classic/sprites/alert.png"),
            ),
            (
                "sprites/dance.png",
                include_bytes!("../../../packs/budgie-classic/sprites/dance.png"),
            ),
            (
                "sprites/fluff.png",
                include_bytes!("../../../packs/budgie-classic/sprites/fluff.png"),
            ),
            (
                "sprites/happy.png",
                include_bytes!("../../../packs/budgie-classic/sprites/happy.png"),
            ),
            (
                "sprites/dizzy.png",
                include_bytes!("../../../packs/budgie-classic/sprites/dizzy.png"),
            ),
            (
                "sprites/petted.png",
                include_bytes!("../../../packs/budgie-classic/sprites/petted.png"),
            ),
            (
                "sprites/idle.png",
                include_bytes!("../../../packs/budgie-classic/sprites/idle.png"),
            ),
            (
                "sprites/land.png",
                include_bytes!("../../../packs/budgie-classic/sprites/land.png"),
            ),
            (
                "sprites/move.png",
                include_bytes!("../../../packs/budgie-classic/sprites/move.png"),
            ),
            (
                "sprites/preen.png",
                include_bytes!("../../../packs/budgie-classic/sprites/preen.png"),
            ),
            (
                "sprites/sleep.png",
                include_bytes!("../../../packs/budgie-classic/sprites/sleep.png"),
            ),
            (
                "sprites/takeoff.png",
                include_bytes!("../../../packs/budgie-classic/sprites/takeoff.png"),
            ),
        ],
    },
    BuiltinPack {
        folder: "squirrel-red",
        files: &[
            (
                "pet.json",
                include_bytes!("../../../packs/squirrel-red/pet.json"),
            ),
            (
                "LICENSE.txt",
                include_bytes!("../../../packs/squirrel-red/LICENSE.txt"),
            ),
            (
                "preview.png",
                include_bytes!("../../../packs/squirrel-red/preview.png"),
            ),
            (
                "sprites/alert.png",
                include_bytes!("../../../packs/squirrel-red/sprites/alert.png"),
            ),
            (
                "sprites/climb.png",
                include_bytes!("../../../packs/squirrel-red/sprites/climb.png"),
            ),
            (
                "sprites/fall.png",
                include_bytes!("../../../packs/squirrel-red/sprites/fall.png"),
            ),
            (
                "sprites/flip.png",
                include_bytes!("../../../packs/squirrel-red/sprites/flip.png"),
            ),
            (
                "sprites/happy.png",
                include_bytes!("../../../packs/squirrel-red/sprites/happy.png"),
            ),
            (
                "sprites/dizzy.png",
                include_bytes!("../../../packs/squirrel-red/sprites/dizzy.png"),
            ),
            (
                "sprites/petted.png",
                include_bytes!("../../../packs/squirrel-red/sprites/petted.png"),
            ),
            (
                "sprites/idle.png",
                include_bytes!("../../../packs/squirrel-red/sprites/idle.png"),
            ),
            (
                "sprites/jump.png",
                include_bytes!("../../../packs/squirrel-red/sprites/jump.png"),
            ),
            (
                "sprites/land.png",
                include_bytes!("../../../packs/squirrel-red/sprites/land.png"),
            ),
            (
                "sprites/move.png",
                include_bytes!("../../../packs/squirrel-red/sprites/move.png"),
            ),
            (
                "sprites/nibble.png",
                include_bytes!("../../../packs/squirrel-red/sprites/nibble.png"),
            ),
            (
                "sprites/sleep.png",
                include_bytes!("../../../packs/squirrel-red/sprites/sleep.png"),
            ),
        ],
    },
    BuiltinPack {
        folder: "gecko-glass",
        files: &[
            (
                "pet.json",
                include_bytes!("../../../packs/gecko-glass/pet.json"),
            ),
            (
                "LICENSE.txt",
                include_bytes!("../../../packs/gecko-glass/LICENSE.txt"),
            ),
            (
                "preview.png",
                include_bytes!("../../../packs/gecko-glass/preview.png"),
            ),
            (
                "sprites/dizzy.png",
                include_bytes!("../../../packs/gecko-glass/sprites/dizzy.png"),
            ),
            (
                "sprites/petted.png",
                include_bytes!("../../../packs/gecko-glass/sprites/petted.png"),
            ),
            (
                "sprites/idle.png",
                include_bytes!("../../../packs/gecko-glass/sprites/idle.png"),
            ),
            (
                "sprites/move.png",
                include_bytes!("../../../packs/gecko-glass/sprites/move.png"),
            ),
        ],
    },
    BuiltinPack {
        folder: "cat-tabby",
        files: &[
            (
                "pet.json",
                include_bytes!("../../../packs/cat-tabby/pet.json"),
            ),
            (
                "LICENSE.txt",
                include_bytes!("../../../packs/cat-tabby/LICENSE.txt"),
            ),
            (
                "preview.png",
                include_bytes!("../../../packs/cat-tabby/preview.png"),
            ),
            (
                "sprites/alert.png",
                include_bytes!("../../../packs/cat-tabby/sprites/alert.png"),
            ),
            (
                "sprites/dizzy.png",
                include_bytes!("../../../packs/cat-tabby/sprites/dizzy.png"),
            ),
            (
                "sprites/fall.png",
                include_bytes!("../../../packs/cat-tabby/sprites/fall.png"),
            ),
            (
                "sprites/happy.png",
                include_bytes!("../../../packs/cat-tabby/sprites/happy.png"),
            ),
            (
                "sprites/idle.png",
                include_bytes!("../../../packs/cat-tabby/sprites/idle.png"),
            ),
            (
                "sprites/jump.png",
                include_bytes!("../../../packs/cat-tabby/sprites/jump.png"),
            ),
            (
                "sprites/land.png",
                include_bytes!("../../../packs/cat-tabby/sprites/land.png"),
            ),
            (
                "sprites/move.png",
                include_bytes!("../../../packs/cat-tabby/sprites/move.png"),
            ),
            (
                "sprites/petted.png",
                include_bytes!("../../../packs/cat-tabby/sprites/petted.png"),
            ),
            (
                "sprites/preen.png",
                include_bytes!("../../../packs/cat-tabby/sprites/preen.png"),
            ),
            (
                "sprites/sleep.png",
                include_bytes!("../../../packs/cat-tabby/sprites/sleep.png"),
            ),
            (
                "sprites/stretch.png",
                include_bytes!("../../../packs/cat-tabby/sprites/stretch.png"),
            ),
        ],
    },
];

pub fn is_builtin_directory(directory: &Path) -> bool {
    directory
        .file_name()
        .is_some_and(|name| BUILTIN.iter().any(|p| p.folder == name.to_string_lossy()))
}

/// Write any built-in pack that is not already on disk. Existing folders are
/// left alone so an author can edit a bundled pack and keep their changes.
pub fn restore(packs_dir: &Path, skip: &[String]) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(packs_dir)
        .map_err(|e| format!("Could not create the packs folder: {e}"))?;
    let mut written = Vec::new();
    for pack in BUILTIN {
        if skip.iter().any(|s| s == pack.folder) {
            continue;
        }
        let directory = packs_dir.join(pack.folder);
        if directory.exists() {
            continue;
        }
        for (name, bytes) in pack.files {
            let target = directory.join(name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
            }
            std::fs::write(&target, bytes)
                .map_err(|e| format!("Could not write {}: {e}", target.display()))?;
        }
        written.push(pack.folder.to_string());
    }
    Ok(written)
}

/// The published JSON Schema, copied beside scaffolded packs for autocomplete.
pub const SCHEMA: &str = include_str!("../../../schema/pet-pack-v1.schema.json");
