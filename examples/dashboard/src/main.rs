//! Worker progress shown as a list and a gauge.
//! Run with `cargo run -p dashboard`; `r` imports again, `q` quits. Click an
//! item or scroll to select.
use eventful_ratatui::prelude::*;
use eventful_rs::*;
use std::cell::{Cell, RefCell};
mod importer;
use importer::*;

declare_shard!(Ui, runtime = main);

#[eventful(shard = Ui)]
struct Dashboard {
    items: RefCell<Vec<String>>,
    list: RefCell<ListState>,
    /// Where the list items were drawn, for clicks.
    list_rows: Cell<Rect>,
    total: Cell<usize>,
    running: Cell<bool>,
    importer: ShardRcHandle<Importer>,
    ui: Invalidate,
    keys: Keymap<Dashboard>,
}

impl ImportEvents for Dashboard {
    fn started(&self, total: usize) {
        self.items.borrow_mut().clear();
        self.list.borrow_mut().select(None);
        self.total.set(total);
        self.running.set(true);
        self.ui.invalidate();
    }
    fn imported(&self, name: String) {
        self.items.borrow_mut().push(name);
        self.ui.invalidate();
    }
    fn finished(&self) {
        self.running.set(false);
        self.ui.invalidate();
    }
}

impl Dashboard {
    fn select(&self, f: impl FnOnce(&mut ListState)) {
        f(&mut self.list.borrow_mut());
        self.ui.invalidate();
    }
    /// Select the item drawn at `row`, if any.
    fn click(&self, row: u16) {
        let rows = self.list_rows.get();
        if !(rows.y..rows.bottom()).contains(&row) {
            return;
        }
        let index = self.list.borrow().offset() + usize::from(row - rows.y);
        if index < self.items.borrow().len() {
            self.select(|list| list.select(Some(index)));
        }
    }
    fn on_mouse(&self, mouse: &MouseEvent) -> Handled {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => self.click(mouse.row),
            MouseEventKind::ScrollDown => self.select(ListState::select_next),
            MouseEventKind::ScrollUp => self.select(ListState::select_previous),
            _ => return Handled::No,
        }
        Handled::Yes
    }
    fn restart(&self) {
        if !self.running.get() {
            self.importer.start();
        }
    }
}

impl Component for Dashboard {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [list_area, gauge_area, help_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(area);

        let items = self.items.borrow();
        self.list_rows.set(list_area.inner(Margin::new(1, 1)));
        let list = List::new(items.iter().map(String::as_str))
            .block(Block::bordered().title(" Imported "))
            .highlight_symbol("> ")
            .highlight_style(Style::new().reversed());
        StatefulWidget::render(list, list_area, buf, &mut self.list.borrow_mut());

        let total = self.total.get().max(1);
        let status = if self.running.get() {
            "importing"
        } else {
            "done"
        };
        Gauge::default()
            .block(Block::bordered().title(format!(" Progress: {status} ")))
            .gauge_style(Style::new().green())
            .ratio(items.len() as f64 / total as f64)
            .label(format!("{}/{}", items.len(), self.total.get()))
            .render(gauge_area, buf);

        let help = if self.running.get() {
            "j/k, click: select  q: quit"
        } else {
            "j/k, click: select  r: import again  q: quit"
        };
        Line::from(help).dim().render(help_area, buf);
    }
    fn on_input(&self, event: &InputEvent) -> Handled {
        match event {
            InputEvent::Mouse(mouse) => self.on_mouse(mouse),
            _ => self.keys.dispatch(self, event),
        }
    }
}

#[sharded_main(Ui)]
async fn main() -> eventful_ratatui::Result<()> {
    let importer = Importer::new().await?;
    let dash = Dashboard::bind_local(Dashboard {
        items: RefCell::default(),
        list: RefCell::default(),
        list_rows: Cell::default(),
        total: Cell::new(0),
        running: Cell::new(false),
        importer: importer.clone(),
        ui: Invalidate::new(),
        keys: Keymap::new()
            .bind("j", |d: &Dashboard| d.select(ListState::select_next))
            .bind("down", |d: &Dashboard| d.select(ListState::select_next))
            .bind("k", |d: &Dashboard| d.select(ListState::select_previous))
            .bind("up", |d: &Dashboard| d.select(ListState::select_previous))
            .bind("r", Dashboard::restart)
            .bind("q", |_| quit())
            .bind("esc", |_| quit()),
        events: Default::default(),
    })?;
    let _subscription = importer.events().role::<()>().connect(&dash).scoped();
    importer.start();
    Tui::new().mouse(true).run(&dash).await
}
