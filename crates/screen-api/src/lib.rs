//! Lifecycle for trusted, compiled screen modules. Slow work belongs on worker threads.

pub trait Screen {
    fn activate(&mut self);
    fn tick(&mut self);
    fn deactivate(&mut self);
}
