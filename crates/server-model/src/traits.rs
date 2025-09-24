pub trait Timer {
    fn tick(&mut self, now: &DateTime) -> Vec<Job>;
}

pub trait Worker {
    fn do_job(&mut self, job: &Job) -> Vec<Job>;
}

pub trait ErrAccumulator {
    fn pop_errs(&mut self) -> Vec<anyhow::Error>;
}
