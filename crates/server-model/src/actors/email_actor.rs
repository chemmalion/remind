pub struct EmailActor {
    pub creds: email::Creds,
    pub backend: Box<dyn EmailBackend>,
    pub last_check: Option<DateTime>,
    pub check_schedule: When,
}

impl EmailActor {
    pub fn new(creds: email::Creds, b: Box<dyn EmailBackend>, schedule: When) -> Self {
        EmailActor {
            creds,
            backend: b,
            last_check: None,
            check_schedule: schedule,
        }
    }
}

impl Timer for EmailActor {
    fn tick(&mut self, now: &DateTime) -> Vec<Job> {
        todo!()
    }
}
