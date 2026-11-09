//! The bridge follows the trait's visibility, needs no imports at the call
//! site, and keeps raw identifiers and other attributes working.
mod keys {
    #[eventful_ratatui::key_events]
    pub trait Keys {
        /// Docs and other attributes stay on the event.
        #[key("ctrl+r")]
        #[allow(unused)]
        fn r#reload(&self);
        #[key("q", "esc")]
        fn quit(&self) {}
    }
}

fn main() {
    let bridge = keys::KeysBridge::default();
    assert_eq!(keys::KeysBridge::BINDINGS.len(), 3);
    assert_eq!(keys::KeysBridge::BINDINGS[0].1, "reload");
    assert_eq!(keys::KeysBridge::BINDINGS[0].0.to_string(), "ctrl+r");
    let _ = format!("{bridge:?}");
}
