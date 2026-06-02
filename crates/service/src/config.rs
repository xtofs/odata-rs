use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use axum::response::{IntoResponse, Response};

use super::context::{
    CollectionContext, ContainedCollectionContext, ContainedEntityContext, EntityContext,
};

// ---------------------------------------------------------------------------
// Boxed async handler types
// ---------------------------------------------------------------------------

type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

pub(crate) type CollectionHandlerFn =
    Arc<dyn Fn(CollectionContext) -> BoxFuture + Send + Sync>;

pub(crate) type EntityHandlerFn =
    Arc<dyn Fn(EntityContext) -> BoxFuture + Send + Sync>;

pub(crate) type ContainedCollectionHandlerFn =
    Arc<dyn Fn(ContainedCollectionContext) -> BoxFuture + Send + Sync>;

pub(crate) type ContainedEntityHandlerFn =
    Arc<dyn Fn(ContainedEntityContext) -> BoxFuture + Send + Sync>;

// ---------------------------------------------------------------------------
// Contained navigation property config
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct ContainedNavConfig {
    pub(crate) list:   Option<ContainedCollectionHandlerFn>,
    pub(crate) get:    Option<ContainedEntityHandlerFn>,
    pub(crate) create: Option<ContainedCollectionHandlerFn>,
    pub(crate) update: Option<ContainedEntityHandlerFn>,
    pub(crate) delete: Option<ContainedEntityHandlerFn>,
}

impl ContainedNavConfig {
    pub fn list<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedCollectionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.list = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn get<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.get = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn create<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedCollectionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.create = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn update<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.update = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn delete<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.delete = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }
}

// ---------------------------------------------------------------------------
// Entity set config
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct EntitySetConfig {
    pub(crate) list:      Option<CollectionHandlerFn>,
    pub(crate) get:       Option<EntityHandlerFn>,
    pub(crate) create:    Option<CollectionHandlerFn>,
    pub(crate) update:    Option<EntityHandlerFn>,
    pub(crate) delete:    Option<EntityHandlerFn>,
    pub(crate) contained: HashMap<String, ContainedNavConfig>,
}

impl EntitySetConfig {
    pub fn list<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(CollectionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.list = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn get<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.get = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn create<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(CollectionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.create = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn update<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.update = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    pub fn delete<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.delete = Some(Arc::new(move |ctx| {
            Box::pin(handler(ctx).then_into_response())
        }));
        self
    }

    /// Register handlers for a contained navigation property.
    /// Validated against the EDM schema in `ODataServiceBuilder::build()`.
    pub fn contained(
        mut self,
        name: &str,
        f: impl FnOnce(ContainedNavConfig) -> ContainedNavConfig,
    ) -> Self {
        self.contained
            .insert(name.to_string(), f(ContainedNavConfig::default()));
        self
    }
}

// ---------------------------------------------------------------------------
// Helper trait to convert a future's output into Response
// ---------------------------------------------------------------------------

trait ThenIntoResponse: Future + Sized {
    async fn then_into_response(self) -> Response
    where
        Self::Output: IntoResponse,
    {
        self.await.into_response()
    }
}

impl<F: Future> ThenIntoResponse for F {}
