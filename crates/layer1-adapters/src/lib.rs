pub mod agent_manager;
pub mod bridge;
pub mod remote_handoff;
pub mod studio_recipes;
pub mod workspace_core;

pub use agent_manager::{
    AgentDiffResult, AgentManager, AgentPreset, AgentStatus, AgentViewMode,
    ManagedAgentDetail, ManagedAgentSummary, MaskedTurn, RemoteNodeTag, SpawnAgentRequest,
    TrailEvent,
};
pub use bridge::{spawn_bridge_servers, AppState, BoundBridges, OverlayState, WindowCommand};
pub use remote_handoff::{
    AddPeerRequest, DispatchHandoffRequest, DiscoveredPeerHint, HandoffRecord, LocalHostInfo,
    PeerStatus, ReceiveHandoffRequest, RemoteHandoffManager, RemotePeer, RemoteStateResponse,
    RemoteTransport,
};
pub use studio_recipes::{
    ActiveStudioSelection, RecipeKind, RecipeRecord, SaveRecipeRequest, SnapshotEntry,
    StudioRecipes, StudioRecipesState,
};
pub use workspace_core::{McpServerEntry, WarmMcpPool, WorkspaceCoreAdapter};
