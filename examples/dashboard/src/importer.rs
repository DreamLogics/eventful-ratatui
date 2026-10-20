//! A worker that "imports" products slowly and reports progress.
use eventful_rs::*;
use futures_timer::Delay;
use std::{cell::Cell, time::Duration};

declare_shard!(pub Worker, runtime = std);
use_shard!(shard = Worker);

/// Progress of an import run.
#[events]
pub trait ImportEvents {
    /// A run of `total` products started.
    fn started(&self, total: usize) {}
    /// One product was imported.
    fn imported(&self, name: String);
    /// The run finished.
    fn finished(&self) {}
}

const PRODUCTS: &[&str] = &[
    "Apples", "Pears", "Plums", "Cherries", "Apricots", "Peaches", "Figs", "Grapes", "Lemons",
    "Limes", "Oranges", "Mangoes", "Papayas", "Kiwis", "Melons", "Quinces",
];

/// Import state stays on the worker thread.
#[eventful(ImportEvents)]
pub struct Importer {
    /// Runs started so far, to tell runs apart.
    runs: Cell<usize>,
}

impl Importer {
    pub async fn new() -> Result<ShardRcHandle<Self>, InvokeError> {
        Self::spawn(|| Self {
            runs: Cell::new(0),
            events: Default::default(),
        })
        .await
    }
}

#[asynchronize(pub)]
impl Importer {
    /// Import every product, one every 150 ms.
    #[action]
    pub async fn start(&self) {
        let run = self.runs.get() + 1;
        self.runs.set(run);
        self.events.started().emit(PRODUCTS.len());
        for product in PRODUCTS {
            Delay::new(Duration::from_millis(150)).await;
            self.events
                .imported()
                .emit(format!("{product} (run {run})"));
        }
        self.events.finished().emit();
    }
}
