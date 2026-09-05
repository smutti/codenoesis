use crate::ObjectId;

pub const LOCAL_GIT_SHA1_PACKED_V1: &str = "local-git-sha1-packed-v1";
pub const LOCAL_GIT_SHA1_PACKED_RUST_8M_V1: &str = "local-git-sha1-packed-rust-8m-v1";
pub const LOCAL_GIT_SHA1_PACKED_RUST_8M_SINGLE_FILE_BYTES: u64 = 8_388_608;

pub const STANDARD_LOCAL_PACKED_LIMITS: StandardLocalPackedLimits = StandardLocalPackedLimits {
    pack_directory_entries: 512,
    pack_pairs: 64,
    single_pack_index_bytes: 134_217_728,
    cumulative_pack_index_bytes: 268_435_456,
    indexed_objects: 8_000_000,
    single_pack_bytes: 4_294_967_296,
    cumulative_verified_pack_bytes: 8_589_934_592,
    compressed_entry_bytes: 67_108_864,
    inflated_entry_bytes: 268_435_456,
    cumulative_entry_inflate_bytes: 1_073_741_824,
    delta_program_bytes: 33_554_432,
    delta_depth: 50,
    delta_instructions: 4_194_304,
    delta_intermediate_bytes: 268_435_456,
    cumulative_delta_work_bytes: 1_073_741_824,
    object_locations: 8,
    reconstructed_object_cache_bytes: 134_217_728,
};

