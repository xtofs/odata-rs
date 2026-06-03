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
//
// Handlers are stored as `Fn(Context, S) -> BoxFuture`. The state `S` is
// cloned by the dispatcher before each call, mirroring axum's `Router::with_state`.

type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

pub(crate) type CollectionHandlerFn<S> =
    Arc<dyn Fn(CollectionContext, S) -> BoxFuture + Send + Sync>;

pub(crate) type EntityHandlerFn<S> =
    Arc<dyn Fn(EntityContext, S) -> BoxFuture + Send + Sync>;

pub(crate) type ContainedCollectionHandlerFn<S> =
    Arc<dyn Fn(ContainedCollectionContext, S) -> BoxFuture + Send + Sync>;

pub(crate) type ContainedEntityHandlerFn<S> =
    Arc<dyn Fn(ContainedEntityContext, S) -> BoxFuture + Send + Sync>;

// ---------------------------------------------------------------------------
// Contained navigation property config
// ---------------------------------------------------------------------------

pub struct ContainedNavConfig<S> {
    pub(crate) list:   Option<ContainedCollectionHandlerFn<S>>,
    pub(crate) get:    Option<ContainedEntityHandlerFn<S>>,
    pub(crate) create: Option<ContainedCollectionHandlerFn<S>>,
    pub(crate) update: Option<ContainedEntityHandlerFn<S>>,
    pub(crate) delete: Option<ContainedEntityHandlerFn<S>>,
}

impl<S> Default for ContainedNavConfig<S> {
    fn default() -> Self {
        Self {
            list: None,
            get: None,
            create: None,
            update: None,
            delete: None,
        }
    }
}

impl<S> Clone for ContainedNavConfig<S> {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            get: self.get.clone(),
            create: self.create.clone(),
            update: self.update.clone(),
            delete: self.delete.clone(),
        }
    }
}

impl<S> ContainedNavConfig<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn list<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedCollectionContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.list = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn get<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.get = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn create<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedCollectionContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.create = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn update<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.update = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn delete<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(ContainedEntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.delete = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }
}

// ---------------------------------------------------------------------------
// Entity set config
// ---------------------------------------------------------------------------

pub struct EntitySetConfig<S> {
    pub(crate) list:      Option<CollectionHandlerFn<S>>,
    pub(crate) get:       Option<EntityHandlerFn<S>>,
    pub(crate) create:    Option<CollectionHandlerFn<S>>,
    pub(crate) update:    Option<EntityHandlerFn<S>>,
    pub(crate) delete:    Option<EntityHandlerFn<S>>,
    pub(crate) contained: HashMap<String, ContainedNavConfig<S>>,
}

impl<S> Default for EntitySetConfig<S> {
    fn default() -> Self {
        Self {
            list: None,
            get: None,
            create: None,
            update: None,
            delete: None,
            contained: HashMap::new(),
        }
    }
}

impl<S> Clone for EntitySetConfig<S> {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            get: self.get.clone(),
            create: self.create.clone(),
            update: self.update.clone(),
            delete: self.delete.clone(),
            contained: self.contained.clone(),
        }
    }
}

impl<S> EntitySetConfig<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn list<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(CollectionContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.list = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn get<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.get = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn create<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(CollectionContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.create = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn update<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.update = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    pub fn delete<H, Fut, R>(mut self, handler: H) -> Self
    where
        H: Fn(EntityContext, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.delete = Some(Arc::new(move |ctx, s| {
            Box::pin(handler(ctx, s).then_into_response())
        }));
        self
    }

    /// Register handlers for a contained navigation property.
    /// Validated against the EDM schema in `ODataServiceBuilder::build()`.
    pub fn contained(
        mut self,
        name: &str,
        f: impl FnOnce(ContainedNavConfig<S>) -> ContainedNavConfig<S>,
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
