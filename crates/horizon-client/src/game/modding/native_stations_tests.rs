//! A workstation a mod opens with no block (game/modding/native_stations.rs).
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
use crate::game::native_item_places::tests::{guest, open_to_lan};
use horizon_client::{
    items::ItemStack,
    lan::{Out, PlayerSender},
};
use horizon_protocol::{
    buf::Reader,
    packets::play::{Clientbound as C, ContainerClose},
};
use mod_api::{Hooks, ServerHost, StationClosingEvent};
use std::sync::{Arc, Mutex, mpsc};

/// Guest 1 (UUID 123) carrying a stick, on a world open to LAN, and what their client is sent.
pub(in crate::game) fn stick_guest(game: &mut crate::game::Game<'_>) -> mpsc::Receiver<Out> {
    open_to_lan(game);
    let (tx, rx) = mpsc::channel();
    let mut player = guest(1);
    player.sender = PlayerSender::new(tx, Arc::default());
    player.inventory[5] = Some(ItemStack::new("minecraft:stick", 1));
    game.lan.as_mut().unwrap().hub.guests.push(player);
    rx
}

/// The packets sent since the last call.
pub(in crate::game) fn sent(rx: &mpsc::Receiver<Out>) -> Vec<C> {
    rx.try_iter()
        .flat_map(|out| match out {
            Out::Packet(body) => vec![body],
            Out::Packets(bodies) => bodies,
            _ => Vec::new(),
        })
        .map(|body| {
            let mut reader = Reader::new(&body);
            let id = reader.varint().unwrap();
            C::decode(id, reader.rest()).unwrap()
        })
        .collect()
}

pub(in crate::game) fn menu_key(id: &str) -> mod_api::MenuKey {
    mod_api::__private::menu_keys()
        .into_iter()
        .find(|key| mod_api::__private::menu_key_id(*key).as_deref() == Some(id))
        .unwrap()
}

/// A crafting table opened with no block starts with the mod's grid, works where the player
/// stands, stays open while they carry the mod's item and closes once they don't. As it closes
/// the mods keep the stacks of the slots they empty; a stack they put in a slot is ignored.
#[test]
#[ignore = "daily"]
fn a_table_with_no_block_keeps_to_its_item_and_lets_the_mods_keep_its_grid() {
    const CASE: &str = "HORIZON_PORTABLE_TABLE_CHILD";
    if run_fresh_cases(
        "game::native_stations_tests::a_table_with_no_block_keeps_to_its_item_and_lets_the_mods_keep_its_grid",
        CASE,
        &["keep"],
    ) {
        return;
    }
    static HEARD: Mutex<Vec<(i32, String, usize)>> = Mutex::new(Vec::new());
    struct Keep;
    #[mod_api::__subscriptions]
    impl Hooks for Keep {
        fn station_closing(&self, _: &mut dyn ServerHost, event: &mut StationClosingEvent<'_>) {
            let held = event.inputs.iter().flatten().count();
            HEARD
                .lock()
                .unwrap()
                .push((event.window, event.menu.to_owned(), held));
            event.inputs[0] = None;
            event.inputs[2] = event.inputs[1].clone();
        }
    }
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("keep", Box::new(Keep))].into();
    mod_platform::__private::install(mods.clone());
    crate::game::api_values::install_parts(&mods).unwrap();
    with_game(|game| {
        let rx = stick_guest(game);
        let player = game.player(123).unwrap();
        let plank = Some(ItemStack::new("minecraft:oak_planks", 1));
        let grid = vec![plank.clone(), plank.clone(), None, plank.clone(), plank];
        let stick = mod_api::__private::item_key("minecraft:stick").unwrap();
        let window = game
            .player_open_station(player, menu_key("minecraft:crafting"), grid, vec![stick])
            .unwrap()
            .unwrap();
        let table = |game: &crate::game::Game<'_>| {
            let menus = game.lan.as_ref().unwrap().menus.get(&1)?;
            let (w, _, menu, pos) = menus.crafting.table.as_ref()?;
            Some((*w, menu.slot(1).cloned(), menu.slot(0).map(|r| r.item.clone()), *pos))
        };
        let (w, first, result, pos) = table(game).unwrap();
        assert_eq!(w, window);
        assert_eq!(first.map(|s| s.item), Some("minecraft:oak_planks".into()));
        assert_eq!(result.as_deref(), Some("minecraft:crafting_table"));
        assert_eq!(pos, [0, 0, 0], "the block the player stands in");
        // No crafting table is there: the stick keeps it open.
        game.lan_tables_tick();
        assert!(table(game).is_some());
        sent(&rx);

        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| g.inventory[5] = None);
        game.lan_tables_tick();
        assert!(table(game).is_none());
        assert!(!game.is_portable_station(1, window));
        assert_eq!(
            *HEARD.lock().unwrap(),
            [(window, "minecraft:crafting".to_owned(), 4)]
        );
        assert!(sent(&rx).iter().any(|p| matches!(
            p,
            C::ContainerClose(ContainerClose { container }) if *container == window
        )));
        // Four planks in the grid: the mod kept one, and the one it put in an empty slot
        // made nothing.
        let planks: u32 = game.lan.as_ref().unwrap().hub.guests[0]
            .inventory
            .iter()
            .flatten()
            .filter(|s| &*s.item == "minecraft:oak_planks")
            .map(|s| s.count)
            .sum();
        assert_eq!(planks, 3);
    });
}
