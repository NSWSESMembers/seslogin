use crate::db;

use super::Handler;

impl db::TrophyStore for Handler {
    async fn list_trophy_holders(&self, _location_id: &str) -> db::Result<Vec<db::TrophyHolder>> {
        Self::unsupported()
    }

    async fn list_trophy_reigns(
        &self,
        _location_id: &str,
        _trophy_id: &str,
        _limit: usize,
    ) -> db::Result<Vec<db::TrophyReign>> {
        Self::unsupported()
    }

    async fn write_trophy(&self, _write: db::TrophyWrite) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_trophy_notices(
        &self,
        _location_id: &str,
        _person_id: &str,
    ) -> db::Result<Vec<db::TrophyNotice>> {
        Self::unsupported()
    }

    async fn delete_trophy_notices(&self, _notices: &[db::TrophyNotice]) -> db::Result<()> {
        Self::unsupported()
    }
}
