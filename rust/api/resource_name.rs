use std::fmt;

/// Macro to make easy to create a new resource name.
#[macro_export]
macro_rules! resource_name {
    ($name: literal) => {
        $crate::ResourceName::__new($name.to_string())
    };
    ($fmt: literal, $($tt: tt),+) => {
        $crate::ResourceName::__new(format!($fmt, $($tt),+))
    }
}

// We've 2 words of stack storage, as smallvec needs to store the heap pointer
// and length.
type Splits = smallvec::SmallVec<[u8; 2 * std::mem::size_of::<*const ()>()]>;

/// A resource name.
///
/// This is a _full path_ identifier of a given resource instance.
///
/// An example would be `"users/john/repos/linux"` for a repository name.
///
/// A resource name can be either the name of an entity, as the example
/// above, or of a collection, e.g. `"users/john/repos"`.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceName {
    inner: String,
    splits: Splits,
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl AsRef<str> for ResourceName {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl prost::Message for ResourceName {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: prost::bytes::BufMut,
        Self: Sized,
    {
        self.inner.encode_raw(buf)
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
        Self: Sized,
    {
        self.inner.merge_field(tag, wire_type, buf, ctx)?;
        self.recompute_splits();

        Ok(())
    }

    fn encoded_len(&self) -> usize {
        self.inner.encoded_len()
    }

    fn clear(&mut self) {
        self.inner.clear();
        self.splits.clear();
    }
}

impl ResourceName {
    /// Get a given segment value from this resource name.
    ///
    /// # Panics
    ///
    /// If the segment is not present in the name.
    pub fn get(&self, segment: &'static str) -> &str {
        for (idx, seg) in self.segments().enumerate() {
            if seg == segment {
                // The value of the segment is the part right after it.
                if let Some(val) = self.parts().nth(2 * idx + 1) {
                    return val;
                }
            }
        }

        panic!("Could not find segment '{segment}' in '{self}'")
    }

    /// Get the parent of this resource name.
    ///
    /// The name returned by this method will always be an entity.
    /// If `self` is a collection, we return the entity owner of
    /// the collection, otherwise, we return the enity owner of
    /// the collection where `self` is stored.
    pub fn parent(&self) -> Option<Self> {
        let last = if self.is_collection() {
            *self.splits.split_last()?.0
        } else {
            let parent_end_idx = (self.splits.len() as u8).saturating_sub(2);
            self.splits[parent_end_idx as usize]
        } as usize;

        if last == 0 {
            None
        } else {
            Some(Self::__new(self.inner[..last].to_string()))
        }
    }

    /// Tests if the resource name matches a specific segments
    /// pattern.
    pub fn matches(&self, segments: &[&'static str]) -> bool {
        dbg!(self.segments().collect::<Vec<_>>());
        self.segments().eq(segments.iter().copied())
    }

    /// Create a new resource name of a child entity.
    ///
    /// Only can be called with entity names. Use [`Self::item`] to
    /// create a name from a collection.
    ///
    /// # Panics
    ///
    /// Panics if called with a collection resource name.
    pub fn child(&self, segment: &'static str, id: impl fmt::Display) -> Self {
        use std::fmt::Write;

        assert!(self.is_entity(), "Cannot create a child from a collection");

        let mut inner = self.inner.to_string();
        let _ = write!(inner, "/{segment}/{id}");

        Self::__new(inner)
    }

    /// Create a new resource name for an item of this collection.
    ///
    /// Only can be called with collection names. Use [`Self::child`] to
    /// create a name from a collection.
    ///
    /// # Panics
    ///
    /// Panics if called with an entity resource name.
    pub fn item(&self, id: impl fmt::Display) -> Self {
        use std::fmt::Write;

        assert!(self.is_collection(), "Cannot create an item from an entity");

        let mut inner = self.inner.to_string();
        let _ = write!(inner, "/{id}");

        Self::__new(inner)
    }

    /// Checks that this entity name if of a given type.
    ///
    /// # Panics
    ///
    /// Panics if called with a collection resource name.
    pub fn is(&self, segment: &'static str) -> bool {
        self.type_() == segment
    }

    /// Get the type of the entity represented by this resource name.
    ///
    /// # Panics
    ///
    /// The method panics if the resource name is not of an entity.
    pub fn type_(&self) -> &str {
        // This assert ensures that there are at least two elements
        // in self.splits
        assert!(self.is_entity(), "cannot get type of collection");

        let end_idx = self.splits.len() - 1;
        let end = self.splits[end_idx] as usize;
        let mut start = self.splits[end_idx - 1] as usize;
        if start > 0 {
            start += 1;
        }

        &self.inner[start..end]
    }

    /// Get the last part of this resource name.
    ///
    /// If this is an entity name, the identifier of the entity will be
    /// returned. If this is a collection, the "name" of the collection
    /// will be returned.
    pub fn id(&self) -> &str {
        let &last = self
            .splits
            .last()
            .expect("all names have at least one split");

        if last == 0 {
            &self.inner
        } else {
            &self.inner[(last + 1) as usize..]
        }
    }

    /// Is this name the name of a collection?
    pub fn is_collection(&self) -> bool {
        // As we add the virtual split at the start, each part
        // starts with a split, meaning that collections, which
        // have odd parts, have odd splits.
        self.splits.len() % 2 == 1
    }

    /// Is this name the name of an entity?
    pub fn is_entity(&self) -> bool {
        !self.is_collection()
    }

    /// Returns the next name in a bytewise fashion.
    pub fn next_bytewise(&self) -> Self {
        let mut inner = self.inner.clone();
        let l = inner.pop().expect("name is never empty");

        let next = {
            let l = l as u32;
            let mut n = l.saturating_add(1);
            if l < 0xD800 && 0xD800 <= n {
                n += 0xD800;
            }

            char::from_u32(n).expect("invalid next char")
        };

        inner.push(next);

        Self {
            inner,
            splits: self.splits.clone(),
        }
    }

    fn recompute_splits(&mut self) {
        let splits_idxs = self
            .inner
            .match_indices('/')
            .map(|(idx, _)| u8::try_from(idx).unwrap());

        self.splits.clear();
        // Add a virtual split at the start to simplify rest of the code.
        self.splits.push(0);
        self.splits.extend(splits_idxs);
    }

    fn parts(&self) -> impl Iterator<Item = &str> {
        let mut last = *self.splits.last().unwrap();
        if last > 0 {
            last += 1;
        }

        self.splits
            .windows(2)
            .map(|w| self.translate_window(w))
            .chain(std::iter::once(&self.inner[last as usize..]))
    }

    fn segments(&self) -> impl Iterator<Item = &str> {
        self.parts().step_by(2)
    }

    fn translate_window(&self, window: &[u8]) -> &str {
        match *window {
            [0, e] => &self.inner[..e as usize],
            [s, e] => &self.inner[(s + 1) as usize..e as usize],
            // Only two elements are returned per iteration.
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn __new(inner: String) -> Self {
        let mut name = Self {
            inner,
            splits: Splits::default(),
        };
        name.recompute_splits();

        name
    }
}

#[cfg(test)]
mod tests {
    mod entities {
        #[test]
        fn test_resource_name() {
            let data = resource_name!("users/john/repos/linux");

            assert!(
                data.is("repos"),
                "Detected invalid type, expected repos, found {}",
                data.type_()
            );
            assert_eq!(data.get("users"), "john", "Invalid users segment value.");

            assert_eq!(data.get("repos"), "linux", "Invalid repos segment value.");
            assert_eq!(
                data.parent(),
                Some(resource_name!("users/john")),
                "Failed to compute correct parent."
            );
            assert_eq!(data.id(), "linux", "Failed to get id");

            assert!(
                data.matches(&["users", "repos"]),
                "Invalid match result, segments: {:?}",
                data.segments().collect::<Vec<_>>()
            );
        }

        #[test]
        fn test_no_parent() {
            let data = resource_name!("users/john");

            assert!(
                data.is("users"),
                "Failed to get correct type, actual: {}",
                data.type_()
            );

            assert_eq!(data.get("users"), "john", "Invalid users value");
            assert_eq!(data.parent(), None, "Found a parent where we shouldn't",);

            assert!(
                data.matches(&["users"]),
                "Invalid match result, segments: {:?}",
                data.segments().collect::<Vec<_>>()
            );
        }

        #[test]
        #[should_panic(expected = "Could not find segment 'repos' in 'users/john'")]
        fn test_missing_segment() {
            resource_name!("users/john").get("repos");
        }

        #[test]
        #[should_panic(expected = "Could not find segment 'repos' in 'users/john_repos'")]
        fn test_missing_segment_inside_value() {
            resource_name!("users/john_repos").get("repos");
        }
    }

    mod collections {
        #[test]
        fn test_root_collection() {
            let col = resource_name!("users");

            assert!(col.is_collection());
            assert_eq!(col.id(), "users");
            assert_eq!(col.parent(), None);
            assert_eq!(col.item("john"), resource_name!("users/john"));
            assert!(col.matches(&["users"]));
            assert!(!col.matches(&["users", "repos"]));
        }

        #[test]
        fn test_subcollection() {
            let col = resource_name!("users/john/repos");

            assert!(col.is_collection());
            assert_eq!(col.id(), "repos");
            assert_eq!(col.parent(), Some(resource_name!("users/john")));
            assert_eq!(col.get("users"), "john");
            assert_eq!(col.item("linux"), resource_name!("users/john/repos/linux"));
            assert!(col.matches(&["users", "repos"]));
            assert!(!col.matches(&["users"]));
        }

        #[test]
        #[should_panic]
        fn test_cannot_get_type_of_collection() {
            resource_name!("users").type_();
        }
    }

    mod protobuf {
        use prost::Message;

        use crate::ResourceName;

        #[test]
        fn test_string_transparent() {
            let message = "users/john/repos".to_string().encode_to_vec();

            let name = ResourceName::decode(&message[..]).unwrap();

            let str = String::decode(&name.encode_to_vec()[..]).unwrap();

            assert_eq!(str, "users/john/repos");
        }
    }
}
