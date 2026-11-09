use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[key("J")]
    fn next(&self);
    #[key("shift+j")]
    fn other(&self);
}

fn main() {}
