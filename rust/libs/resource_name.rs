use std::fmt;

use uuid::Uuid;

#[cfg(debug_assertions)]
/// Macro to make easy to create a new resource name.
///
/// Usable only in tests.
macro_rules! resource_name {
    ($name: literal) => {
        $crate::ResourceName::__new($name.to_string())
    };
}


/// A resource name.
///
/// This is a _full path_ identifier of a given resource instance.
///
/// An example would be `"users/john/repo/linux"` for a repository name.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceName {
    inner: String
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl prost::Message for ResourceName {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: prost::bytes::BufMut,
        Self: Sized
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
        Self: Sized {
        self.inner.merge_field(tag, wire_type, buf, ctx)
    }

    fn encoded_len(&self) -> usize {
        self.inner.encoded_len()
    }

    fn clear(&mut self) {
        self.inner.clear()
    }
}

impl ResourceName {
    /// Get a given segment value from this resource name.
    ///
    /// # Panics
    ///
    /// If the segment is not present in the name.
    pub fn get(&self, segment: &'static str) -> &str {
        self.as_view().get(segment)
    }

    /// Gets the parent of this resource name.
    ///
    /// Returns `None` if the name has no parent.
    pub fn parent(&self) -> Option<ResourceNameView<'_>> {
        self.as_view().parent()
    }

    /// Creates a new child resource name with this name as parent.
    pub fn child(&self, segment: &'static str, id: impl fmt::Display) -> Self {
        self.as_view().child(segment, id)
    }

    /// Creates a new child with a generated id.
    ///
    /// The id can be ordered chronologically.
    pub fn generate_child(&self, segment: &'static str) -> Self {
        self.as_view().generate_child(segment)
    }

    /// Checks if the given resource name is of the given segment type.
    pub fn is(&self, segment: &'static str) -> bool {
        self.as_view().is(segment)
    }

    /// The id of this resource name.
    ///
    /// Id is the last segment value in the name.
    pub fn id(&self) -> &str {
        self.as_view().id()
    }

    fn as_view(&self) -> ResourceNameView<'_> {
        ResourceNameView { inner: &self.inner }
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn __new(inner: String) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceNameView<'n> {
    inner: &'n str
}

impl fmt::Display for ResourceNameView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl<'n> ResourceNameView<'n> {
    pub fn get(&self, segment: &'static str) -> &'n str {
        let mut curr_segment = "";

        for (i, split) in self.inner.split('/').enumerate() {
            if i % 2 == 0 {
                curr_segment = split;
                continue;
            }

            if curr_segment == segment {
                return split;
            }
        }

        panic!("Could not find segment '{segment}' in '{self}'")
    }

    pub fn parent(&self) -> Option<ResourceNameView<'n>> {
        self.inner
            .rsplitn(3, '/')
            .nth(2)
            .map(|inner| ResourceNameView { inner })
    }

    pub fn child(&self, segment: &'static str, id: impl fmt::Display) -> ResourceName {
        use std::fmt::Write;

        let mut inner = self.inner.to_string();
        let _ = write!(inner, "/{segment}/{id}");

        ResourceName { inner }
    }

    pub fn generate_child(&self, segment: &'static str) -> ResourceName {
        self.child(segment, &Uuid::now_v7())
    }

    pub fn is(&self, segment: &'static str) -> bool {
        let last_segment = self
            .inner
            .rsplitn(3, '/')
            .nth(1)
            .unwrap_or_else(|| panic!("Invalid resource name: {self}"));

        last_segment == segment
    }

    pub fn id(&self) -> &'n str {
        self.inner
            .rsplit_once('/')
            .unwrap_or_else(|| panic!("Invalid resource name: {self}"))
            .1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_name() {
        let data = resource_name!("users/john/repos/linux");

        assert!(data.is("repos"));
        assert_eq!(data.get("users"), "john");
        assert_eq!(data.get("repos"), "linux");
        assert_eq!(data.parent(), Some(resource_name!("users/john").as_view()));
        assert_eq!(data.id(), "linux");
    }

    #[test]
    fn test_no_parent() {
        let data = resource_name!("users/john");

        assert!(data.is("users"));
        assert_eq!(data.get("users"), "john");
        assert!(data.parent().is_none());
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
