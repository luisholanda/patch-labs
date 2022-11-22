use std::{io, marker::PhantomData};

use foundationdb::{
    tuple::{Subspace, TuplePack},
    RangeOption,
};
use pl_database::{DbError, DbResult, Tx};
use prost::Message;

/// V1 metadata: protobuf encoded, no extra transformations done.
const V1_METADATA: u8 = 0;

pub struct Collection<E> {
    subspace: Subspace,
    _marker: PhantomData<E>,
    // TODO(mempool): use mempool to reduce allocation cost.
    //
    // Most of the operations in this type need to allocate
    // at least the buffer to the key (sometimes multiple).
    // The cost associated can be reduced by reusing memory
    // buffers.
}

impl<E> Collection<E> {
    /// Create a new static collection.
    ///
    /// Static collections are those that have a static name, and thus
    /// we don't need to translate them into a short prefix value.
    ///
    /// When creating a static collection, try to keep its name small,
    /// as improves the database performance.
    pub fn from_static(name: &'static str) -> Self {
        Self {
            subspace: Subspace::from_bytes(name.as_bytes()),
            _marker: PhantomData,
        }
    }
}

impl<E> Collection<E>
where
    E: Message + Default,
{
    /// Get an entity from the collection, returning `None` if the key is not present.
    ///
    /// # Errors
    ///
    /// This method returns an error if the database operation fails or if we can't
    /// decode the database value into an instance of `E`. Note that, as we use protocol
    /// buffers for the encoding, the later can be prevented by not making wire incompatible
    /// changes to the entity.
    pub async fn get(&self, tx: &Tx, key: &impl TuplePack) -> DbResult<Option<E>, io::Error> {
        let key = self.subspace.pack(key);
        let Some(bytes) = tx.get(&key).await? else {
            return Ok(None);
        };

        decode_to_entity(&bytes).map(Some)
    }

    /// Get a range of entities.
    ///
    /// # Errors
    ///
    /// Each value can return an error if the database returns an error at that point or
    /// if we fail to decode the returned bytes into an instance of `E`. The same observations
    /// in [`Self::get`] applies here.
    pub async fn range<'t>(
        &self,
        tx: &'t Tx,
        opts: RangeOption<'_>,
    ) -> DbResult<Vec<E>, io::Error> {
        let mut range_elems = opts.limit.map_or_else(Vec::new, Vec::with_capacity);

        tx.for_each_in_range(opts, |_, value| {
            let res = decode_to_entity::<E>(value);

            match res {
                Err(err) => std::future::ready(Err(err)),
                Ok(elem) => {
                    range_elems.push(elem);
                    std::future::ready(Ok(true))
                }
            }
        })
        .await?;

        todo!()
    }

    /// Set the value of a specific key.
    pub fn set(&self, tx: &mut Tx, key: &impl TuplePack, value: &E) {
        // Add extra byte for encoded value metadata.
        let mut bytes = Vec::with_capacity(value.encoded_len() + 1);
        bytes.push(V1_METADATA);
        value.encode_raw(&mut bytes);

        let key = self.subspace.pack(key);

        tx.set(&key, &bytes)
    }

    /// Clear a specific value from the collection.
    pub fn clear(&self, tx: &mut Tx, key: &impl TuplePack) {
        let key = self.subspace.pack(key);

        tx.clear(&key)
    }
}

fn decode_to_entity<E>(bytes: &[u8]) -> DbResult<E, io::Error>
where
    E: Message + Default,
{
    // The first byte from a collection-stored value is always present and
    // represents the encoding metadata.
    let metadata_byte = bytes[0];

    if metadata_byte == V1_METADATA {
        E::decode(&bytes[1..]).map_err(|err| DbError::Abort(err.into()))
    } else {
        Err(DbError::Abort(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid metadata byte from database!",
        )))
    }
}
