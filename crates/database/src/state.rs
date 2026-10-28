use strum::{Display, EnumIter, EnumString};

use self::GalleryState::*;

/// Gallery lifecycle (ADR-001). Variant order is the forward path and matches the
/// `gallery_state` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, sqlx::Type)]
#[strum(serialize_all = "snake_case")]
#[sqlx(type_name = "gallery_state", rename_all = "snake_case")]
pub enum GalleryState {
    Draft,
    Proofing,
    SelectionReceived,
    Retouching,
    Delivery,
    Expired,
    Archived,
}

impl GalleryState {
    /// Permitted moves: one step forward or back along the main path, any live state to
    /// `Expired`, any state to `Archived`, and `Archived` back to `Draft` only.
    pub fn can_transition_to(self, to: Self) -> bool {
        match (self, to) {
            (from, to) if from == to => false,
            (Archived, to) => to == Draft,
            (_, Archived) => true,
            (Expired, _) => false,
            (_, Expired) => true,
            (from, to) => from.path_position().abs_diff(to.path_position()) == 1,
        }
    }

    // Only meaningful for the five states on the main path.
    fn path_position(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    const PERMITTED: [(GalleryState, GalleryState); 20] = [
        (Draft, Proofing),
        (Proofing, SelectionReceived),
        (SelectionReceived, Retouching),
        (Retouching, Delivery),
        (Proofing, Draft),
        (SelectionReceived, Proofing),
        (Retouching, SelectionReceived),
        (Delivery, Retouching),
        (Draft, Expired),
        (Proofing, Expired),
        (SelectionReceived, Expired),
        (Retouching, Expired),
        (Delivery, Expired),
        (Draft, Archived),
        (Proofing, Archived),
        (SelectionReceived, Archived),
        (Retouching, Archived),
        (Delivery, Archived),
        (Expired, Archived),
        (Archived, Draft),
    ];

    #[test]
    fn permits_exactly_the_documented_transitions() {
        for from in GalleryState::iter() {
            for to in GalleryState::iter() {
                assert_eq!(
                    from.can_transition_to(to),
                    PERMITTED.contains(&(from, to)),
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn round_trips_snake_case_names() {
        assert_eq!(SelectionReceived.to_string(), "selection_received");
        assert_eq!("selection_received".parse(), Ok(SelectionReceived));
    }
}
