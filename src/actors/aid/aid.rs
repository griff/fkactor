use std::cmp;
use std::fmt;
use std::hash;
use std::sync::Arc;

use uuid::Uuid;

#[derive(Debug)]
struct InnerAid {
    uuid: Uuid,
    system_uuid: Uuid,
    name: Option<String>,
}

#[derive(Clone)]
pub struct Aid {
    data: Arc<InnerAid>
}

impl Aid {
    pub(crate) fn new(name: Option<String>, system_uuid: Uuid) -> Aid {
        Aid {
            data: Arc::new(InnerAid {
                uuid: Uuid::new_v4(),
                name, system_uuid,
            })
        }
    }
}

impl fmt::Debug for Aid {
    fn fmt(&self, formatter: &'_ mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "Aid{{id: {}, system_uuid: {}, name: {:?}}}",
            self.data.uuid.to_string(),
            self.data.system_uuid.to_string(),
            self.data.name
        )
    }
}

impl fmt::Display for Aid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data.name {
            Some(name) => write!(f, "{}:{}", name, self.data.uuid),
            None => write!(f, "{}", self.data.uuid),
        }
    }
}

impl hash::Hash for Aid {
    fn hash<H: hash::Hasher>(&self, state: &'_ mut H) {
        self.data.uuid.hash(state);
        self.data.system_uuid.hash(state);
    }
}

impl cmp::PartialEq for Aid {
    fn eq(&self, other: &Self) -> bool {
        self.data.uuid == other.data.uuid && self.data.system_uuid == other.data.system_uuid
    }
}

impl cmp::Eq for Aid {}

impl cmp::PartialOrd for Aid {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        use std::cmp::Ordering;
        // Order by name, then by system, then by uuid.  Also, sort `None` names before others.
        match (&self.data.name, &other.data.name) {
            (None, Some(_)) => Some(Ordering::Less),
            (Some(_), None) => Some(Ordering::Greater),
            (Some(a), Some(b)) if a != b => Some(a.cmp(b)),
            (_, _) => {
                // Names are equal, either both `None` or `Some(thing)` where `thing1 == thing2`
                // so we impose a secondary order by system uuid.
                match self.data.system_uuid.cmp(&other.data.system_uuid) {
                    Ordering::Equal => Some(self.data.uuid.cmp(&other.data.uuid)),
                    x => Some(x),
                }
            }
        }
    }
}

impl cmp::Ord for Aid {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.partial_cmp(other)
            .expect("Aid::partial_cmp() returned None; can't happen")
    }
}
