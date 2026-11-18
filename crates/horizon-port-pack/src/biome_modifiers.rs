//! Biome modifiers of mods' own types whose fields say everything the game
//! needs, written as NeoForge's (`add_features`, `remove_features`,
//! `add_spawns`). The game reads only NeoForge's and skips the others, so the
//! ores, plants, nests and fish they add never appeared. Each type is read
//! from its fields alone: what it holds beyond NeoForge's (a temperature
//! range, a config switch) is lost and counted. A type whose biomes or
//! features are in the mod's Java (`cataclysm:cataclysm_mob_spawns`, only a
//! `type`) stays as it is.
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Keys a modifier may carry besides its own, kept as they are.
const KEPT: [&str; 2] = ["neoforge:conditions", "forge:conditions"];

/// `v` rewritten as NeoForge's modifier when its type is one known here; the
/// type it had. The fields NeoForge has no place for go to `lost`
/// (`<type>: <field>`).
pub fn translate(v: &mut Value, lost: &mut BTreeMap<String, usize>) -> Option<String> {
    let kind = v.get("type")?.as_str()?.to_owned();
    let (ns, path) = kind.split_once(':')?;
    let mut lose = |field: &str| {
        if v.get(field).is_some() {
            *lost.entry(format!("{kind}: {field}")).or_default() += 1;
        }
    };
    let new = match (ns, path) {
        // One ore feature where the mod's code puts its ores.
        ("mysticalagriculture" | "mysticalagradditions", _) | ("bigreactors", _)
            if path.ends_with("_ore") || path == "soulstone" || path.starts_with("oregen_") =>
        {
            add_features(v.get("biomes")?, v.get("feature")?, "underground_ores")
        }
        ("actuallyadditions", "bool_config_feature_modifier") => {
            lose("boolConfig");
            add_features(
                v.get("biomes")?,
                v.get("features")?,
                v.get("step")?.as_str()?,
            )
        }
        ("repurposed_structures", "additions_modifier" | "additions_temperature_modifier") => {
            lose("biome_temperature_allowed");
            add_features(
                v.get("biomes")?,
                v.get("feature")?,
                v.get("step")?.as_str()?,
            )
        }
        ("repurposed_structures", "removals_modifier") => json!({
            "type": "neoforge:remove_features",
            "biomes": v.get("biomes")?,
            "features": v.get("feature")?,
            "steps": v.get("step")?,
        }),
        ("productivebees", "nest") => {
            lose("maxTemperature");
            lose("minTemperature");
            let step = v.get("generation_stage")?.as_str()?.to_ascii_lowercase();
            add_features(v.get("biomes")?, v.get("features")?, &step)
        }
        ("farmersdelight", "add_features_by_filter") => {
            lose("min_temperature");
            lose("max_temperature");
            let biomes = selector(v.get("allowed_biomes")?, false, v.get("denied_biomes"));
            add_features(&biomes, v.get("features")?, v.get("step")?.as_str()?)
        }
        ("aquaculture", "fish_spawn" | "mob_spawn") => {
            let all = v.get("and").and_then(Value::as_bool).unwrap_or(false);
            let biomes = selector(v.get("includeBiomes")?, all, v.get("excludeBiomes"));
            json!({"type": "neoforge:add_spawns", "biomes": biomes, "spawners": v.get("spawn")?})
        }
        _ => return None,
    };
    let mut new = match new {
        Value::Object(o) => o,
        _ => return None,
    };
    for key in KEPT {
        if let Some(x) = v.get(key) {
            new.insert(key.to_owned(), x.clone());
        }
    }
    *v = Value::Object(new);
    Some(kind)
}

fn add_features(biomes: &Value, features: &Value, step: &str) -> Value {
    json!({"type": "neoforge:add_features", "biomes": biomes, "features": features,
        "step": step})
}

/// The biomes in `include` (any of them, or with `all` each), less those in
/// `exclude`, as NeoForge's biome selectors: a list may not hold tags, so
/// each entry is a selector of its own.
fn selector(include: &Value, all: bool, exclude: Option<&Value>) -> Value {
    let included = match include {
        Value::Array(list) if list.len() == 1 => list[0].clone(),
        Value::Array(list) if all || list.iter().any(is_tag) => {
            let kind = if all { "neoforge:and" } else { "neoforge:or" };
            json!({"type": kind, "values": list})
        }
        other => other.clone(),
    };
    match exclude.filter(|e| !e.is_null() && e.as_array().is_none_or(|l| !l.is_empty())) {
        Some(excluded) => {
            let excluded = match excluded {
                Value::Array(list) if list.len() == 1 => list[0].clone(),
                Value::Array(list) if list.iter().any(is_tag) => {
                    json!({"type": "neoforge:or", "values": list})
                }
                other => other.clone(),
            };
            let mut not = Map::new();
            not.insert("type".into(), json!("neoforge:not"));
            not.insert("value".into(), excluded);
            json!({"type": "neoforge:and", "values": [included, Value::Object(not)]})
        }
        None => included,
    }
}

