//! Enrolled LAN devices (lan-security.md § Device enrollment; ADR 0009).
//!
//! A device asks to enroll with a key it cannot export, and shows the pairing code the edge gave
//! it. A facility admin approves it by entering that code, naming the device and assigning it a
//! location. The request expires 10 minutes after it was made. Once enrolled, the device's key is
//! the DPoP key for every request, and revoking the device ends its access at the next one.

use std::fmt;

use health_time::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A device's id: a UUIDv7 minted by the edge at enrollment, never reused, bound to one key
/// (identifiers.md). A re-enrolled device gets a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub Uuid);

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The enrollment rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrollmentPolicy {
    /// How long a request waits for approval.
    pub lifetime: SignedDuration,
    /// Digits in a pairing code.
    pub digits: u32,
}

impl Default for EnrollmentPolicy {
    /// Pending requests expire after 10 minutes; codes have 6 digits.
    fn default() -> Self {
        Self {
            lifetime: SignedDuration::from_mins(10),
            digits: 6,
        }
    }
}

impl EnrollmentPolicy {
    /// The pairing code for a random `draw`: `digits` decimal digits, zero-padded.
    pub fn code(&self, draw: u64) -> String {
        let modulus = 10u64.pow(self.digits);
        format!("{:0width$}", draw % modulus, width = self.digits as usize)
    }

    /// A request for the key with thumbprint `thumbprint`, made at `now`.
    pub fn request(
        &self,
        id: Uuid,
        thumbprint: String,
        label: String,
        code: String,
        now: Timestamp,
    ) -> EnrollmentRequest {
        EnrollmentRequest {
            id,
            thumbprint,
            label,
            code,
            requested_at: now,
            expires: now.checked_add(self.lifetime).unwrap_or(now),
        }
    }
}

/// A device's request to enroll, waiting for an admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollmentRequest {
    /// The request's id.
    pub id: Uuid,
    /// The RFC 7638 thumbprint of the device's public key.
    pub thumbprint: String,
    /// What the device says it is (browser and platform), for the admin to recognise it.
    pub label: String,
    /// The pairing code the device shows. Unique among pending requests.
    pub code: String,
    /// When it was made.
    pub requested_at: Timestamp,
    /// When it lapses unapproved.
    pub expires: Timestamp,
}

/// Why an enrollment was not approved, or a device not revoked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnrollmentRefused {
    /// No pending request has that code.
    #[error("no pending request has that pairing code")]
    NoSuchCode,
    /// The request waited too long.
    #[error("the request has expired: ask the device to enroll again")]
    Expired,
    /// The device needs a name and a location.
    #[error("{0}")]
    Invalid(&'static str),
    /// The device is already revoked.
    #[error("the device is already revoked")]
    AlreadyRevoked,
}

/// Whether an enrolled device may still be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceStatus {
    /// Its key is accepted.
    Active,
    /// Its key is refused, for good.
    Revoked,
}

/// A device in the edge's device registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// The device id.
    pub id: DeviceId,
    /// The RFC 7638 thumbprint of its key: what its DPoP proofs are signed with.
    pub thumbprint: String,
    /// The name the admin gave it.
    pub name: String,
    /// The location or ward it is assigned to.
    pub location: String,
    /// What the device said it was when it asked.
    pub label: String,
    /// Who approved it.
    pub enrolled_by: String,
    /// When.
    pub enrolled_at: Timestamp,
    /// Active or revoked.
    pub status: DeviceStatus,
    /// Who revoked it, and when.
    pub revoked: Option<(String, Timestamp)>,
    /// Bumped by every change, for sync (synchronization.md: the registry syncs by version, and
    /// revocation wins).
    pub version: u64,
}

impl EnrollmentRequest {
    /// Whether the request has lapsed at `now`.
    pub fn expired(&self, now: Timestamp) -> bool {
        now >= self.expires
    }

    /// Approves the request as device `id`, named `name` at `location`, by `by` at `now`.
    pub fn approve(
        &self,
        id: DeviceId,
        name: &str,
        location: &str,
        by: &str,
        now: Timestamp,
    ) -> Result<Device, EnrollmentRefused> {
        if self.expired(now) {
            return Err(EnrollmentRefused::Expired);
        }
        let (name, location) = (name.trim(), location.trim());
        if name.is_empty() {
            return Err(EnrollmentRefused::Invalid("the device needs a name"));
        }
        if location.is_empty() {
            return Err(EnrollmentRefused::Invalid("the device needs a location"));
        }
        Ok(Device {
            id,
            thumbprint: self.thumbprint.clone(),
            name: name.to_owned(),
            location: location.to_owned(),
            label: self.label.clone(),
            enrolled_by: by.to_owned(),
            enrolled_at: now,
            status: DeviceStatus::Active,
            revoked: None,
            version: 1,
        })
    }
}

impl Device {
    /// Whether its key is accepted.
    pub fn active(&self) -> bool {
        self.status == DeviceStatus::Active
    }

    /// Revokes the device, by `by` at `now`. Revocation is final.
    pub fn revoke(&mut self, by: &str, now: Timestamp) -> Result<(), EnrollmentRefused> {
        if !self.active() {
            return Err(EnrollmentRefused::AlreadyRevoked);
        }
        self.status = DeviceStatus::Revoked;
        self.revoked = Some((by.to_owned(), now));
        self.version += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("timestamp")
    }

    fn request() -> EnrollmentRequest {
        EnrollmentPolicy::default().request(
            Uuid::nil(),
            "jkt".into(),
            "Chrome on Windows".into(),
            "042137".into(),
            at("2026-11-02T09:00:00Z"),
        )
    }

    #[test]
    fn codes_are_zero_padded_digits() {
        let policy = EnrollmentPolicy::default();
        assert_eq!(policy.code(42), "000042");
        assert_eq!(policy.code(1_234_567), "234567");
    }

    #[test]
    fn a_request_lapses_after_ten_minutes() {
        let request = request();
        let device = DeviceId(Uuid::nil());
        assert!(!request.expired(at("2026-11-02T09:09:59Z")));
        assert_eq!(
            request.approve(
                device,
                "Ward 3 PC",
                "Ward 3",
                "admin",
                at("2026-11-02T09:10:00Z")
            ),
            Err(EnrollmentRefused::Expired)
        );
        let enrolled = request
            .approve(
                device,
                " Ward 3 PC ",
                "Ward 3",
                "admin",
                at("2026-11-02T09:09:59Z"),
            )
            .expect("approved");
        assert_eq!(enrolled.name, "Ward 3 PC");
        assert_eq!(enrolled.thumbprint, "jkt");
        assert!(enrolled.active());
    }

    #[test]
    fn a_device_needs_a_name_and_a_location() {
        let now = at("2026-11-02T09:01:00Z");
        let device = DeviceId(Uuid::nil());
        assert!(matches!(
            request().approve(device, " ", "Ward 3", "admin", now),
            Err(EnrollmentRefused::Invalid(_))
        ));
        assert!(matches!(
            request().approve(device, "PC", "", "admin", now),
            Err(EnrollmentRefused::Invalid(_))
        ));
    }

    #[test]
    fn revocation_is_final_and_versioned() {
        let now = at("2026-11-02T09:01:00Z");
        let mut device = request()
            .approve(DeviceId(Uuid::nil()), "PC", "Ward 3", "admin", now)
            .expect("approved");
        device.revoke("admin", now).expect("revoked");
        assert!(!device.active());
        assert_eq!(device.version, 2);
        assert_eq!(
            device.revoke("admin", now),
            Err(EnrollmentRefused::AlreadyRevoked)
        );
    }
}
