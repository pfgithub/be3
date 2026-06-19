use super::*;

#[test]
fn locked_challenge_completion_hotbar_slot_has_unlock_tooltip() {
    assert_eq!(
        challenge_completion_locked_tooltip(ChallengeId::And),
        "Complete the challenge AND to unlock"
    );
}
