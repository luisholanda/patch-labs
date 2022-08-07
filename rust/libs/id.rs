use std::{fmt, str::FromStr, hash::{Hash, Hasher}};

use svix_ksuid::{KsuidMs, KsuidLike};

// TODO(proto-gen): this should be const generic by a resource URI.
/// A generic identifier for Patch Labs code.
///
/// This is based on the work made by Segment.io on their [KSUID]
/// implementation.
///
/// [KSUID]: https://github.com/segmentio/ksuid
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id(KsuidMs);

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.bytes().hash(state)
    }
}

impl Default for Id {
    fn default() -> Self {
        Self(KsuidMs::from_bytes([0; 20]))
    }
}

impl Id {
    /// Create a new identifier value.
    pub fn new() -> Self {
        Self(KsuidMs::new(None, None))
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_base62())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_base62())
    }
}

impl prost::Message for Id {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: prost::bytes::BufMut,
        Self: Sized
    {
        self.0.to_string().encode_raw(buf)
    }

    fn merge_field<B>(
        &mut self,
        tag: u32,
        wire_type: prost::encoding::WireType,
        buf: &mut B,
        ctx: prost::encoding::DecodeContext,
    ) -> Result<(), prost::DecodeError>
    where
        B: prost::bytes::Buf,
        Self: Sized
    {
        let mut string = String::with_capacity(self.encoded_len());
        string.merge_field(tag, wire_type, buf, ctx)?;

        self.0 = KsuidMs::from_str(&string)
            .map_err(|err| prost::DecodeError::new(err.to_string()))?;

        Ok(())
    }

    fn encoded_len(&self) -> usize {
        27
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}
