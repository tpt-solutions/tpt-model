use tpt_model::Model;

#[derive(Model)]
enum Thing {
    Pair(u32, u32),
}

fn main() {}
