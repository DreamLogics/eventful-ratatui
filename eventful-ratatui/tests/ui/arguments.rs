use eventful_ratatui::key_events;

#[key_events(shard = Ui)]
trait Keys {
    #[key("d")]
    fn delete(&self);
}

fn main() {}
