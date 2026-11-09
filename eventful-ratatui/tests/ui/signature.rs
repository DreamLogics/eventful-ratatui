use eventful_ratatui::key_events;

#[key_events]
trait Keys {
    #[key("j")]
    fn with_argument(&self, count: u32);
    #[key("k")]
    fn with_return(&self) -> bool;
    #[key("l")]
    fn without_receiver();
}

fn main() {}
