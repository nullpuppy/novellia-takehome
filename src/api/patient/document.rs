use crate::api::patient::models::DocumentSummary;
use crate::{fhir, store};
use anyhow::anyhow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

impl DocumentSummary {
    /// Returns the best available content type for this [`models::DocumentSummary`]
    ///
    /// Prefers the content type on the referenced [`fhir::Binary`] falling back
    /// to the [`fhir::DocumentReference`] attachment content type
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
    /// # Errors
    /// [`AppError::BadResource`] document reference does not reference a binary,
    /// the attached binary is missing, or has bad data
    pub fn content(&self, store: &store::Store) -> anyhow::Result<Vec<u8>> {
        self.decode_b64_attachment_content(store)
    }

    /// Decodes base64 content for this document's referenced binary
    ///
    /// # Errors
    /// [`AppError::BadResource`] binary reference cannot be resolved or decoded
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
        let binary_id = binary_url.as_ref().and_then(|uri| {
            store::resource_id_from_typed_fhir_uri("Binary", uri).map(String::from)
        });

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

impl From<&fhir::Binary> for DocumentSummary {
    fn from(value: &fhir::Binary) -> Self {
        let content_type = value.content_type.clone();
        let binary_id = value.id.clone();

        Self {
            id: binary_id.clone(),
            binary_id: Some(binary_id),
            content_type,
            ..Default::default()
        }
    }
}
