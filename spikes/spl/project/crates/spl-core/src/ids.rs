use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }
            pub fn from_str(s: impl Into<String>) -> Self {
                Self(s.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_newtype!(TaskId);
id_newtype!(RevisionId);
id_newtype!(SpecRevId);
id_newtype!(QueueId);
id_newtype!(LeaseId);
id_newtype!(RunId);
id_newtype!(ArtifactId);
id_newtype!(AnchorId);
