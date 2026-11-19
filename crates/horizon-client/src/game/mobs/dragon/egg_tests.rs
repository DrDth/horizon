//! `dragon_egg_form` on every won fight, as Paper's `DragonEggFormEvent`: vanilla's egg on the
//! first win, and on a later one an event that starts cancelled, which a listener receiving
//! cancelled events can clear to place an egg anyway.
use mod_api::{DragonEggFormEvent, Flow, Hooks, LevelHost};
use std::sync::{Arc, Mutex};

type Heard = Arc<Mutex<Vec<bool>>>;

/// Hears the event as most mods do: only while nothing cancelled it.
struct Plain(Heard);
#[mod_api::__subscriptions]
impl Hooks for Plain {
    fn dragon_egg_form(&self, _: &dyn LevelHost, event: &mut DragonEggFormEvent) -> Flow {
        self.0.lock().unwrap().push(event.cancelled);
        Flow::Continue
    }
}

/// Places an egg on every win, as More Dragon Eggs does.
struct EveryWin(Heard);
#[mod_api::__subscriptions]
impl Hooks for EveryWin {
    #[listener(receive_cancelled)]
    fn dragon_egg_form(&self, _: &dyn LevelHost, event: &mut DragonEggFormEvent) -> Flow {
        self.0.lock().unwrap().push(event.cancelled);
        event.cancelled = false;
        Flow::Continue
    }
}

/// The heights of the dragon eggs on the podium's column after two won fights.
fn eggs_after_two_wins(mods: mod_api::Mods) -> Vec<i32> {
    let mut eggs = Vec::new();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            for (x, z) in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
                game.world
                    .chunks
                    .insert((x, z), Arc::new(horizon_client::chunk::Chunk::empty(x, z)));
            }
            game.dragon_killed();
            game.dragon_killed();
            let egg = game
                .pack
                .assets
                .registry
                .default_state("minecraft:dragon_egg")
                .unwrap();
            eggs = (0..256)
                .filter(|&y| game.world.block(0, y, 0) == egg)
                .collect();
        });
    });
    eggs
}

#[test]
#[ignore = "daily"]
fn a_later_win_asks_cancelled_and_a_listener_clearing_it_places_another_egg() {
    // Vanilla: the egg on the podium's pillar (bedrock up to y 64 at the default rim) once.
    assert_eq!(eggs_after_two_wins(Vec::new().into()), [65]);

    let plain = Heard::default();
    let every_win = Heard::default();
    let mods: mod_api::Mods = vec![
        mod_api::ModEntry::new("plain", Box::new(Plain(plain.clone()))),
        mod_api::ModEntry::new("every_win", Box::new(EveryWin(every_win.clone()))),
    ]
    .into();
    // The second egg lands on the first, as the game places the first on the pillar.
    assert_eq!(eggs_after_two_wins(mods), [65, 66]);
    assert_eq!(*plain.lock().unwrap(), [false], "a plain listener hears the first win only");
    assert_eq!(
        *every_win.lock().unwrap(),
        [false, true],
        "the later win arrives cancelled"
    );
}
