pub struct Backends {
    pub email: Box<dyn EmailBackend>,
    pub json: Box<dyn JsonBackend>,
}
