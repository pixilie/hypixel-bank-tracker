use std::{
	sync::{mpsc, Arc, Mutex},
	thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
	id: usize,
	thread: thread::JoinHandle<()>,
}

impl Worker {
	fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
		let thread = thread::spawn(move || loop {
			let message = receiver.lock().unwrap().recv();

			if let Ok(job) = message {
				job();
			} else {
				break;
			}
		});

		Self { id, thread }
	}
}

pub struct ThreadPool {
	workers: Vec<Worker>,
	sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
	pub fn new(size: usize) -> Self {
		assert!(size > 0);

		let (sender, receiver) = mpsc::channel();
		let receiver = Arc::new(Mutex::new(receiver));
		let mut workers = Vec::with_capacity(size);

		for id in 0..size {
			workers.push(Worker::new(id, Arc::clone(&receiver)));
		}

		Self {
			workers,
			sender: Some(sender),
		}
	}

	pub fn execute<F>(&self, f: F)
	where
		F: FnOnce() + Send + 'static,
	{
		let job = Box::new(f);
		self.sender.as_ref().unwrap().send(job).unwrap();
	}
}

impl Drop for ThreadPool {
	fn drop(&mut self) {
		drop(self.sender.take());

		for worker in self.workers.drain(..) {
			worker.thread.join().unwrap();
		}
	}
}
