use tpt_model::Model;

#[derive(Model)]
#[tpt(version = 1)]
struct Thing {
    #[tpt(since = 2)]
    added_later: u32,
}

fn main() {}
