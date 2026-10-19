use tpt_model::Model;

#[derive(Model)]
struct Thing<T> {
    value: T,
}

fn main() {}
