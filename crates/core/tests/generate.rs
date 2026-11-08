use std::sync::atomic::{AtomicU64, Ordering};

use pp_core::types::generate::Generate;

#[derive(Debug, PartialEq)]
struct TestId(u64);

impl Generate for TestId {
    fn generate() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);

        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

fn generate_pair<T: Generate>() -> (T, T) {
    (T::generate(), T::generate())
}

#[test]
fn ids_can_be_generated_without_their_representation() {
    let (first, second) = generate_pair::<TestId>();

    assert_ne!(first, second);
}
