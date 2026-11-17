//! [`FakeDb`]'s shared contacts.

use std::sync::atomic::Ordering;

use seslogin::db;

use super::FakeDb;

impl db::SharedContactStore for FakeDb {
    async fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::SharedContact>>> {
        let contacts = self.shared_contacts.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| contacts.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_shared_contacts(
        &self,
        scope: db::SharedContactScope<'_>,
    ) -> db::Result<Vec<db::SharedContact>> {
        Ok(self
            .shared_contacts
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.location_id.as_deref() == scope.location_id() && c.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn list_all_shared_contacts(&self) -> db::Result<Vec<db::SharedContact>> {
        if self.fail_shared_contact_reads.load(Ordering::SeqCst) {
            return Err(db::Error::Infrastructure(
                "ResourceNotFoundException: shared_contact".to_string(),
            ));
        }
        Ok(self
            .shared_contacts
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_shared_contact(
        &self,
        contact: db::NewSharedContact<'_>,
    ) -> db::Result<db::SharedContact> {
        let now = seslogin::clock::now_sec();
        let row = db::SharedContact {
            id: db::new_id(),
            location_id: contact.scope.location_id().map(str::to_string),
            name: contact.name.to_string(),
            description: contact.description.to_string(),
            phone: contact.phone.map(str::to_string),
            phone_can_sms: contact.phone_can_sms,
            email: contact.email.map(str::to_string),
            created_by: contact.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.shared_contacts
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_shared_contact(
        &self,
        id: &str,
        change: db::SharedContactUpdateShape<'_>,
    ) -> db::Result<db::SharedContact> {
        let now = seslogin::clock::now_sec();
        let mut contacts = self.shared_contacts.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Shared contact {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let c = contacts
            .get_mut(id)
            .filter(|c| c.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::SharedContactUpdateShape::Fields {
                name,
                description,
                phone,
                phone_can_sms,
                email,
                by,
                expected_version,
            } => {
                if c.version != expected_version {
                    return Err(not_found());
                }
                c.name = name.to_string();
                c.description = description.to_string();
                c.phone = phone.map(str::to_string);
                c.phone_can_sms = phone_can_sms;
                c.email = email.map(str::to_string);
                c.updated_by = by.map(str::to_string);
            }
            db::SharedContactUpdateShape::Delete { by } => {
                c.deleted = Some(now);
                c.updated_by = by.map(str::to_string);
            }
        }
        c.version += 1;
        c.updated_at = now;
        Ok(c.clone())
    }
}