fn is_tag(v: &Value) -> bool {
    v.as_str().is_some_and(|s| s.starts_with('#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_modifiers_become_neoforges() {
        let mut lost = BTreeMap::new();
        let mut ore = json!({"type": "mysticalagriculture:inferium_ore",
            "biomes": "#is_overworld", "feature": "mysticalagriculture:inferium_ore"});
        assert_eq!(
            translate(&mut ore, &mut lost).as_deref(),
            Some("mysticalagriculture:inferium_ore")
        );
        assert_eq!(
            ore,
            json!({"type": "neoforge:add_features", "biomes": "#is_overworld",
                "features": "mysticalagriculture:inferium_ore", "step": "underground_ores"})
        );
        let mut quartz = json!({"type": "actuallyadditions:bool_config_feature_modifier",
            "biomes": "#minecraft:is_overworld", "boolConfig": "generateQuartz",
            "features": "actuallyadditions:ore_black_quartz", "step": "underground_ores"});
        translate(&mut quartz, &mut lost).unwrap();
        assert_eq!(quartz["type"], "neoforge:add_features");
        assert_eq!(quartz.get("boolConfig"), None);
        let mut nest = json!({"type": "productivebees:nest", "biomes": "#c:is_snowy",
            "generation_stage": "TOP_LAYER_MODIFICATION",
            "features": ["productivebees:snow_nest"], "maxTemperature": 0});
        translate(&mut nest, &mut lost).unwrap();
        assert_eq!(nest["step"], "top_layer_modification");
        let mut removal = json!({"type": "repurposed_structures:removals_modifier",
            "biomes": "#x:deep", "feature": "minecraft:monster_room_deep",
            "step": "underground_structures"});
        translate(&mut removal, &mut lost).unwrap();
        assert_eq!(
            removal,
            json!({"type": "neoforge:remove_features", "biomes": "#x:deep",
                "features": "minecraft:monster_room_deep", "steps": "underground_structures"})
        );
        let mut beets = json!({"type": "farmersdelight:add_features_by_filter",
            "allowed_biomes": "#minecraft:is_beach", "denied_biomes": ["minecraft:stony_shore"],
            "features": "farmersdelight:patch_wild_beetroots", "step": "vegetal_decoration",
            "min_temperature": 0.2});
        translate(&mut beets, &mut lost).unwrap();
        assert_eq!(
            beets["biomes"],
            json!({"type": "neoforge:and", "values": ["#minecraft:is_beach",
                {"type": "neoforge:not", "value": "minecraft:stony_shore"}]})
        );
        let mut trout = json!({"type": "aquaculture:fish_spawn",
            "spawn": {"type": "aquaculture:rainbow_trout", "weight": 6, "minCount": 1,
                "maxCount": 2},
            "includeBiomes": ["#c:is_cold", "#c:is_ocean"], "and": true,
            "excludeBiomes": ["#aquaculture:empty"],
            "neoforge:conditions": [{"type": "neoforge:true"}]});
        translate(&mut trout, &mut lost).unwrap();
        assert_eq!(
            trout,
            json!({"type": "neoforge:add_spawns",
                "biomes": {"type": "neoforge:and", "values": [
                    {"type": "neoforge:and", "values": ["#c:is_cold", "#c:is_ocean"]},
                    {"type": "neoforge:not", "value": "#aquaculture:empty"}]},
                "spawners": {"type": "aquaculture:rainbow_trout", "weight": 6, "minCount": 1,
                    "maxCount": 2},
                "neoforge:conditions": [{"type": "neoforge:true"}]})
        );
        assert_eq!(
            lost.keys().collect::<Vec<_>>(),
            [
                "actuallyadditions:bool_config_feature_modifier: boolConfig",
                "farmersdelight:add_features_by_filter: min_temperature",
                "productivebees:nest: maxTemperature",
            ]
        );
        // Biomes and features in the mod's Java: left alone.
        let mut code = json!({"type": "cataclysm:cataclysm_mob_spawns"});
        assert_eq!(translate(&mut code, &mut lost), None);
        assert_eq!(code, json!({"type": "cataclysm:cataclysm_mob_spawns"}));
    }
}
