//! Worker progress shown as a list and a gauge.
//! Run with `cargo run -p dashboard`.
use eventful_rs::*;

declare_shard!(Ui, runtime = main);

#[sharded_main(Ui)]
async fn main() -> Result<(), DeliveryError> {
    // Phase 2: spawn an importer worker and run `Dashboard` through `run_tui`.
    Ok(())
}
