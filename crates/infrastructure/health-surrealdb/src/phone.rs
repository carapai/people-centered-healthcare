//! Patients' phone accounts and one-time codes (migration `0012_phone_sign_in`), used by
//! `health_auth::SurrealAuthStore`.

use health_identity::{AccountId, AccountStatus, PhoneAccount, PhoneNumber};
use health_persistence::{StoreError, WriteError};
use health_time::Timestamp;

use crate::store::SurrealTx;

const PHONE_ACCOUNT: &str = "[record::id(id), phone, status, <string>verified_at]";

fn phone_account(row: Vec<String>) -> Result<PhoneAccount, StoreError> {
    let invalid = || StoreError::Unavailable("auth_phone_account row has an invalid shape".into());
    let [id, phone, status, verified_at]: [String; 4] = row.try_into().map_err(|_| invalid())?;
    Ok(PhoneAccount {
        id: AccountId(id.parse().map_err(|_| invalid())?),
        phone: PhoneNumber::try_from(phone).map_err(|_| invalid())?,
        status: if status == "active" {
            AccountStatus::Active
        } else {
            AccountStatus::Disabled
        },
        verified_at: verified_at.parse().map_err(|_| invalid())?,
    })
}

impl SurrealTx {
    /// The phone account of `phone`.
    pub async fn phone_account_by_phone(
        &mut self,
        phone: &PhoneNumber,
    ) -> Result<Option<PhoneAccount>, StoreError> {
        let rows = self
            .rows(
                &format!(
                    "SELECT VALUE {PHONE_ACCOUNT} FROM auth_phone_account WHERE phone = $phone LIMIT 1"
                ),
                vec![("phone", phone.to_string())],
            )
            .await?;
        rows.into_iter().next().map(phone_account).transpose()
    }

    /// The phone account with `id`.
    pub async fn phone_account(
        &mut self,
        id: AccountId,
    ) -> Result<Option<PhoneAccount>, StoreError> {
        let rows = self
            .rows(
                &format!(
                    "SELECT VALUE {PHONE_ACCOUNT} FROM type::record('auth_phone_account', $id)"
                ),
                vec![("id", id.to_string())],
            )
            .await?;
        rows.into_iter().next().map(phone_account).transpose()
    }

    /// Creates a phone account. A second account for the same phone fails on the unique index.
    pub async fn create_phone_account(&mut self, account: &PhoneAccount) -> Result<(), WriteError> {
        self.run(
            "CREATE type::record('auth_phone_account', $id) CONTENT { phone: $phone, status: $status, \
             verified_at: <datetime>$verified_at } RETURN VALUE record::id(id)",
            vec![
                ("id", account.id.to_string()),
                ("phone", account.phone.to_string()),
                (
                    "status",
                    match account.status {
                        AccountStatus::Active => "active",
                        AccountStatus::Disabled => "disabled",
                    }
                    .to_owned(),
                ),
                ("verified_at", account.verified_at.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// The phone's code challenge, as health-auth wrote it.
    pub async fn phone_otp(&mut self, phone: &PhoneNumber) -> Result<Option<String>, StoreError> {
        let rows = self
            .rows(
                "SELECT VALUE [data] FROM type::record('auth_otp', $phone)",
                vec![("phone", phone.to_string())],
            )
            .await?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()))
    }

    /// Replaces the phone's code challenge (a new code was sent).
    pub async fn put_phone_otp(
        &mut self,
        phone: &PhoneNumber,
        data: &str,
        issued_at: Timestamp,
    ) -> Result<(), WriteError> {
        self.run(
            "UPSERT type::record('auth_otp', $phone) CONTENT { data: $data, attempts: 0, \
             issued_at: <datetime>$issued_at } RETURN VALUE record::id(id)",
            vec![
                ("phone", phone.to_string()),
                ("data", data.to_owned()),
                ("issued_at", issued_at.to_string()),
            ],
        )
        .await?;
        Ok(())
    }

    /// Records a tried code, if the challenge is still the one issued at `issued_at` with
    /// `before` tries (compare-and-set); false when a concurrent try or a new code came first.
    pub async fn attempt_phone_otp(
        &mut self,
        phone: &PhoneNumber,
        issued_at: Timestamp,
        before: u32,
        data: &str,
    ) -> Result<bool, WriteError> {
        let updated = self
            .run(
                "UPDATE type::record('auth_otp', $phone) SET data = $data, attempts = <int>$after \
                 WHERE attempts = <int>$before AND issued_at = <datetime>$issued_at \
                 RETURN VALUE record::id(id)",
                vec![
                    ("phone", phone.to_string()),
                    ("data", data.to_owned()),
                    ("before", before.to_string()),
                    ("after", (before + 1).to_string()),
                    ("issued_at", issued_at.to_string()),
                ],
            )
            .await?;
        Ok(!updated.is_empty())
    }
}
