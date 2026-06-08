#[cfg(feature = "edm")]
pub use csdl_edm as edm;

#[cfg(feature = "url")]
pub use odata_url as url;

#[cfg(feature = "service")]
pub use odata_service as service;
