//! A Tokio network worker, an input box and focus switching.
//! Run with `cargo run -p chat`.
use eventful_rs::*;

declare_shard!(Ui, runtime = main);

#[sharded_main(Ui)]
async fn main() -> Result<(), DeliveryError> {
    // Phase 3: connect a Tokio worker and switch focus between components.
    Ok(())
}
