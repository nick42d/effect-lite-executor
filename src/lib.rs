#![no_std]

use core::{ops::DerefMut, task::Poll};

use futures::StreamExt;
use smallbox::{smallbox, SmallBox};

type NoDepsEffect<T> = dyn futures::Stream<Item = T>;
type SmallboxSize = smallbox::space::S8;

struct Executor<T, const N: usize> {
    next_task_id: u64,
    task_list: heapless::Vec<SmallBox<NoDepsEffect<T>, SmallboxSize>, N>,
}

impl<T, const N: usize> Executor<T, N> {
    fn new() -> Self {
        Self {
            next_task_id: 0,
            task_list: heapless::Vec::new(),
        }
    }
    fn push(&mut self, t: impl futures::Stream<Item = T> + 'static) {
        self.task_list[0] = smallbox!(t)
    }
    fn get_next(&mut self) -> GetNext<T> {
        GetNext {
            items: self.task_list.as_mut_slice(),
        }
    }
}

struct GetNext<'a, T> {
    items: &'a mut [SmallBox<NoDepsEffect<T>, SmallboxSize>],
}

impl<'a, T> futures::Future for GetNext<'a, T> {
    type Output = Option<T>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        for item in self.items.iter_mut() {
            let item = unsafe { core::pin::Pin::new_unchecked(item.deref_mut()) };
            match futures::stream::Stream::poll_next(item, cx) {
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(t)) => return Poll::Ready(Some(t)),
                Poll::Pending => (),
            }
        }
        Poll::Pending
    }
}

#[cfg(feature = "tokio")]
mod tokio;

#[cfg(test)]
mod tests {}
