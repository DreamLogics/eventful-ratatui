use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[key("j")]
    fn next(&self);
    fn unbound(&self);
    #[key()]
    fn empty(&self);
}

fn main() {}
