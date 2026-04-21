//! A concurrent (not parrallel) executor for [effect_light::Effect]s.
//! The executor is no-std, however an allocator is required.
//! # Usage example
//! ```
//!
//! ```
#![no_std]

use allocator_api2::{alloc::Allocator, boxed::Box, vec::Vec};
use core::{
    any::{Any, TypeId},
    marker::PhantomData,
    pin::Pin,
    task::Poll,
};
use effect_light::Effect;
use futures::StreamExt;

type NoDepsEffect<T> = dyn futures::Stream<Item = T>;
type SmallboxSize = smallbox::space::S8;

struct Executor<E, T, D, A: Allocator, F1> {
    next_task_id: u64,
    last_polled: usize,
    task_list: Vec<ExecutorItem<T, A>, A>,
    on_push_cb: Option<F1>,
    effect_type: PhantomData<E>,
    dependencies: D,
}

struct Enumerated<T>(usize, T);

struct ExecutorItem<T, A: Allocator> {
    stream: Pin<Box<NoDepsEffect<Enumerated<T>>, A>>,
    task_id: u64,
    task_type_name: &'static str,
    task_type_id: TypeId,
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

impl<E: effect_light::Effect<T>, T, A: Allocator, D> Executor<E, T, D, A, ()> {
    fn new(dependencies: D, alloc: A) -> Self {
        Self {
            next_task_id: 0,
            task_list: Vec::new_in(alloc),
            last_polled: 0,
            on_push_cb: None,
            effect_type: PhantomData,
            dependencies,
        }
    }
    fn push<'a, S>(&'a mut self, effect: E, alloc: A)
    where
        A: 'static,
        E: Effect<&'a mut D, Output = S> + Any,
        S: futures::Stream<Item = T> + Any + 'static,
    {
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        let task_type_name = core::any::type_name_of_val(&effect);
        let task_type_id = effect.type_id();
        let stream = effect.resolve(&mut self.dependencies);
        let stream: Box<NoDepsEffect<Enumerated<T>>, A> =
            allocator_api2::unsize_box!(allocator_api2::boxed::Box::new_in(
                stream.enumerate().map(|(idx, item)| Enumerated(idx, item)),
                alloc,
            ));
        self.task_list.push(ExecutorItem {
            stream: Pin::from(stream),
            task_id,
            task_type_name,
            task_type_id,
        })
    }
    fn get_next(&mut self) -> GetNext<T, A> {
        GetNext {
            items: &mut self.task_list,
            last_polled: &mut self.last_polled,
        }
    }
}

impl<E: effect_light::Effect<T>, T, D, A: Allocator, F1> Executor<E, T, D, A, F1> {
    fn with_on_push_cb<F>(self, cb: F) -> Executor<E, T, D, A, F>
    where
        F: FnMut(E),
    {
        let Self {
            next_task_id,
            last_polled,
            task_list,
            on_push_cb: _,
            effect_type,
            dependencies,
        } = self;
        Executor {
            next_task_id,
            last_polled,
            task_list,
            on_push_cb: Some(cb),
            effect_type,
            dependencies,
        }
    }
}
struct GetNext<'a, T, A: Allocator> {
    items: &'a mut Vec<ExecutorItem<T, A>, A>,
    last_polled: &'a mut usize,
}

impl<'a, T, A: Allocator> futures::Future for GetNext<'a, T, A> {
    type Output = Option<EffectOutput<T>>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let len = self.items.len();
        for idx in 0..len {
            // TODO: Tests for poll order
            *self.last_polled += 1;
            let adj_idx = (idx + *self.last_polled) % len;
            let mut stream = self
                .items
                .get_mut(adj_idx)
                .expect("adj_idx should be within bounds since it's modulod with len")
                .stream
                .as_mut();
            match futures::stream::Stream::poll_next(stream.as_mut(), cx) {
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
