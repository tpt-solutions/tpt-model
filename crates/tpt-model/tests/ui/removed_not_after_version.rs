use tpt_model::Model;

fn migrate(
    _old: &tpt_model::UirNode,
    _ctx: &tpt_model::MigrationContext,
) -> Result<tpt_model::UirNode, tpt_model::MigrationError> {
    unimplemented!()
}

#[derive(Model)]
#[tpt(version = 3)]
struct Thing {
    #[tpt(removed_in = 3, migrate = "migrate")]
    old: u32,
}

fn main() {}
