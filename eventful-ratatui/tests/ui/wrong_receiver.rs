use eventful_ratatui::key_events;
use eventful_rs::*;

declare_shard!(Ui, runtime = main);

#[key_events]
trait Keys {
    #[key("d")]
    fn delete(&self);
}

#[eventful(shard = Ui)]
struct NotAListener {
    keys: KeysBridge,
}

fn connect(target: &ShardRc<NotAListener>) {
    let _ = target.keys.connect_to(target);
}

fn main() {}
