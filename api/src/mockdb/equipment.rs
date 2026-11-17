use crate::db;

use super::Handler;

impl db::EquipmentStore for Handler {
    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        Self::unsupported()
    }

    async fn list_equipment(&self, _location_id: &str) -> db::Result<Vec<db::Equipment>> {
        Self::unsupported()
    }

    async fn create_equipment(
        &self,
        _equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        Self::unsupported()
    }

    async fn update_equipment(
        &self,
        _id: &str,
        _change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        Self::unsupported()
    }

    async fn add_equipment_log_entry(
        &self,
        _entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        Self::unsupported()
    }

    async fn list_equipment_log(
        &self,
        _equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        Self::unsupported()
    }

    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        Self::unsupported()
    }

    async fn list_equipment_reservations(
        &self,
        _location_id: &str,
        _ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        Self::unsupported()
    }

    async fn create_equipment_reservation(
        &self,
        _reservation: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        Self::unsupported()
    }

    async fn update_equipment_reservation(
        &self,
        _id: &str,
        _change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        Self::unsupported()
    }
}
