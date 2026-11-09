use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[key("d")]
    fn dispatch(&self);
}

fn main() {}