pub const PACKED_LIMIT_KINDS: [crate::LimitKind; 17] = [
    crate::LimitKind::PackDirectoryEntries,
    crate::LimitKind::PackPairs,
    crate::LimitKind::SinglePackIndexBytes,
    crate::LimitKind::CumulativePackIndexBytes,
    crate::LimitKind::IndexedObjects,
    crate::LimitKind::SinglePackBytes,
    crate::LimitKind::CumulativeVerifiedPackBytes,
    crate::LimitKind::CompressedEntryBytes,
    crate::LimitKind::InflatedEntryBytes,
    crate::LimitKind::CumulativeEntryInflateBytes,
    crate::LimitKind::DeltaProgramBytes,
    crate::LimitKind::DeltaDepth,
    crate::LimitKind::DeltaInstructions,
    crate::LimitKind::DeltaIntermediateBytes,
    crate::LimitKind::CumulativeDeltaWorkBytes,
    crate::LimitKind::ObjectLocations,
    crate::LimitKind::ReconstructedObjectCacheBytes,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandardLocalPackedLimits {
    pub pack_directory_entries: u64,
    pub pack_pairs: u64,
    pub single_pack_index_bytes: u64,
    pub cumulative_pack_index_bytes: u64,
    pub indexed_objects: u64,
    pub single_pack_bytes: u64,
    pub cumulative_verified_pack_bytes: u64,
    pub compressed_entry_bytes: u64,
    pub inflated_entry_bytes: u64,
    pub cumulative_entry_inflate_bytes: u64,
    pub delta_program_bytes: u64,
    pub delta_depth: u64,
    pub delta_instructions: u64,
    pub delta_intermediate_bytes: u64,
    pub cumulative_delta_work_bytes: u64,
    pub object_locations: u64,
    pub reconstructed_object_cache_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PackedComponent {
    Catalog,
    Index,
    Pack,
    Entry,
    Delta,
    Object,
}

impl PackedComponent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Index => "index",
            Self::Pack => "pack",
            Self::Entry => "entry",
            Self::Delta => "delta",
            Self::Object => "object",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedIndexReason {
    Layout,
    Fanout,
    Checksum,
    Sha1Collision,
}

impl PackedIndexReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "index_layout",
            Self::Fanout => "index_fanout",
            Self::Checksum => "index_checksum",
            Self::Sha1Collision => "sha1_collision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedIndexObjectReason {
    ObjectOrder,
    Offset,
}

impl PackedIndexObjectReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObjectOrder => "index_object_order",
            Self::Offset => "index_offset",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedPackReason {
    Header,
    Checksum,
    IndexMismatch,
    ObjectCount,
    Sha1Collision,
}

impl PackedPackReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "pack_header",
            Self::Checksum => "pack_checksum",
            Self::IndexMismatch => "pack_index_mismatch",
            Self::ObjectCount => "object_count",
            Self::Sha1Collision => "sha1_collision",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedEntryReason {
    Header,
    Crc,
    ZlibStream,
}

impl PackedEntryReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "entry_header",
            Self::Crc => "entry_crc",
            Self::ZlibStream => "zlib_stream",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedObjectReason {
    Size,
    Oid,
    Sha1Collision,
    DuplicateConflict,
}

impl PackedObjectReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Size => "object_size",
            Self::Oid => "object_oid",
            Self::Sha1Collision => "sha1_collision",
            Self::DuplicateConflict => "duplicate_object_conflict",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedDeltaReason {
    Base,
    Cycle,
    Program,
}

impl PackedDeltaReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Base => "delta_base",
            Self::Cycle => "delta_cycle",
            Self::Program => "delta_program",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackedObjectDatabaseInvalid {
    CatalogEntry,
    Index {
        reason: PackedIndexReason,
        pack_id: ObjectId,
    },
    IndexObject {
        reason: PackedIndexObjectReason,
        pack_id: ObjectId,
        object_oid: ObjectId,
    },
    Pack {
        reason: PackedPackReason,
        pack_id: ObjectId,
    },
    Entry {
        reason: PackedEntryReason,
        pack_id: ObjectId,
        object_oid: ObjectId,
    },
    Object {
        reason: PackedObjectReason,
        object_oid: ObjectId,
    },
    Delta {
        reason: PackedDeltaReason,
        pack_id: ObjectId,
        object_oid: ObjectId,
    },
}

impl PackedObjectDatabaseInvalid {
    #[must_use]
    pub const fn component(&self) -> PackedComponent {
        match self {
            Self::CatalogEntry => PackedComponent::Catalog,
            Self::Index { .. } | Self::IndexObject { .. } => PackedComponent::Index,
            Self::Pack { .. } => PackedComponent::Pack,
            Self::Entry { .. } => PackedComponent::Entry,
            Self::Object { .. } => PackedComponent::Object,
            Self::Delta { .. } => PackedComponent::Delta,
        }
    }

    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::CatalogEntry => "catalog_entry",
            Self::Index { reason, .. } => reason.as_str(),
            Self::IndexObject { reason, .. } => reason.as_str(),
            Self::Pack { reason, .. } => reason.as_str(),
            Self::Entry { reason, .. } => reason.as_str(),
            Self::Object { reason, .. } => reason.as_str(),
            Self::Delta { reason, .. } => reason.as_str(),
        }
    }

    #[must_use]
    pub const fn pack_id(&self) -> Option<&ObjectId> {
        match self {
            Self::Index { pack_id, .. }
            | Self::IndexObject { pack_id, .. }
            | Self::Pack { pack_id, .. }
            | Self::Entry { pack_id, .. }
            | Self::Delta { pack_id, .. } => Some(pack_id),
            Self::CatalogEntry | Self::Object { .. } => None,
        }
    }

    #[must_use]
    pub const fn object_oid(&self) -> Option<&ObjectId> {
        match self {
            Self::IndexObject { object_oid, .. }
            | Self::Entry { object_oid, .. }
            | Self::Object { object_oid, .. }
            | Self::Delta { object_oid, .. } => Some(object_oid),
            Self::CatalogEntry | Self::Index { .. } | Self::Pack { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackedAcquisitionError {
    Invalid(PackedObjectDatabaseInvalid),
    Changed(PackedComponent),
    Unavailable(PackedComponent),
}

impl PackedAcquisitionError {
    #[must_use]
    pub const fn component(&self) -> PackedComponent {
        match self {
            Self::Invalid(error) => error.component(),
            Self::Changed(component) | Self::Unavailable(component) => *component,
        }
    }

    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Invalid(error) => Some(error.reason()),
            Self::Changed(_) | Self::Unavailable(_) => None,
        }
    }
}
