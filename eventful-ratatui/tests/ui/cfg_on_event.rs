use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[cfg(unix)]
    #[key("d")]
    fn delete(&self);
}

fn main() {}
