use crate::db::DraftOrderKind;

/// Whose turn it is for `pick_index` (0-based number of picks already made).
///
/// `order` is the randomized first-round sequence (position 0 picks first in round 0).
pub fn next_picker(order: &[u64], pick_index: usize, kind: DraftOrderKind) -> Option<u64> {
    if order.is_empty() {
        return None;
    }
    let n = order.len();
    let round = pick_index / n;
    let pos_in_round = pick_index % n;
    let index = match kind {
        DraftOrderKind::Linear => pos_in_round,
        DraftOrderKind::Snake => {
            if round.is_multiple_of(2) {
                pos_in_round
            } else {
                n - 1 - pos_in_round
            }
        }
    };
    order.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::next_picker;
    use crate::db::DraftOrderKind;

    #[test]
    fn snake_round_zero_goes_forward() {
        let order = [10, 20, 30];
        assert_eq!(next_picker(&order, 0, DraftOrderKind::Snake), Some(10));
        assert_eq!(next_picker(&order, 1, DraftOrderKind::Snake), Some(20));
        assert_eq!(next_picker(&order, 2, DraftOrderKind::Snake), Some(30));
    }

    #[test]
    fn snake_round_one_reverses() {
        let order = [10, 20, 30];
        assert_eq!(next_picker(&order, 3, DraftOrderKind::Snake), Some(30));
        assert_eq!(next_picker(&order, 4, DraftOrderKind::Snake), Some(20));
        assert_eq!(next_picker(&order, 5, DraftOrderKind::Snake), Some(10));
    }

    #[test]
    fn snake_round_two_forward_again() {
        let order = [10, 20, 30];
        assert_eq!(next_picker(&order, 6, DraftOrderKind::Snake), Some(10));
    }

    #[test]
    fn linear_never_reverses() {
        let order = [10, 20, 30];
        assert_eq!(next_picker(&order, 3, DraftOrderKind::Linear), Some(10));
        assert_eq!(next_picker(&order, 4, DraftOrderKind::Linear), Some(20));
    }

    #[test]
    fn empty_order_returns_none() {
        assert_eq!(next_picker(&[], 0, DraftOrderKind::Snake), None);
    }
}
