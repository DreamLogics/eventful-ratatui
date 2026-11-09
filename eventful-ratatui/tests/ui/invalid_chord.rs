use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[key("j", "ctrl+banana")]
    fn next(&self);
    #[key("hyperspace+x")]
    fn other(&self);
}

fn main() {}
