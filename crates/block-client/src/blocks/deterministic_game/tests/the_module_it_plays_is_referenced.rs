use block::Block;
use uuid::Uuid;

use super::DeterministicGame;

#[test]
fn the_module_it_plays_is_referenced() {
    let module = Uuid::new_v4();

    let game = DeterministicGame::new(module);

    assert_eq!(game.module(), module);
    assert_eq!(game.references(), [module]);
    assert_eq!(game.implicit_name(), None);
}
