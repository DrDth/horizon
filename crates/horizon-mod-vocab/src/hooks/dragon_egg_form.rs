//! The `dragon_egg_form` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0194

pub trait Hooks {
    /// The Ender Dragon fight is won and a dragon egg may form at `pos`, on every win as
    /// Paper's `DragonEggFormEvent`. Set `cancel` to leave the block unchanged. Vanilla places
    /// the egg on the first win only: on a later one the event starts cancelled, and a
    /// listener receiving cancelled events that sets it back places the egg anyway.
    /// Server-side.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "cancel",
        neoforge = "none (NeoForge has no such event)",
        paper = "DragonEggFormEvent",
        cancellable = true,
        target = "horizon_client::game::native_hooks::base::GameLevel<'_>::native_dragon_egg_form"
    )]
    fn dragon_egg_form();
}
