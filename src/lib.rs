use effect_light::{Effect, EffectAsync, EffectExt};
use futures::Stream;

type NoDepsEffect<T> = dyn Stream<Item = T>;

struct Executor<T> {
    next_task_id: u64,
    task_list: Vec<Box<NoDepsEffect<T>>>,
}

impl<T> Executor<T> {
    pub fn push_effect<D>(&mut self, dependency: D, e: impl EffectAsync<D, OutputAsync = T>) {
        let s = Box::new(
            e.map_output(|fut| futures::stream::once(fut))
                .resolve(dependency),
        ) as Box<NoDepsEffect<T>>;
    }
    pub async fn get_next(&mut self) -> Option<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {}
