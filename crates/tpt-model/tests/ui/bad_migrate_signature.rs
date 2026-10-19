use tpt_model::Model;

fn migrate(_old: u32) -> u32 {
    0
}

#[derive(Model)]
#[tpt(version = 2)]
struct Thing {
    #[tpt(removed_in = 3, migrate = "migrate")]
    old: u32,
}

fn main() {}
