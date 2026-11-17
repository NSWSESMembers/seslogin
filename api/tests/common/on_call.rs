//! [`FakeDb`]'s on-call roles.

use seslogin::db;

use super::FakeDb;

impl db::OnCallStore for FakeDb {
    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        let roles = self.on_call_roles.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| roles.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_on_call_roles(&self, location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        Ok(self
            .on_call_roles
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.location_id == location_id && r.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_on_call_role(&self, role: db::NewOnCallRole<'_>) -> db::Result<db::OnCallRole> {
        let now = seslogin::clock::now_sec();
        let row = db::OnCallRole {
            id: role.id.to_string(),
            location_id: role.location_id.to_string(),
            name: role.name.to_string(),
            description: role.description.to_string(),
            phone: role.phone.map(str::to_string),
            primary_person_id: role.primary_person_id.map(str::to_string),
            secondary_person_ids: role.secondary_person_ids.to_vec(),
            members_changed_at: role.primary_person_id.map(|_| now),
            note_ids: role.note_ids.to_vec(),
            created_by: role.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.on_call_roles
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_on_call_role(
        &self,
        id: &str,
        change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        let now = seslogin::clock::now_sec();
        let mut roles = self.on_call_roles.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("On-call role {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let r = roles
            .get_mut(id)
            .filter(|r| r.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::OnCallRoleUpdateShape::Fields {
                name,
                description,
                phone,
                primary_person_id,
                secondary_person_ids,
                members_changed_at,
                note_ids,
                by,
                expected_version,
            } => {
                if r.version != expected_version {
                    return Err(not_found());
                }
                r.note_ids = note_ids.to_vec();
                r.name = name.to_string();
                r.description = description.to_string();
                r.phone = phone.map(str::to_string);
                r.primary_person_id = primary_person_id.map(str::to_string);
                r.secondary_person_ids = secondary_person_ids.to_vec();
                r.members_changed_at = members_changed_at;
                r.updated_by = by.map(str::to_string);
            }
            db::OnCallRoleUpdateShape::Delete { by } => {
                r.deleted = Some(now);
                r.updated_by = by.map(str::to_string);
            }
        }
        r.version += 1;
        r.updated_at = now;
        Ok(r.clone())
    }
}
