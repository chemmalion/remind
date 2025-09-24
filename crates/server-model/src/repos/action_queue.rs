pub struct ActionQueue {
    pub path: json::Address,
    pub backend: Box<dyn JsonBackend>,
    pub items: Vec<Item>,
    pub errs: Vec<anyhow::Error>,
}

pub enum ActionQueueJob {
    TahtiLasku(TahtiLasku),
}

pub enum Item {
    TahtiLasku(TahtiLasku),
}

impl ActionQueue {
    pub fn new(path: json::Address, b: Box<dyn JsonBackend>) -> AResult<Self> {
        Ok(ActionQueue { path, backend: b }) // load from dir
    }

    pub fn set_item(&mut self, item: Item) {
        todo!()
    }
}

impl Worker for ActionQueue {
    fn do_job(&mut self, job: &Job) -> Vec<Job> {
        use ActionQueueJob::*;
        Ok(match job {
            Job::ForActionQueue(job) => match job {
                TahtiLasku(l) => {
                    self.set_item(Item::TahtiLasku(l));
                }
            },
            _ => vec![],
        })
    }
}

impl ErrAccumulator for ActionQueue {
    fn pop_errs(&mut self) -> Vec<anyhow::Error> {
        std::mem::take(&mut self.errs)
    }
}
