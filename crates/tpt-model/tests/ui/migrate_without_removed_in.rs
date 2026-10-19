use tpt_model::Model;

fn migrate(
    _old: &tpt_model::UirNode,
    _ctx: &tpt_model::MigrationContext,
) -> Result<tpt_model::UirNode, tpt_model::MigrationError> {
    unimplemented!()
}

#[derive(Model)]
struct Thing {
    #[tpt(migrate = "migrate")]
    value: u32,
}

fn main() {}
