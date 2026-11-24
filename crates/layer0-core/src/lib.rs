pub mod guard;
pub mod index;
pub mod seam;

pub use guard::ImmutableCoreGuard;
pub use index::{default_workspace_roots, SearchMatch, WorkspaceIndex};
pub use seam::{
    CapabilitySeam, EffectGuard, LifecycleHook, SeamContext, SeamError, SeamMetadata, SeamRegistry,
};
