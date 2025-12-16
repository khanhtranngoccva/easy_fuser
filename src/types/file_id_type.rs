//! File identification types and traits for FUSE filesystems.
//!
//! This module defines the `FileIdType` trait and its implementations, which provide
//! flexible ways to identify files in a FUSE filesystem. It supports three main
//! identification methods: inode-based, path-based, and component-based. Each method
//! offers different trade-offs in terms of performance, ease of use, and memory usage.
//! The module also includes associated types for full and minimal metadata, which
//! are different possible return values in FUSE operations.

use std::{
    ffi::OsString,
    fmt::{Debug, Display},
    hash::Hash,
    path::{Path, PathBuf},
};

use super::arguments::FileAttribute;
use super::inode::*;
use crate::core::InodeResolvable;
use fuser::FileType as FileKind;

/// Represents the type used to identify files in the file system.
///
/// This trait allows different approaches to file identification:
///
/// 1. `Inode`: The user provides their own unique inode numbers.
///    - Pros: Direct control over inode assignment.
///    - Cons: Requires manual management of inode uniqueness.
///    - Root: Represented by the constant ROOT_INODE with a value of 1.
///
/// 2. `PathBuf`: Uses file paths for identification.
///    - Pros: Automatic inode-to-path mapping and caching.
///    - Cons: May have performance overhead for large file systems.
///    - Root: Represented by an empty string. Paths are relative and never begin with a forward slash.
///
/// 3. `Vec<OsString>`: Uses a vector of path components for identification.
///    - Pros: Slightly lower overhead than PathBuf, allows path to be divided into parts.
///    - Cons: Path components are stored in reverse order, which may require additional handling.
///    - Root: Represented by an empty vector.
///
/// 4. `HybridId`: Composes of an `Inode` and a `PathBuf`. Also uses file paths for identification, but also exposes the managed inode number.
///    - Pros: Automatic inode-to-path mapping and caching, while allowing an escape hatch when precise inode management is required
///         (for example when tracking inode reference counts to prevent dangerous operations while any handles to the inode are still open)
///    - Cons: May have performance overhead for large file systems.
///    - Root: Represented by the constant ROOT_INODE with a value of 1, and an empty string as a path.
pub trait FileIdType:
    'static + Debug + Clone + PartialEq + Eq + std::hash::Hash + InodeResolvable
{
    /// Full metadata type for the file system.
    ///
    /// For Inode-based: (Inode, FileAttribute)
    /// - User must provide both Inode and FileAttribute.
    ///
    /// For PathBuf-based: FileAttribute
    /// - User only needs to provide FileAttribute; Inode is managed internally.
    type Metadata;

    /// Minimal metadata type for the file system.
    ///
    /// For Inode-based: (Inode, FileKind)
    /// - User must provide both Inode and FileKind.
    ///
    /// For PathBuf-based: FileKind
    /// - User only needs to provide FileKind; Inode is managed internally.
    type MinimalMetadata;
    #[doc(hidden)]
    type _Id;

    /// Returns a displayable representation of the file identifier.
    ///
    /// This method provides a human-readable string representation of the file identifier,
    /// which can be useful for debugging, logging, or user-facing output.
    fn display(&self) -> impl Display;

    /// Checks if this file identifier represents the root of the filesystem.
    ///
    /// This method determines whether the current file identifier corresponds to the
    /// topmost directory in the filesystem hierarchy.
    fn is_filesystem_root(&self) -> bool;

    #[doc(hidden)]
    fn extract_metadata(metadata: Self::Metadata) -> (Self::_Id, FileAttribute);
    #[doc(hidden)]
    fn extract_minimal_metadata(minimal_metadata: Self::MinimalMetadata) -> (Self::_Id, FileKind);
}

impl FileIdType for Inode {
    type _Id = Inode;
    type Metadata = (Inode, FileAttribute);
    type MinimalMetadata = (Inode, FileKind);

    fn display(&self) -> impl Display {
        format!("{:?}", self)
    }

    fn is_filesystem_root(&self) -> bool {
        *self == ROOT_INODE
    }

    fn extract_metadata(metadata: Self::Metadata) -> (Self::_Id, FileAttribute) {
        metadata
    }

    fn extract_minimal_metadata(minimal_metadata: Self::MinimalMetadata) -> (Self::_Id, FileKind) {
        minimal_metadata
    }
}

impl FileIdType for PathBuf {
    type _Id = ();
    type Metadata = FileAttribute;
    type MinimalMetadata = FileKind;

    fn display(&self) -> impl Display {
        Path::display(self)
    }

    fn is_filesystem_root(&self) -> bool {
        self.as_os_str().is_empty()
    }

    fn extract_metadata(metadata: Self::Metadata) -> (Self::_Id, FileAttribute) {
        ((), metadata)
    }

    fn extract_minimal_metadata(minimal_metadata: Self::MinimalMetadata) -> (Self::_Id, FileKind) {
        ((), minimal_metadata)
    }
}

impl FileIdType for Vec<OsString> {
    type _Id = ();
    type Metadata = FileAttribute;
    type MinimalMetadata = FileKind;

    fn display(&self) -> impl Display {
        // Join all paths with a separator for display
        self.iter()
            .map(|os_str| os_str.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn is_filesystem_root(&self) -> bool {
        self.is_empty()
    }

    fn extract_metadata(metadata: Self::Metadata) -> (Self::_Id, FileAttribute) {
        ((), metadata)
    }

    fn extract_minimal_metadata(minimal_metadata: Self::MinimalMetadata) -> (Self::_Id, FileKind) {
        ((), minimal_metadata)
    }
}

#[derive(Debug, Clone, Eq)]
pub struct HybridId(pub Inode, pub PathBuf);

impl PartialEq for HybridId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for HybridId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl HybridId {
    pub fn inode(&self) -> &Inode {
        &self.0
    }

    pub fn path(&self) -> &Path {
        &self.1
    }
}

impl FileIdType for HybridId {
    type _Id = ();
    type Metadata = FileAttribute;
    type MinimalMetadata = FileKind;

    fn display(&self) -> impl Display {
        format!("HybridId({}, {})", self.0.as_raw(), self.1.display())
    }

    fn is_filesystem_root(&self) -> bool {
        let inode_eq = self.0 == ROOT_INODE;
        let path_eq = self.1.as_os_str().is_empty();
        if inode_eq ^ path_eq == false {
            panic!("an empty pathbuf must have ROOT_INODE, and vice versa");
        }
        inode_eq
    }

    fn extract_metadata(metadata: Self::Metadata) -> (Self::_Id, FileAttribute) {
        ((), metadata)
    }

    fn extract_minimal_metadata(minimal_metadata: Self::MinimalMetadata) -> (Self::_Id, FileKind) {
        ((), minimal_metadata)
    }
}
