use crate::api::patient::models::DocumentSummary;
use crate::{fhir, store};
use anyhow::anyhow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

impl DocumentSummary {

    /// Gets the best possible content type for the binary referenced
    /// by this instance
    ///
    /// # Returns
    /// an option that will contain either the content type from the
    /// binary, if we could get it, and it's not empty, the content type
    /// from the attachment, or None if no content type could be found
    #[must_use]
    pub fn content_type(&self, store: &store::Store) -> Option<String> {
        let content_type = self
            .binary_id
            .as_ref()
            .map(|id| store::normalize_id(id))
            .and_then(|id| store.binaries.get(&id))
            .and_then(|binary| binary.content_type.clone());

        if content_type.as_ref().is_some_and(|s| !s.is_empty()) {
            content_type
        } else {
            self.content_type.clone()
        }
    }

    /// Decodes this document's referenced binary content.
    ///
    /// # Results
    /// Decoded vector of bytes
    ///
    /// # Errors
    /// document does not reference a binary,
    /// binary that document references is missing,
    /// binary that document references is missing data,
    /// binary that document references has invalid baso64 contents
    /// [`AppError::BadResource`]
    pub fn content(&self, store: &store::Store) -> anyhow::Result<Vec<u8>> {
        self.decode_b64_attachment_content(store)
    }


    /// Decodes base64 content for the binary_id present on the
    /// document
    ///
    /// # Results
    /// Decoded vector of bytes
    ///
    /// # Errors
    /// document does not reference a binary,
    /// binary that document references is missing,
    /// binary that document references is missing data,
    /// binary that document references has invalid baso64 contents
    /// [`AppError::BadResource`]
    fn decode_b64_attachment_content(&self, store: &store::Store) -> anyhow::Result<Vec<u8>> {
        let binary_id = self
            .binary_id
            .as_deref()
            .map(store::normalize_id)
            .ok_or_else(|| anyhow!("document '{}' does not have a binary reference", self.id))?;

        let binary = store.binaries.get(&binary_id).ok_or_else(|| {
            anyhow!(
                "document '{}' references unknown binary '{binary_id}'",
                self.id
            )
        })?;

        let data = binary.data.as_deref().ok_or_else(|| {
            anyhow!(
                "document '{}' references binary '{binary_id}' binary does not have data",
                self.id
            )
        })?;

        STANDARD.decode(data).map_err(|err| anyhow!(err))
    }
}

impl From<&fhir::DocumentReference> for DocumentSummary {
    fn from(doc: &fhir::DocumentReference) -> Self {
        let attachment = doc.content.first().and_then(|c| c.attachment.as_ref());
        let binary_url = attachment
            .and_then(|a| a.url.clone())
            .filter(|s| !s.is_empty());
        let binary_id = binary_url
            .as_ref()
            .and_then(|uri| store::resource_id_from_typed_uri("Binary", uri).map(String::from));

        let content_type = attachment
            .and_then(|a| a.content_type.clone())
            .filter(|s| !s.is_empty());

        Self {
            id: doc.id.clone(),
            status: doc.status.clone().unwrap_or_default(),
            date: doc.date.clone().unwrap_or_default(),
            author: doc
                .author
                .iter()
                .filter_map(|r| r.reference.clone())
                .collect(),
            content_type,
            binary_id,
        }
    }
}
