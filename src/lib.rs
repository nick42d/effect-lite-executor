#![no_std]

use core::{
    any::{Any, TypeId},
    marker::PhantomData,
    ops::DerefMut,
    pin::{pin, Pin},
    task::Poll,
};
use futures::StreamExt;
use pin_project::pin_project;
use smallbox::{smallbox, SmallBox};

type NoDepsEffect<T> = dyn futures::Stream<Item = T>;
type SmallboxSize = smallbox::space::S8;

struct Executor<T, const N: usize, A: allocator_api2::alloc::Allocator> {
    next_task_id: u64,
    last_polled: usize,
    task_list: heapless::Vec<ExecutorItem<T, A>, N>,
}

struct Enumerated<T>(usize, T);

struct ExecutorItem<T, A: allocator_api2::alloc::Allocator> {
    stream: allocator_api2::boxed::Box<NoDepsEffect<Enumerated<T>>, A>,
    task_id: u64,
    task_type_name: &'static str,
    task_type_id: TypeId,
}
impl<T, A: allocator_api2::alloc::Allocator> core::fmt::Debug for ExecutorItem<T, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}

enum EffectOutput<T> {
    Finished {
        task_id: u64,
        task_type_name: &'static str,
        task_type_id: TypeId,
    },
    Continuing {
        task_id: u64,
        task_type_name: &'static str,
        task_type_id: TypeId,
        output_n: usize,
        output: T,
    },
}

impl<T, const N: usize, A: allocator_api2::alloc::Allocator> Executor<T, N, A> {
    fn new() -> Self {
        Self {
            next_task_id: 0,
            task_list: heapless::Vec::new(),
            last_polled: 0,
        }
    }
    fn push(&mut self, stream: impl futures::Stream<Item = T> + Any + 'static, alloc: A) {
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        let task_type_name = core::any::type_name_of_val(&stream);
        let task_type_id = stream.type_id();
        self.task_list
            .push(ExecutorItem {
                stream: allocator_api2::unsize_box!(allocator_api2::boxed::Box::new_in(
                    stream.enumerate().map(|(idx, item)| Enumerated(idx, item)),
                    alloc,
                )),
                task_id,
                task_type_name,
                task_type_id,
            })
            .unwrap()
    }
    fn get_next(&mut self) -> GetNext<T, N> {
        GetNext {
            items: &mut self.task_list,
        }
    }
}

#[pin_project]
struct GetNext<'a, T, const N: usize> {
    #[pin]
    items: &'a mut heapless::Vec<ExecutorItem<T>, N>,
}

impl<'a, T, const N: usize> futures::Future for GetNext<'a, T, N> {
    type Output = Option<EffectOutput<T>>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let len = self.items.len();
        for idx in 0..len {
            // NOPANIC: len checked above
            let stream =
                unsafe { core::pin::Pin::new_unchecked(self.items[idx].stream.deref_mut()) };
            match futures::stream::Stream::poll_next(stream, cx) {
                Poll::Ready(Option::None) => {
                    // NOPANIC: len checked above
                    let task_id = self.items[idx].task_id;
                    let task_type_name = self.items[idx].task_type_name;
                    let task_type_id = self.items[idx].task_type_id;
                    self.items.remove(idx);
                    return Poll::Ready(Some(EffectOutput::Finished {
                        task_id,
                        task_type_name,
                        task_type_id,
                    }));
                }
                Poll::Ready(Option::Some(Enumerated(output_n, output))) => {
                    // NOPANIC: len checked above
                    let task_id = self.items[idx].task_id;
                    let task_type_name = self.items[idx].task_type_name;
                    let task_type_id = self.items[idx].task_type_id;
                    return Poll::Ready(Some(EffectOutput::Continuing {
                        task_id,
                        task_type_name,
                        task_type_id,
                        output_n,
                        output,
                    }));
                }
                Poll::Pending => (),
            }
        }
        Poll::Ready(None)
    }
}

#[cfg(feature = "tokio")]
mod tokio;

#[cfg(test)]
mod tests {}
