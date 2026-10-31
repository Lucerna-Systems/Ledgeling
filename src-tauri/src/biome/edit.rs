//! Decorate mode: the user picks a window, and then places, moves, flips and
//! removes props on its top edge. Every change goes through here and lands in
//! `Settings::biomes`; the overlay only reports where the pointer was.
use super::{clean_app, BiomeSetting, PlacedProp, MAX_BIOMES, MAX_PROPS};
use crate::packs::decor::DecorPack;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    /// Waiting for the user to click the window they want to decorate.
    Picking,
    Editing {
        biome: u64,
    },
}

/// What the overlay needs to show Decorate mode. Positions are physical
/// until the app turns them into overlay-local logical pixels.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditView {
    pub picking: bool,
    pub biome: Option<u64>,
    pub app: Option<String>,
    /// The host window's top edge: left end and length.
    pub edge_x: f64,
    pub edge_y: f64,
    pub edge_width: f64,
    pub props: Vec<PlacedProp>,
    pub palette: Vec<PaletteItem>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteItem {
    pub decor: String,
    pub prop: String,
    pub width: f64,
    pub height: f64,
}

pub fn palette(decor: &HashMap<String, Arc<DecorPack>>) -> Vec<PaletteItem> {
    let mut items: Vec<PaletteItem> = decor
        .values()
        .flat_map(|pack| {
            pack.manifest.props.iter().map(|(name, prop)| PaletteItem {
                decor: pack.manifest.id.clone(),
                prop: name.clone(),
                width: prop.width,
                height: prop.height,
            })
        })
        .collect();
    items.sort_by(|a, b| (&a.decor, &a.prop).cmp(&(&b.decor, &b.prop)));
    items
}

/// The biome for this app, made if there is none yet.
pub fn adopt(biomes: &mut Vec<BiomeSetting>, app: &str) -> Result<u64, String> {
    let app = clean_app(app).ok_or("This window's app can't hold a biome.")?;
    if let Some(biome) = biomes.iter().find(|b| b.app == app) {
        return Ok(biome.id);
    }
    if biomes.len() >= MAX_BIOMES {
        return Err(format!(
            "There are already {MAX_BIOMES} biomes. Remove one in Settings to make room."
        ));
    }
    let id = biomes.iter().map(|b| b.id).max().unwrap_or(0) + 1;
    biomes.push(BiomeSetting {
        id,
        app,
        props: Vec::new(),
    });
    Ok(id)
}

fn biome_mut(biomes: &mut [BiomeSetting], id: u64) -> Result<&mut BiomeSetting, String> {
    biomes
        .iter_mut()
        .find(|b| b.id == id)
        .ok_or_else(|| "That biome no longer exists.".to_string())
}

fn prop_mut(biomes: &mut [BiomeSetting], id: u64, index: usize) -> Result<&mut PlacedProp, String> {
    biome_mut(biomes, id)?
        .props
        .get_mut(index)
        .ok_or_else(|| "That prop is no longer there.".to_string())
}

fn along(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(0., 1.)
    } else {
        0.5
    }
}

pub fn place(
    biomes: &mut [BiomeSetting],
    decor: &HashMap<String, Arc<DecorPack>>,
    id: u64,
    pack: &str,
    prop: &str,
    x: f64,
) -> Result<usize, String> {
    let known = decor
        .get(pack)
        .is_some_and(|p| p.manifest.props.contains_key(prop));
    if !known {
        return Err("That prop's decor pack isn't installed.".into());
    }
    let biome = biome_mut(biomes, id)?;
    if biome.props.len() >= MAX_PROPS {
        return Err(format!("A biome holds up to {MAX_PROPS} props."));
    }
    biome.props.push(PlacedProp {
        decor: pack.into(),
        prop: prop.into(),
        x: along(x),
        flip: false,
    });
    Ok(biome.props.len() - 1)
}

