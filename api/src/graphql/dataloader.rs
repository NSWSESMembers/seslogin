use anyhow::anyhow;
use async_graphql::dataloader::Loader;
use std::collections::HashMap;
use std::iter::zip;
use std::sync::Arc;

use crate::app::App;
use crate::app::HasDb;
use crate::db;
use crate::db::Handler;

use super::query::{Category, Location, Person, Session, User};
use super::{CategoryId, LocationId, NitcEventId, PersonId, SessionId, UserId};

pub struct DatabaseLoader<A: App + HasDb + Send + Sync> {
    app: Arc<A>,
}

impl<A: App + HasDb + Send + Sync> DatabaseLoader<A> {
    pub fn new(app: Arc<A>) -> Self {
        DatabaseLoader { app }
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<PersonId> for DatabaseLoader<A> {
    type Value = Option<Person<A>>;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[PersonId],
    ) -> std::result::Result<HashMap<PersonId, Option<Person<A>>>, Arc<anyhow::Error>> {
        let str_keys = keys.iter().map(|k| &k.0.0).collect::<Vec<&String>>();
        let recs = self
            .app
            .db()
            .get_persons(&str_keys)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        let map: HashMap<_, _> = zip(keys.iter().cloned(), recs)
            .map(|(key, rec)| (key, rec.map(Person::new)))
            .collect();
        Ok(map)
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<LocationId> for DatabaseLoader<A> {
    type Value = Option<Location<A>>;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[LocationId],
    ) -> std::result::Result<HashMap<LocationId, Option<Location<A>>>, Arc<anyhow::Error>> {
        let str_keys = keys.iter().map(|k| &k.0.0).collect::<Vec<&String>>();
        let recs = self
            .app
            .db()
            .get_locations(&str_keys)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        let map: HashMap<_, _> = zip(keys.iter().cloned(), recs)
            .map(|(key, rec)| (key, rec.map(Location::new_db)))
            .collect();
        Ok(map)
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<CategoryId> for DatabaseLoader<A> {
    type Value = Option<Category<A>>;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[CategoryId],
    ) -> std::result::Result<HashMap<CategoryId, Option<Category<A>>>, Arc<anyhow::Error>> {
        let str_keys = keys.iter().map(|k| &k.0.0).collect::<Vec<&String>>();
        let recs = self
            .app
            .db()
            .get_categories(&str_keys)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        let map: HashMap<_, _> = zip(keys.iter().cloned(), recs)
            .map(|(key, rec)| (key, rec.map(Category::new)))
            .collect();
        Ok(map)
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<UserId> for DatabaseLoader<A> {
    type Value = Option<User<A>>;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[UserId],
    ) -> std::result::Result<HashMap<UserId, Option<User<A>>>, Arc<anyhow::Error>> {
        let str_keys = keys.iter().map(|k| &k.0.0).collect::<Vec<&String>>();
        let recs = self
            .app
            .db()
            .get_users(&str_keys)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        let map: HashMap<_, _> = zip(keys.iter().cloned(), recs)
            .map(|(key, rec)| (key, rec.map(User::new)))
            .collect();
        Ok(map)
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<SessionId> for DatabaseLoader<A> {
    type Value = Option<Session<A>>;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[SessionId],
    ) -> std::result::Result<HashMap<SessionId, Option<Session<A>>>, Arc<anyhow::Error>> {
        let str_keys = keys.iter().map(|k| &k.0.0).collect::<Vec<&String>>();
        let recs = self
            .app
            .db()
            .get_sessions(&str_keys)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        let map: HashMap<_, _> = zip(keys.iter().cloned(), recs)
            .map(|(key, rec)| (key, rec.map(Session::new)))
            .collect();
        Ok(map)
    }
}

impl<A: App + HasDb + Send + Sync + 'static> Loader<NitcEventId> for DatabaseLoader<A> {
    type Value = db::NitcEvent;
    type Error = Arc<anyhow::Error>;

    async fn load(
        &self,
        keys: &[NitcEventId],
    ) -> std::result::Result<HashMap<NitcEventId, db::NitcEvent>, Arc<anyhow::Error>> {
        let ids = keys.iter().map(|k| k.0.as_str()).collect::<Vec<&str>>();
        let recs = self
            .app
            .db()
            .get_nitc_events_by_ids(&ids)
            .await
            .map_err(|e| Arc::new(anyhow!("DB error: {:?}", e)))?;
        // A dataloader reports what exists; ids with no row are simply absent from the
        // map, which is how async-graphql signals "not found" to the resolver.
        let map = recs
            .into_iter()
            .flatten()
            .map(|rec| (NitcEventId(rec.id.clone()), rec))
            .collect();
        Ok(map)
    }
}
