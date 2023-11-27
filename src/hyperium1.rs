mod outgoing;
mod incoming;

pub use outgoing::Hyperium1OutgoingBodyCopier;

use crate::{wasi::FieldEntries, Error};

impl TryFrom<FieldEntries> for http1::HeaderMap {
    type Error = Error;

    fn try_from(entries: FieldEntries) -> Result<Self, Self::Error> {
        Ok(entries
            .into_iter()
            .map(|(name, val)| Ok((name.try_into()?, val.try_into()?)))
            .collect::<Result<Self, http1::Error>>()?)
    }
}

impl From<http1::HeaderMap> for FieldEntries {
    fn from(map: http1::HeaderMap) -> Self {
        map.iter()
            .map(|(name, val)| (name.to_string(), val.as_bytes().to_vec()))
            .collect::<Vec<_>>()
            .into()
    }
}
