use super::*;

fn client() -> BlockClient {
    BlockClient::new(Uuid::new_v4(), Uuid::new_v4())
}

mod an_unknown_block_type_has_no_handle;
mod create_default_makes_the_types_that_declare_one;
mod every_declared_type_opens;
