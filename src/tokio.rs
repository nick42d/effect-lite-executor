use effect_light::{stream_effect::EffectStream, Effect, EffectAsync, EffectExt};
use futures::{Stream, StreamExt};
use std::{any::Any, pin::Pin};
use tokio::task::JoinError;
struct Executor<T> {
    next_task_id: u64,
    task_list: futures::stream::SelectAll<Pin<Box<NoDepsEffect<T>>>>,
}

struct PanicError;

enum StreamResult<T> {
    StreamPanic(JoinError),
    StreamClosed,
    StreamCancel,
    Some(T),
}

fn spawn_stream<T>(
    mut s: impl Stream<Item = T> + Unpin + Send + 'static,
) -> impl Stream<Item = StreamResult<T>>
where
    T: Send + 'static,
{
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let handle = tokio::spawn(async move {
        while let Some(t) = s.next().await {
            tx.unbounded_send(t).unwrap()
        }
    });
    let task_stream = futures::stream::once(handle).map(|outcome| match outcome {
        Ok(()) => StreamResult::<T>::StreamClosed,
        Err(e) if e.is_panic() => StreamResult::<T>::StreamPanic(e),
        // Ignore the error if it's just a cancel, not a panic.
        Err(_) => StreamResult::<T>::StreamCancel,
    });
    let rx_stream = rx.map(|t| StreamResult::Some(t));
    rx_stream.chain(task_stream)
}

impl<T> Executor<T> {
    pub fn push_effect<D, E>(&mut self, dependency: D, e: E)
    where
        E: EffectStream<D, StreamItem = T>,
        E::Output: 'static,
    {
        let s = Box::new(e.resolve(dependency)) as Box<NoDepsEffect<T>>;
        self.next_task_id += 1;
        self.task_list.push(Box::into_pin(s))
    }
    pub async fn get_next(&mut self) -> Option<T> {
        self.task_list.next().await
    }
}
