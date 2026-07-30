use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub const fn new(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self::new(value)
            }
        }
    };
}

uuid_id!(UserId);
uuid_id!(PageId);
uuid_id!(GroupId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip_without_becoming_interchangeable() {
        let value = Uuid::new_v4();
        let user_id = UserId::new(value);
        let page_id = PageId::new(value);

        assert_eq!(user_id.into_uuid(), value);
        assert_eq!(page_id.into_uuid(), value);
    }
}
