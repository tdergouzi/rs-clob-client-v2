// Module declarations
pub mod auth;
pub mod markets;
pub mod notifications;
pub mod orders;
pub mod primitives;
pub mod rewards;
pub mod server;

// Re-export all public types for backward compatibility
pub use auth::*;
pub use markets::*;
pub use notifications::*;
pub use orders::*;
pub use primitives::*;
pub use rewards::*;
pub use server::*;

