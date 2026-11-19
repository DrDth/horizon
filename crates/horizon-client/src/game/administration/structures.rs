//! Which structures contain a position (`StructureManager.getStructureAt`
//! and `getStructureWithPieceAt`), from the chunk's structure references and
//! the generator's cached starts.
use super::Game;
use horizon_world::worldgen::structure::start::StructureStart;
use std::sync::Arc;

impl Game<'_> {
    /// Structures reaching `pos`: (id, inside a piece). `getStructureAt`
    /// tests the start's whole box, `getStructureWithPieceAt` a piece's.
    pub(super) fn structures_reaching(&self, pos: [i32; 3]) -> Vec<(String, bool)> {
        self.structures_reaching_where(pos, &|_| true)
    }
    /// The same, for the structures whose id `wanted` takes: the others'
    /// starts aren't looked up.
    pub(super) fn structures_reaching_where(
        &self,
        pos: [i32; 3],
        wanted: &dyn Fn(&str) -> bool,
    ) -> Vec<(String, bool)> {
        structures_reaching_in(&self.world, &|| self.end.locator(), pos, wanted)
    }
    /// The structures with a piece containing `pos`.
    pub(super) fn structures_at(&self, pos: [i32; 3]) -> Vec<String> {
        self.structures_reaching(pos)
            .into_iter()
            .filter(|(_, piece)| *piece)
            .map(|(n, _)| n)
            .collect()
    }
}

type Locator<'a> = &'a dyn Fn() -> Option<Arc<horizon_client::vanillagen::VanillaTerrain>>;

/// [`Game::structures_reaching_where`] over what it reads: the world's chunks and, asked only
/// when a chunk refers to a structure, the terrain that locates the starts. A start the
/// generator no longer holds is built again, as vanilla's `getChunk(STRUCTURE_STARTS)` loads it.
pub(super) fn structures_reaching_in(
    world: &horizon_client::world::World,
    locator: Locator<'_>,
    pos: [i32; 3],
    wanted: &dyn Fn(&str) -> bool,
) -> Vec<(String, bool)> {
    let Some(chunk) = world.chunks.get(&(pos[0] >> 4, pos[2] >> 4)) else {
        return Vec::new();
    };
    if chunk.structure_refs.is_empty() {
        return Vec::new();
    }
    let Some(locator) = locator() else {
        return Vec::new();
    };
    reaching(chunk, pos, wanted, &|x, z| Some(locator.starts_at(x, z))).unwrap_or_default()
}

/// [`structures_reaching_in`] for a read that loads and builds nothing: from the starts the
/// generator holds already. None for an unloaded chunk, a level without the terrain that
/// locates its references, and a reference whose starts aren't held.
pub(super) fn cached_structures_reaching_in(
    world: &horizon_client::world::World,
    locator: Locator<'_>,
    pos: [i32; 3],
) -> Option<Vec<(String, bool)>> {
    let chunk = world.chunks.get(&(pos[0] >> 4, pos[2] >> 4))?;
    if chunk.structure_refs.is_empty() {
        return Some(Vec::new());
    }
    let locator = locator()?;
    reaching(chunk, pos, &|_| true, &|x, z| locator.cached_starts_at(x, z))
}

/// The structures `chunk` refers to whose start's box holds `pos`, and whether one of its
/// pieces does, as placed (`structureHasPieceAt`); `starts` gives a chunk's starts. None when
/// it can't give those of a chunk a wanted reference names.
fn reaching(
    chunk: &horizon_client::chunk::Chunk,
    pos: [i32; 3],
    wanted: &dyn Fn(&str) -> bool,
    starts: &dyn Fn(i32, i32) -> Option<Vec<Arc<StructureStart>>>,
) -> Option<Vec<(String, bool)>> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (id, (sx, sz)) in &chunk.structure_refs {
        if !wanted(id) {
            continue;
        }
        for start in starts(*sx, *sz)? {
            if *start.name != **id || !start.bounding_box.is_inside(pos) {
                continue;
            }
            let piece = start
                .pieces
                .iter()
                .any(|p| p.placed_bounding_box().is_inside(pos));
            match out.iter_mut().find(|(n, _)| *n == start.name) {
                Some(e) => e.1 |= piece,
                None => out.push((start.name.clone(), piece)),
            }
        }
    }
    Some(out)
}
