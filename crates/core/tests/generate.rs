use pp_core::{notifications::PhotoId, types::generate::Generate};

fn generate_pair<T: Generate>() -> (T, T) {
    (T::generate(), T::generate())
}

#[test]
fn photo_ids_can_be_generated_without_their_representation() {
    let (first, second) = generate_pair::<PhotoId>();

    assert_ne!(first, second);
}
