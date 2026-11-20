//! A draft's one discussion (studio.md § The authoring API, Discussion; #496).
//!
//! The thread is the draft's records, oldest first: the history the lifecycle writes (created,
//! revised, submitted, reviewed, approved, handed over, ...) and the comments people add, which
//! are records too ([`Lifecycle::comment`]). A client sends only a comment's text and the
//! artefact it names; the record is the lifecycle's. Each is audited like any other record, and
//! none is ever removed.
//!
//! A comment moves nothing: neither the stage nor the draft's revision changes, so an editor
//! open on the draft does not go stale. It may name one artefact of the draft by its path.

use health_configuration::lifecycle::{Lifecycle, Refused, Stage};
use health_persistence::Transaction as _;

use crate::studio::Refusal;
use crate::{Error, Publications, failed};

/// Why no one may comment on `lifecycle` any more; `None` while its discussion is open. Anyone
/// the Studio lets in may comment: the API checks that.
pub fn comment_refusal(lifecycle: &Lifecycle) -> Option<Refusal> {
    matches!(lifecycle.stage, Stage::Published | Stage::Withdrawn)
        .then(|| Refusal::from(&Refused::Stage(lifecycle.stage)))
}

impl Publications {
    /// `by` comments `text` on package version `id`, naming the artefact at path `about` if
    /// they like, which must be one of its artefacts.
    pub async fn comment(
        &self,
        id: &str,
        by: &str,
        text: &str,
        about: Option<&str>,
    ) -> Result<Lifecycle, Error> {
        let mut tx = self.begin().await?;
        let (lifecycle, _) = self.load(&mut tx, id).await?;
        let about = about.map(str::trim).filter(|path| !path.is_empty());
        let record = lifecycle.comment(by, text, about, self.clock.now())?;
        let detail = match about {
            Some(path) => {
                let packed = self.packed(&lifecycle.content).await?;
                if !packed.contents.artefacts.iter().any(|a| a.path == path) {
                    return Err(Error::Invalid(format!("{id} has no artefact {path}")));
                }
                format!("commented on {path}")
            }
            None => "commented".to_owned(),
        };
        self.append(&mut tx, id, &[(record, detail)]).await?;
        tx.commit().await.map_err(failed)?;
        Ok(lifecycle)
    }
}
