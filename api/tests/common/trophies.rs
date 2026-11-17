//! [`FakeDb`]'s perpetual trophies.

use seslogin::db;

use super::FakeDb;

impl db::TrophyStore for FakeDb {
    async fn list_trophy_holders(&self, location_id: &str) -> db::Result<Vec<db::TrophyHolder>> {
        let mut holders: Vec<db::TrophyHolder> = self
            .trophy_holders
            .lock()
            .unwrap()
            .values()
            .filter(|h| h.location_id == location_id)
            .cloned()
            .collect();
        holders.sort_by(|a, b| a.trophy_id.cmp(&b.trophy_id));
        Ok(holders)
    }

    async fn list_trophy_reigns(
        &self,
        location_id: &str,
        trophy_id: &str,
        limit: usize,
    ) -> db::Result<Vec<db::TrophyReign>> {
        let mut reigns: Vec<db::TrophyReign> = self
            .trophy_reigns
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.location_id == location_id && r.trophy_id == trophy_id)
            .cloned()
            .collect();
        reigns.sort_by_key(|r| std::cmp::Reverse(db::trophy_reign_sk(r)));
        reigns.truncate(limit);
        Ok(reigns)
    }

    async fn write_trophy(&self, write: db::TrophyWrite) -> db::Result<()> {
        let key = (write.location_id.clone(), write.trophy_id.clone());
        let mut holders = self.trophy_holders.lock().unwrap();
        // The same condition as the DynamoDB transaction.
        if holders.get(&key).map(|h| h.version) != write.expected_version {
            return Err(db::Error::Conflict(format!(
                "trophy {} at {} changed since it was read",
                write.trophy_id, write.location_id
            )));
        }
        match write.holder {
            Some(mut holder) => {
                holder.version = write.expected_version.map_or(1, |v| v + 1);
                holders.insert(key, holder);
            }
            None => {
                holders.remove(&key);
            }
        }
        if let Some(reign) = write.ended_reign {
            self.trophy_reigns.lock().unwrap().push(reign);
        }
        let mut notices = self.trophy_notices.lock().unwrap();
        for notice in write.notices {
            let sk = db::trophy_notice_sk(&notice);
            notices.retain(|n| {
                !(n.location_id == notice.location_id && db::trophy_notice_sk(n) == sk)
            });
            notices.push(notice);
        }
        Ok(())
    }

    async fn list_trophy_notices(
        &self,
        location_id: &str,
        person_id: &str,
    ) -> db::Result<Vec<db::TrophyNotice>> {
        let mut notices: Vec<db::TrophyNotice> = self
            .trophy_notices
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.location_id == location_id && n.person_id == person_id)
            .cloned()
            .collect();
        notices.sort_by_key(|n| n.at);
        Ok(notices)
    }

    async fn delete_trophy_notices(&self, notices: &[db::TrophyNotice]) -> db::Result<()> {
        self.trophy_notices
            .lock()
            .unwrap()
            .retain(|n| !notices.contains(n));
        Ok(())
    }
}
