use tpt_model::Model;

#[derive(Model)]
struct Thing {
    #[tpt(intent = "admin", capability = "x")]
    value: u32,
}

fn main() {}
