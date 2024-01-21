use songbird::input::{Input, RawAdapter};
use std::{
    io::{Read, Result as IoResult},
    mem,
    process::Child,
};
use symphonia::core::io::ReadOnlySource;
use tokio::runtime::Handle;

/// Handle for a child process which ensures that any subprocesses are properly closed
/// on drop.
///
/// # Warning
/// To allow proper cleanup of child processes, if you create a process chain you must
/// make sure to use `From<Vec<Child>>`. Here, the *last* process in the `Vec` will be
/// used as the audio byte source.
#[derive(Debug)]
pub struct ChildContainer(pub Vec<Child>);

impl Read for ChildContainer {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        match self.0.last_mut() {
            Some(ref mut child) => child.stdout.as_mut().unwrap().read(buffer),
            None => Ok(0),
        }
    }
}

impl ChildContainer {
    /// Create a new [`ChildContainer`] from a child process
    #[must_use]
    pub fn new(children: Vec<Child>) -> Self {
        Self(children)
    }
}

impl From<Child> for ChildContainer {
    fn from(container: Child) -> Self {
        Self(vec![container])
    }
}

impl From<Vec<Child>> for ChildContainer {
    fn from(container: Vec<Child>) -> Self {
        Self(container)
    }
}

impl From<ChildContainer> for Input {
    fn from(val: ChildContainer) -> Self {
        let source = ReadOnlySource::new(val);
        let audio_stream = RawAdapter::new(
            source, 48000, 2
        );
        Input::from(audio_stream)
    }
}

impl Drop for ChildContainer {
    fn drop(&mut self) {
        let children = mem::take(&mut self.0);

        if let Ok(handle) = Handle::try_current() {
            handle.spawn_blocking(move || {
                cleanup_child_processes(children);
            });
        } else {
            cleanup_child_processes(children);
        }
    }
}

fn cleanup_child_processes(mut children: Vec<Child>) {
    let attempt = if let Some(child) = children.last_mut() {
        child.kill()
    } else {
        return;
    };

    let _ = attempt.and_then(|()| {
        children
            .iter_mut()
            .rev()
            .try_for_each(|child| child.wait().map(|_| ()))
    });
}
