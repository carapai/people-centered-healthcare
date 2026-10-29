use uuid::Uuid;

/// Which UUID versions an id type accepts when parsed or wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionRule {
    /// Only UUIDv7: ids minted by a node.
    V7,
    /// UUIDv7, or UUIDv5 for records derived from another record.
    V7OrV5,
}

impl VersionRule {
    /// Whether `uuid` satisfies this rule.
    pub fn accepts(self, uuid: &Uuid) -> bool {
        matches!((self, uuid.get_version_num()), (_, 7) | (Self::V7OrV5, 5))
    }
}

/// Why a string or UUID is not a valid id of a given type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    /// Not a UUID in canonical lowercase hyphenated form.
    #[error("`{0}` is not a lowercase hyphenated UUID")]
    NotCanonical(String),
    /// A UUID of a version this id type does not accept.
    #[error("{kind} must be {rule:?}, got UUID version {version}")]
    WrongVersion {
        /// The id type.
        kind: &'static str,
        /// The rule it enforces.
        rule: VersionRule,
        /// The version found.
        version: usize,
    },
}

/// Parses `text` as a canonical id: exactly the lowercase hyphenated form, nothing else.
#[doc(hidden)]
pub fn parse_canonical(kind: &'static str, rule: VersionRule, text: &str) -> Result<Uuid, IdError> {
    let uuid = Uuid::try_parse(text).map_err(|_| IdError::NotCanonical(text.to_owned()))?;
    if uuid.hyphenated().to_string() != text {
        return Err(IdError::NotCanonical(text.to_owned()));
    }
    check_version(kind, rule, uuid)
}

#[doc(hidden)]
pub fn check_version(kind: &'static str, rule: VersionRule, uuid: Uuid) -> Result<Uuid, IdError> {
    if rule.accepts(&uuid) {
        Ok(uuid)
    } else {
        Err(IdError::WrongVersion {
            kind,
            rule,
            version: uuid.get_version_num(),
        })
    }
}

/// Declares a typed id: a UUID newtype that mints UUIDv7, prints lowercase hyphenated, and
/// parses only that form, enforcing its [`VersionRule`](crate::VersionRule).
///
/// ```
/// health_identifiers::define_id! {
///     /// A Patient resource.
///     PatientId, health_identifiers::VersionRule::V7OrV5
/// }
/// let id = PatientId::new();
/// assert_eq!(id.to_string().parse::<PatientId>(), Ok(id));
/// ```
#[macro_export]
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $rule:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($crate::__private::uuid::Uuid);

        impl $name {
            /// The version rule this id type enforces.
            pub const RULE: $crate::VersionRule = $rule;

            /// Mints a new UUIDv7 id.
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self($crate::__private::uuid::Uuid::now_v7())
            }

            /// Wraps an existing UUID, enforcing the version rule.
            pub fn from_uuid(uuid: $crate::__private::uuid::Uuid) -> ::core::result::Result<Self, $crate::IdError> {
                $crate::__id_support::check_version(stringify!($name), Self::RULE, uuid).map(Self)
            }

            /// The underlying UUID.
            pub fn as_uuid(&self) -> &$crate::__private::uuid::Uuid {
                &self.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::IdError;

            fn from_str(text: &str) -> ::core::result::Result<Self, Self::Err> {
                $crate::__id_support::parse_canonical(stringify!($name), Self::RULE, text).map(Self)
            }
        }

        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                let text = <::std::borrow::Cow<'de, str> as $crate::__private::serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err($crate::__private::serde::de::Error::custom)
            }
        }
    };
}
