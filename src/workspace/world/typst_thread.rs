use std::sync::mpsc;
use std::thread;

use tokio::runtime;
use tokio::sync::oneshot;
use tower_lsp::lsp_types::Url;
use tracing::{trace, warn};

use crate::workspace::project::Project;

use super::ProjectWorld;

pub type Task = Box<dyn FnOnce(runtime::Handle) + Send + 'static>;

pub struct TypstThread {
    sender: parking_lot::Mutex<mpsc::Sender<Request>>,
}

impl Default for TypstThread {
    fn default() -> Self {
        let handle = runtime::Handle::current();
        let (sender, receiver) = mpsc::channel::<Request>();

        thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
                trace!("got new request on Typst thread");
                request.run(handle.clone());
                trace!("completed request on Typst thread");
            }
        });

        Self {
            sender: parking_lot::Mutex::new(sender),
        }
    }
}

impl TypstThread {
    #[tracing::instrument(skip(self, world_main, f), fields(%world_main))]
    pub async fn run_with_world<Ret: Send + 'static>(
        &self,
        world_project: Project,
        world_main: Url,
        f: impl FnOnce(ProjectWorld) -> Ret + Send + 'static,
    ) -> Ret {
        let f_prime = move |handle| {
            let world = ProjectWorld::new(world_project, world_main, handle);
            f(world)
        };

        self.run(f_prime).await
    }

    #[tracing::instrument(skip_all)]
    pub async fn run<Ret: Send + 'static>(
        &self,
        f: impl FnOnce(runtime::Handle) -> Ret + Send + 'static,
    ) -> Ret {
        let (sender, receiver) = oneshot::channel();
        let f_prime = move |handle| {
            let t = f(handle);
            if sender.send(t).is_err() {
                // Receiver was dropped. The main thread may have exited, or the request may have
                // been cancelled.
                warn!("could not send back return value from Typst thread");
            }
        };

        self.send_request(Request::new(f_prime));

        receiver.await.unwrap()
    }

    #[tracing::instrument(skip_all)]
    fn send_request(&self, request: Request) {
        let sender = self.sender.lock();
        sender.send(request).unwrap();
    }
}

struct Request {
    task: Task,
}

impl Request {
    pub fn new(f: impl FnOnce(runtime::Handle) + Send + 'static) -> Self {
        Self { task: Box::new(f) }
    }

    pub fn run(self, handle: runtime::Handle) {
        (self.task)(handle);
    }
}
