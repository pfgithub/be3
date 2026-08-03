use super::*;

mod active_account_round_trips;
mod fresh_store_has_no_accounts;
mod removing_an_account_clears_active_selection;
mod saved_accounts_and_last_workspace_survive_reopen;

fn account(server: ServerLocation, email: &str) -> SavedAccount {
    SavedAccount {
        server,
        id: Uuid::new_v4(),
        email: email.into(),
        name: email.into(),
        last_workspace_id: None,
    }
}
