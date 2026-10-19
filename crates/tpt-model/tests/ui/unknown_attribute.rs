use tpt_model::Model;

#[derive(Model)]
#[tpt(version = 1)]
struct Thing {
    #[tpt(sinse = 1)]
    field: u32,
}

fn main() {}