pub fn move_to(biomes: &mut [BiomeSetting], id: u64, index: usize, x: f64) -> Result<(), String> {
    prop_mut(biomes, id, index)?.x = along(x);
    Ok(())
}

pub fn flip(biomes: &mut [BiomeSetting], id: u64, index: usize) -> Result<(), String> {
    let prop = prop_mut(biomes, id, index)?;
    prop.flip = !prop.flip;
    Ok(())
}

pub fn remove(biomes: &mut [BiomeSetting], id: u64, index: usize) -> Result<(), String> {
    let biome = biome_mut(biomes, id)?;
    if index >= biome.props.len() {
        return Err("That prop is no longer there.".into());
    }
    biome.props.remove(index);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn decor() -> HashMap<String, Arc<DecorPack>> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/decor-meadow");
        let pack = crate::packs::decor::load(&dir, true).unwrap();
        HashMap::from([(pack.manifest.id.clone(), Arc::new(pack))])
    }
    const MEADOW: &str = "com.perch.decor-meadow";

    #[test]
    fn adopting_an_app_twice_finds_the_same_biome_and_the_limit_is_explained() {
        let mut biomes = Vec::new();
        let first = adopt(&mut biomes, "Safari").unwrap();
        assert_eq!(adopt(&mut biomes, "safari").unwrap(), first);
        assert_eq!(biomes.len(), 1);
        for i in 1..MAX_BIOMES {
            adopt(&mut biomes, &format!("app{i}")).unwrap();
        }
        let full = adopt(&mut biomes, "one-more").unwrap_err();
        assert!(full.contains("Remove one"), "{full}");
        assert!(adopt(&mut biomes, "/bin/sh").is_err());
    }

    #[test]
    fn placing_clamps_checks_the_pack_and_stops_at_the_limit() {
        let decor = decor();
        let mut biomes = Vec::new();
        let id = adopt(&mut biomes, "safari").unwrap();
        assert_eq!(
            place(&mut biomes, &decor, id, MEADOW, "oak", 7.).unwrap(),
            0
        );
        assert_eq!(biomes[0].props[0].x, 1.);
        assert!(place(&mut biomes, &decor, id, MEADOW, "palm", 0.5).is_err());
        assert!(place(&mut biomes, &decor, id, "com.nobody.decor", "oak", 0.5).is_err());
        for _ in 1..MAX_PROPS {
            place(&mut biomes, &decor, id, MEADOW, "tuft", 0.3).unwrap();
        }
        let full = place(&mut biomes, &decor, id, MEADOW, "tuft", 0.3).unwrap_err();
        assert!(full.contains(&MAX_PROPS.to_string()), "{full}");
    }

    #[test]
    fn moving_flipping_and_removing_act_on_the_right_prop() {
        let decor = decor();
        let mut biomes = Vec::new();
        let id = adopt(&mut biomes, "safari").unwrap();
        place(&mut biomes, &decor, id, MEADOW, "oak", 0.2).unwrap();
        place(&mut biomes, &decor, id, MEADOW, "bush", 0.8).unwrap();
        move_to(&mut biomes, id, 1, f64::NAN).unwrap();
        flip(&mut biomes, id, 0).unwrap();
        assert_eq!(biomes[0].props[1].x, 0.5);
        assert!(biomes[0].props[0].flip);
        remove(&mut biomes, id, 0).unwrap();
        assert_eq!(biomes[0].props.len(), 1);
        assert_eq!(biomes[0].props[0].prop, "bush");
        assert!(remove(&mut biomes, id, 5).is_err());
        assert!(move_to(&mut biomes, 99, 0, 0.5).is_err());
    }

    #[test]
    fn the_palette_lists_every_installed_prop_in_a_stable_order() {
        let items = palette(&decor());
        assert_eq!(items.len(), 7);
        assert!(items.windows(2).all(|w| w[0].prop <= w[1].prop));
    }
}
