use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Result, anyhow};
use skim::prelude::*;

use crate::source::Source;

/// Identity is independent of the escaped, searchable label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Tool(String),
    Path(std::path::PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub value: Value,
}

impl Entry {
    pub fn new(label: impl Into<String>, value: Value) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

impl SkimItem for Entry {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.label)
    }
}

pub struct Request {
    pub prompt: String,
    pub header: String,
    pub multi: bool,
    /// Skim key bindings, e.g. `ctrl-o:accept(open)`.
    pub bindings: Vec<String>,
    pub source: Box<dyn Source>,
}

impl Request {
    pub fn new(
        prompt: impl Into<String>,
        header: impl Into<String>,
        source: impl Source + 'static,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            header: header.into(),
            multi: false,
            bindings: Vec::new(),
            source: Box::new(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Named accept action; ordinary Enter is `None`.
    pub action: Option<String>,
    pub entries: Vec<Entry>,
}

/// Implement this interface to test tools without a TTY.
pub trait Picker {
    fn pick(&mut self, request: Request) -> Result<Option<Selection>>;
}

#[derive(Default)]
pub struct SkimPicker;

/// Owns the producer lifecycle, including error and unwind paths.
struct Producer {
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
}

impl Producer {
    fn start(source: Box<dyn Source>) -> (Self, SkimItemReceiver) {
        // Skim 5.9 receives batches. Flush the first candidate immediately,
        // then amortize channel/allocator overhead across batches of 64.
        let (sender, receiver) = bounded::<Vec<Arc<dyn SkimItem>>>(8);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let worker = thread::spawn(move || {
            let send = |batch| {
                let mut pending = Some(batch);
                while !stop.load(Ordering::Relaxed) {
                    match sender.try_send_option(&mut pending) {
                        Ok(true) => return true,
                        Ok(false) => thread::sleep(Duration::from_millis(2)),
                        Err(_) => return false,
                    }
                }
                false
            };
            let mut first = true;
            let mut batch = Vec::with_capacity(64);
            let result = source.produce(&mut |entry| {
                if stop.load(Ordering::Relaxed) {
                    return false;
                }
                batch.push(Arc::new(entry) as Arc<dyn SkimItem>);
                if first || batch.len() == 64 {
                    first = false;
                    return send(std::mem::replace(&mut batch, Vec::with_capacity(64)));
                }
                true
            });
            if !batch.is_empty() {
                send(batch);
            }
            result
        });
        (
            Self {
                cancelled,
                worker: Some(worker),
            },
            receiver,
        )
    }

    fn finish(&mut self) -> Result<()> {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow!("candidate producer panicked"))??;
        }
        Ok(())
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl SkimPicker {
    fn run(&mut self, request: Request, filter: Option<String>) -> Result<Option<Selection>> {
        let mut builder = SkimOptionsBuilder::default();
        builder
            .height("100%")
            .reverse(true)
            .no_mouse(true)
            .prompt(request.prompt)
            .header(request.header)
            .multi(request.multi)
            .bind(request.bindings);
        if let Some(filter) = filter {
            builder.filter(filter);
        }
        let options = builder.build()?;
        let (mut producer, receiver) = Producer::start(request.source);
        let output = Skim::run_with(options, Some(receiver)).map_err(|e| anyhow!("skim: {e:#}"));
        let source_result = producer.finish();
        let output = output?;
        source_result?;
        if output.is_abort {
            return Ok(None);
        }
        let action = match output.final_event {
            Event::Action(Action::Accept(action)) => action,
            _ => None,
        };
        let entries = output
            .selected_items
            .iter()
            .map(|matched| {
                matched
                    .item
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Entry>()
                    .cloned()
                    .ok_or_else(|| anyhow!("unexpected skim candidate type"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(Selection { action, entries }))
    }
}

impl Picker for SkimPicker {
    fn pick(&mut self, request: Request) -> Result<Option<Selection>> {
        self.run(request, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, time::Instant};

    #[test]
    fn skim_preserves_typed_identity_in_filter_mode() -> Result<()> {
        let path = PathBuf::from("a\n'b");
        let entry = Entry::new("unique", Value::Path(path));
        let output = SkimPicker.run(
            Request::new("> ", "", vec![entry.clone()]),
            Some("uniq".into()),
        )?;
        assert_eq!(output.unwrap().entries, vec![entry]);
        Ok(())
    }

    #[test]
    fn batches_preserve_all_candidates_and_flush_the_tail() -> Result<()> {
        let entries: Vec<_> = (0..4097)
            .map(|i| Entry::new(format!("candidate-{i:05}"), Value::Tool(i.to_string())))
            .collect();
        let selection = SkimPicker
            .run(
                Request::new("> ", "", entries.clone()),
                Some("candidate".into()),
            )?
            .expect("filter output");
        assert_eq!(selection.entries.len(), entries.len());
        let mut actual: Vec<_> = selection.entries.into_iter().map(|e| e.label).collect();
        actual.sort();
        assert_eq!(
            actual,
            entries.into_iter().map(|e| e.label).collect::<Vec<_>>()
        );
        Ok(())
    }

    struct Endless;
    impl Source for Endless {
        fn produce(self: Box<Self>, emit: &mut dyn FnMut(Entry) -> bool) -> Result<()> {
            while emit(Entry::new("x", Value::Tool("x".into()))) {}
            Ok(())
        }
    }

    #[test]
    fn cancellation_joins_a_backpressured_producer() -> Result<()> {
        let (mut producer, _receiver) = Producer::start(Box::new(Endless));
        thread::sleep(Duration::from_millis(30));
        let start = Instant::now();
        producer.finish()?;
        assert!(start.elapsed() < Duration::from_secs(1));
        Ok(())
    }

    struct Fails;
    impl Source for Fails {
        fn produce(self: Box<Self>, _: &mut dyn FnMut(Entry) -> bool) -> Result<()> {
            anyhow::bail!("source failed")
        }
    }

    #[test]
    fn source_errors_are_reported() {
        let result = SkimPicker.run(Request::new("> ", "", Fails), Some(String::new()));
        assert!(result.unwrap_err().to_string().contains("source failed"));
    }
}
