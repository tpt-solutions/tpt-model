use tpt_model::Model;

#[derive(Model)]
#[tpt(version = 2)]
struct Thing {
    #[tpt(removed_in = 3, migrate = "does_not_exist")]
    old: u32,
}

fn main() {}
