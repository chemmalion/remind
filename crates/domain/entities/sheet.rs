pub struct Sheet {
    pub cols: Vec<Column>,
}

pub struct Column {
    pub cells: Vec<Cell>,
}

pub struct Cell {
    pub text: String,
}

pub enum Path {
    TahtiLasku,
}
